use std::sync::OnceLock;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::response::IntoResponse;
use http_body_util::BodyExt;
use serde_json::Value;
use testcontainers::core::{IntoContainerPort, WaitFor};
use testcontainers::runners::AsyncRunner;
use testcontainers::{GenericImage, ImageExt};
use tokio::sync::Mutex;
use tower::ServiceExt;
use url_shortener::config::{AppEnv, Config};
use url_shortener::encoding::base62;
use url_shortener::error::{friendly_not_found, AppError};
use url_shortener::repo::url_mapping::{SqlxUrlMappingRepository, UrlMapping, UrlMappingRepository};
use url_shortener::schedule::cleanup;
use url_shortener::schema;
use url_shortener::service::url_shortener::UrlShortenerService;
use url_shortener::web;
use url_shortener::web::html_sanitize::{finding_in_body, inspects};
use url_shortener::web::html_sanitizer::HtmlSanitizer;
use url_shortener::web::legal_path::LegalPaths;
use url_shortener::web::public_access::PublicAccess;
use url_shortener::web::redirect::redirect;

static GATE: Mutex<()> = Mutex::const_new(());
static DB: OnceLock<Mutex<Db>> = OnceLock::new();

struct Db {
    port: u16,
    _container: testcontainers::ContainerAsync<GenericImage>,
}

async fn database() -> &'static Mutex<Db> {
    if let Some(existing) = DB.get() {
        return existing;
    }
    let container = GenericImage::new("mysql", "8.0")
        .with_exposed_port(3306.tcp())
        .with_wait_for(WaitFor::message_on_stderr(
            "port: 3306  MySQL Community Server",
        ))
        .with_env_var("MYSQL_ROOT_PASSWORD", "test")
        .with_startup_timeout(Duration::from_secs(180))
        .start()
        .await
        .expect("MySQL 8.0 test container");
    let port = container
        .get_host_port_ipv4(3306)
        .await
        .expect("MySQL host port");
    let _ = DB.set(Mutex::new(Db {
        port,
        _container: container,
    }));
    DB.get().expect("database lock")
}

async fn test_config(port: u16) -> url_shortener::config::Config {
    url_shortener::config::Config::from_lookup(|name| match name {
        "APP_ENV" => Some("test".to_string()),
        "APP_BASE_URL" => Some("http://localhost:8080".to_string()),
        "DB_HOST" => Some("127.0.0.1".to_string()),
        "DB_PORT" => Some(port.to_string()),
        "DB_NAME" => Some("shortener_dev".to_string()),
        "DB_USER" => Some("root".to_string()),
        "DB_PASSWORD" => Some("test".to_string()),
        _ => None,
    })
    .expect("test config")
}

async fn app() -> axum::Router {
    let db = database().await.lock().await;
    let config = test_config(db.port).await;
    let pool = schema::connect(&config).await.expect("schema");
    let service = UrlShortenerService::new(SqlxUrlMappingRepository::new(pool), config.base_url.clone());
    let openapi = config.openapi_enabled.then(|| config.base_url.clone());
    web::application(service, openapi.as_deref())
}

async fn call(method: &str, uri: &str, body: Option<&str>) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let mut builder = Request::builder().method(method).uri(uri);
    if body.is_some() {
        builder = builder.header("content-type", "application/json");
    }
    let request = builder
        .body(Body::from(body.unwrap_or("").to_string()))
        .unwrap();
    let response = app().await.oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, headers, bytes.to_vec())
}

#[tokio::test]
async fn post_creates_a_short_link_and_reuses_it() {
    let _guard = GATE.lock().await;
    let payload = r#"{"longUrl":"https://example.com/api-design/api-design-url-shortening-service/"}"#;
    let (status, _, bytes) = call("POST", "/api/v1/shorten", Some(payload)).await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    let short_code = body["shortCode"].as_str().unwrap();
    assert!(short_code.len() >= 5 && short_code.len() <= 7);
    let short_url = body["shortUrl"].as_str().unwrap();
    assert!(short_url.starts_with("http://localhost:8080/short/"));
    assert!(body["expiresAt"].as_str().is_some());

    let (again, _, again_bytes) = call("POST", "/api/v1/shorten", Some(payload)).await;
    assert_eq!(
        again,
        StatusCode::CREATED,
        "{}",
        String::from_utf8_lossy(&again_bytes)
    );
    let again_body: Value = serde_json::from_slice(&again_bytes).unwrap();
    assert_eq!(again_body["shortCode"].as_str().unwrap(), short_code);
}

#[tokio::test]
async fn post_rejects_an_invalid_url_and_a_self_loop() {
    let _guard = GATE.lock().await;
    let (status, _, bytes) = call(
        "POST",
        "/api/v1/shorten",
        Some(r#"{"longUrl":"not-a-valid-url"}"#),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["status"], 400);
    assert!(body["message"].as_str().unwrap().len() > 0);

    let (loop_status, _, loop_bytes) = call(
        "POST",
        "/api/v1/shorten",
        Some(r#"{"longUrl":"http://localhost:8080/own"}"#),
    )
    .await;
    assert_eq!(loop_status, StatusCode::BAD_REQUEST);
    let loop_body: Value = serde_json::from_slice(&loop_bytes).unwrap();
    assert_eq!(loop_body["status"], 400);
    assert!(loop_body.get("shortCode").unwrap().is_null());
}

#[tokio::test]
async fn get_redirects_an_active_code_and_hides_missing_or_expired() {
    let _guard = GATE.lock().await;
    let long_url = "https://example.com/redirect-target";
    let payload = format!(r#"{{"longUrl":"{long_url}"}}"#);
    let (created_status, _, created_bytes) = call("POST", "/api/v1/shorten", Some(&payload)).await;
    assert_eq!(
        created_status,
        StatusCode::CREATED,
        "{}",
        String::from_utf8_lossy(&created_bytes)
    );
    let created: Value = serde_json::from_slice(&created_bytes).unwrap();
    let code = created["shortCode"].as_str().unwrap();

    let (status, headers, redirect_body) = call("GET", &format!("/short/{code}"), None).await;
    assert_eq!(status, StatusCode::FOUND, "{}", String::from_utf8_lossy(&redirect_body));
    assert_eq!(headers.get("location").unwrap().to_str().unwrap(), long_url);

    let (missing, missing_headers, missing_body) = call("GET", "/short/unknown", None).await;
    assert_eq!(missing, StatusCode::NOT_FOUND);
    let missing_type = missing_headers
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(missing_type.contains("text/html"));
    let missing_html = String::from_utf8(missing_body.to_vec()).unwrap();
    assert!(missing_html.contains("<h1>Link not found</h1>"));
    assert!(missing_html.contains("<p>/short/unknown</p>"));
    assert!(missing_html.contains("POST /api/v1/shorten"));
    assert!(missing_html.contains("/swagger-ui.html"));
    assert!(missing_html.contains("/short/{code}"));
    assert!(!missing_html.contains("Whitelabel"));
    assert!(!missing_html.starts_with('{'));

    seed_expired("expired1", "https://example.com/expired").await;
    let (expired, expired_headers, expired_body) = call("GET", "/short/expired1", None).await;
    assert_eq!(expired, StatusCode::NOT_FOUND);
    assert!(expired_headers
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap()
        .contains("text/html"));
    let expired_html = String::from_utf8(expired_body.to_vec()).unwrap();
    assert!(expired_html.contains("Link not found"));
}

#[tokio::test]
async fn cleanup_removes_expired_rows_and_the_listener_binds() {
    let _guard = GATE.lock().await;
    seed_expired("cleanup1", "https://example.com/cleanup").await;
    let port = {
        let db = database().await.lock().await;
        db.port
    };
    let config = test_config(port).await;
    let pool = schema::connect(&config).await.expect("schema");
    let removed = cleanup::delete_expired(&SqlxUrlMappingRepository::new(pool), chrono::Utc::now())
        .await
        .expect("cleanup");
    assert!(removed >= 1);

    let mut listening = test_config(port).await;
    listening.bind_addr = "127.0.0.1:0".to_string();
    let server = tokio::spawn(url_shortener::serve_with(listening));
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert!(
        !server.is_finished(),
        "listener exited early: {:?}",
        server.await
    );
    server.abort();
}

#[tokio::test]
async fn helpers_reject_bad_input_and_round_trip_ids() {
    assert_eq!(base62::encode(0), "0");
    assert!(base62::decode("").is_err());
    assert!(base62::decode("nope+").is_err());
    assert!(base62::decode(&"z".repeat(40)).is_err());
    let encoded = base62::encode(56_800_235);
    assert_eq!(base62::decode(&encoded).unwrap(), 56_800_235);

    let dev = Config::from_lookup(|_| None).unwrap();
    assert_eq!(dev.env, AppEnv::Dev);
    assert!(dev.apply_schema);
    assert!(dev.database_url().contains("ssl-mode=disabled"));
    let prod = Config::from_lookup(|name| match name {
        "APP_ENV" => Some("prod".to_string()),
        "APP_BASE_URL" => Some("https://short.example/".to_string()),
        "DB_HOST" => Some("db".to_string()),
        "DB_USER" => Some("app".to_string()),
        "DB_PASSWORD" => Some("p@ss word".to_string()),
        "DB_PORT" => Some("3307".to_string()),
        _ => None,
    })
    .unwrap();
    assert_eq!(prod.env, AppEnv::Prod);
    assert!(!prod.apply_schema);
    assert!(!prod.openapi_enabled);
    let enabled = Config::from_lookup(|name| match name {
        "APP_ENV" => Some("prod".to_string()),
        "APP_BASE_URL" => Some("https://short.example".to_string()),
        "DB_HOST" => Some("db".to_string()),
        "DB_USER" => Some("app".to_string()),
        "DB_PASSWORD" => Some("secret".to_string()),
        "OPENAPI_ENABLED" => Some("TRUE".to_string()),
        _ => None,
    })
    .unwrap();
    assert!(enabled.openapi_enabled);
    assert!(prod.database_url().contains("ssl-mode=required"));
    assert!(prod.database_url().contains("p%40ss%20word"));
    assert!(Config::from_lookup(|name| match name {
        "DB_PORT" => Some("nope".to_string()),
        _ => None,
    })
    .is_err());

    let bad = AppError::BadRequest("longUrl is required".to_string());
    assert_eq!(bad.into_response().status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        AppError::Unavailable("down".to_string()).into_response().status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        friendly_not_found("a&b<c>\"d").status(),
        StatusCode::NOT_FOUND
    );
    let page = friendly_not_found("a&b<c>\"d");
    let page_html = String::from_utf8(page.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap();
    assert!(page_html.contains("a&amp;b&lt;c&gt;&quot;d"));
    assert!(page_html.contains("<h1>Link not found</h1>"));
    assert!(page_html.contains("POST /api/v1/shorten"));
    assert!(page_html.contains("href=\"/swagger-ui.html\""));
    assert!(page_html.contains("/short/{code}"));
    assert!(!page_html.contains("stack trace"));
    let cases = [
        (axum::http::Method::POST, "/api/v1/shorten", true),
        (axum::http::Method::GET, "/api/v1/shorten", false),
        (axum::http::Method::GET, "/short/b9K2x1", true),
        (axum::http::Method::GET, "/short/0", true),
        (axum::http::Method::GET, "/short/1234567890", true),
        (axum::http::Method::GET, "/short/", false),
        (axum::http::Method::GET, "/short/12345678901", false),
        (axum::http::Method::GET, "/short/abc_def", false),
        (axum::http::Method::POST, "/short/abc", false),
        (axum::http::Method::GET, "/b9K2x1", false),
        (axum::http::Method::GET, "/", false),
        (axum::http::Method::GET, "/index.html", false),
        (axum::http::Method::GET, "/foo", false),
        (axum::http::Method::GET, "/swagger-ui.html", true),
        (axum::http::Method::GET, "/swagger-ui/swagger-ui.css", true),
        (axum::http::Method::GET, "/swagger-ui", false),
        (axum::http::Method::GET, "/v3/api-docs", true),
        (axum::http::Method::GET, "/v3/api-docs.yaml", true),
        (axum::http::Method::GET, "/v3/api-docs/extra", true),
        (axum::http::Method::PUT, "/v3/api-docs", false),
        (axum::http::Method::GET, "/actuator/health", false),
        (axum::http::Method::GET, "/metrics", false),
    ];
    for (method, path, allowed) in cases {
        assert_eq!(LegalPaths::allows(&method, path), allowed, "{method} {path}");
    }
    assert!(PublicAccess::is_documented("/api/v1/shorten"));
    assert!(PublicAccess::is_documented("/short/abc"));
    assert!(PublicAccess::is_documented("/swagger-ui.html"));
    assert!(PublicAccess::is_documented("/swagger-ui/swagger-ui.css"));
    assert!(PublicAccess::is_documented("/v3/api-docs"));
    assert!(PublicAccess::is_documented("/v3/api-docs.yaml"));
    assert!(PublicAccess::is_documented("/v3/api-docs/extra"));
    assert!(!PublicAccess::is_documented("/"));
    assert!(!PublicAccess::is_documented("/nope"));
    assert!(!HtmlSanitizer::looks_unsafe("https://example.com/path"));
    assert!(!HtmlSanitizer::looks_unsafe(""));
    assert!(!HtmlSanitizer::looks_unsafe("   "));
    assert!(HtmlSanitizer::looks_unsafe("https://example.com/<script>alert(1)</script>"));
    assert!(HtmlSanitizer::looks_unsafe("<img src=x onerror=alert(1)>"));
    assert!(HtmlSanitizer::looks_unsafe("<b>bold</b>"));
    assert!(inspects(
        &axum::http::Method::POST,
        "/api/v1/shorten",
        Some("application/json")
    ));
    assert!(!inspects(&axum::http::Method::GET, "/api/v1/shorten", Some("application/json")));
    assert!(!inspects(&axum::http::Method::POST, "/short/abc", Some("application/json")));
    assert!(!inspects(&axum::http::Method::POST, "/api/v1/shorten", None));
    assert!(!inspects(&axum::http::Method::POST, "/api/v1/shorten", Some("text/plain")));
    assert!(finding_in_body("").is_none());
    assert!(finding_in_body("not-json").is_none());
    assert!(finding_in_body("{}").is_none());
    assert!(finding_in_body(r#"{"longUrl":1}"#).is_none());
    assert!(finding_in_body(r#"{"longUrl":"https://example.com/path"}"#).is_none());
    assert_eq!(
        finding_in_body(r#"{"longUrl":"<script>alert(1)</script>"}"#)
            .unwrap()
            .to_string(),
        "longUrl contains disallowed HTML"
    );

    let now = chrono::Utc::now();
    let service = UrlShortenerService::new(StubRepo, "http://localhost:8080");
    let missing = service.create("", None, now).await.unwrap_err();
    assert_eq!(missing.to_string(), "longUrl is required");
    let script = service
        .create("javascript:alert(1)", None, now)
        .await
        .unwrap_err();
    assert_eq!(script.to_string(), "longUrl contains a disallowed payload");
    let ttl = service
        .create("https://example.com/ttl", Some(0), now)
        .await
        .unwrap_err();
    assert_eq!(ttl.to_string(), "ttlInDays must be between 1 and 3650");
    let created = service
        .create("https://example.com/from-helper", Some(10), now)
        .await
        .unwrap();
    assert!(created.short_url.starts_with("http://localhost:8080/short/"));
    let response = redirect(
        axum::extract::State(UrlShortenerService::new(StubRepo, "http://localhost:8080")),
        axum::extract::Path("missing".to_string()),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let unavailable = redirect(
        axum::extract::State(UrlShortenerService::new(StubRepo, "http://localhost:8080")),
        axum::extract::Path("down".to_string()),
    )
    .await;
    assert_eq!(unavailable.status(), StatusCode::SERVICE_UNAVAILABLE);

    let repo = StubRepo;
    let handle = cleanup::spawn_with_interval(repo, Duration::from_millis(5));
    tokio::time::sleep(Duration::from_millis(40)).await;
    handle.abort();
}

#[tokio::test]
async fn legal_paths_return_the_shared_html_page() {
    let _guard = GATE.lock().await;
    let payload = r#"{"longUrl":"https://example.com/legal-path"}"#;
    let (created_status, _, created_body) = call("POST", "/api/v1/shorten", Some(payload)).await;
    assert_eq!(created_status, StatusCode::CREATED, "{}", String::from_utf8_lossy(&created_body));
    let created: Value = serde_json::from_slice(&created_body).unwrap();
    assert!(created["shortCode"].is_string());
    let code = created["shortCode"].as_str().unwrap();

    let (redirect_status, redirect_headers, _) = call("GET", &format!("/short/{code}"), None).await;
    assert_eq!(redirect_status, StatusCode::FOUND);
    assert_eq!(
        redirect_headers.get("location").unwrap().to_str().unwrap(),
        "https://example.com/legal-path"
    );

    let (docs_status, _, docs_body) = call("GET", "/v3/api-docs", None).await;
    assert_eq!(docs_status, StatusCode::OK);
    let docs = String::from_utf8(docs_body).unwrap();
    assert!(docs.contains("/api/v1/shorten"));
    assert!(!docs.contains("/{shortCode}"));

    for path in [
        "/short/unknown",
        "/b9K2x1",
        "/",
        "/nope",
        "/index.html",
        "/foo",
        "/api/v1/shorten",
        "/short/",
        "/short/abc_def",
        "/short/12345678901",
        "/swagger-ui",
        "/actuator/health",
        "/metrics",
        "/v3/api-docs/extra",
    ] {
        let (status, headers, body) = call("GET", path, None).await;
        assert_friendly(status, &headers, &body, path);
    }

    let (post_root, post_headers, post_body) = call("POST", "/", Some("{}")).await;
    assert_friendly(post_root, &post_headers, &post_body, "/");
}

fn assert_friendly(status: StatusCode, headers: &axum::http::HeaderMap, body: &[u8], path: &str) {
    assert_eq!(status, StatusCode::NOT_FOUND, "{path}: {}", String::from_utf8_lossy(body));
    let content_type = headers.get("content-type").unwrap().to_str().unwrap();
    assert!(content_type.starts_with("text/html"), "{path}: {content_type}");
    let html = String::from_utf8(body.to_vec()).unwrap();
    assert!(html.contains("<h1>Link not found</h1>"), "{path}: {html}");
    assert!(html.contains("This short link is missing or has expired."), "{path}: {html}");
    assert!(html.contains(&format!("<p>{path}</p>")), "{path}: {html}");
    assert!(html.contains("POST /api/v1/shorten"), "{path}: {html}");
    assert!(html.contains("/swagger-ui.html"), "{path}: {html}");
    assert!(html.contains("/short/{code}"), "{path}: {html}");
    assert!(!html.contains("Whitelabel"), "{path}: {html}");
    assert!(!html.contains("Method Not Allowed"), "{path}: {html}");
    assert!(!html.contains("404 Not Found"), "{path}: {html}");
    assert!(!html.contains("stack trace"), "{path}: {html}");
    assert!(!html.starts_with('{'), "{path}: {html}");
}

#[tokio::test]
async fn public_routes_accept_callers_and_html_is_rejected() {
    let _guard = GATE.lock().await;
    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/shorten")
        .header("content-type", "application/json")
        .header("authorization", "Bearer demo")
        .body(Body::from(
            r#"{"longUrl":"https://example.com/public-access"}"#.to_string(),
        ))
        .unwrap();
    let response = app().await.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    assert!(response.headers().get("www-authenticate").is_none());

    for payload in [
        r#"{"longUrl":"https://example.com/<script>alert(1)</script>"}"#,
        r#"{"longUrl":"<script>alert(1)</script>"}"#,
        r#"{"longUrl":"<img src=x onerror=alert(1)>"}"#,
        r#"{"longUrl":"<b>bold</b>"}"#,
    ] {
        let (status, headers, body) = call("POST", "/api/v1/shorten", Some(payload)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{}", String::from_utf8_lossy(&body));
        assert!(headers.get("www-authenticate").is_none());
        let error: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(error["status"], 400);
        assert_eq!(error["error"], "Bad Request");
        assert!(error.get("shortCode").unwrap().is_null());
        let message = error["message"].as_str().unwrap();
        assert!(
            message.to_ascii_lowercase().contains("html") || message.to_ascii_lowercase().contains("sanit"),
            "{message}"
        );
    }

    let (missing, missing_headers, _) = call("GET", "/nope", None).await;
    assert_eq!(missing, StatusCode::NOT_FOUND);
    assert_ne!(missing, StatusCode::UNAUTHORIZED);
    assert!(missing_headers.get("www-authenticate").is_none());
}

#[tokio::test]
async fn swagger_lists_only_the_create_api() {
    let _guard = GATE.lock().await;
    let (html_status, html_headers, html_body) = call("GET", "/swagger-ui.html", None).await;
    assert_eq!(html_status, StatusCode::OK);
    assert!(html_headers
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap()
        .contains("text/html"));
    let html = String::from_utf8(html_body).unwrap();
    assert!(html.contains("URL Shortener API"));
    assert!(html.contains("0.1.0"));

    let (json_status, _, json_body) = call("GET", "/v3/api-docs", None).await;
    assert_eq!(json_status, StatusCode::OK);
    let doc: Value = serde_json::from_slice(&json_body).unwrap();
    assert!(doc["openapi"].as_str().unwrap().starts_with('3'));
    assert_eq!(doc["info"]["title"], "URL Shortener API");
    assert_eq!(doc["info"]["version"], "0.1.0");
    assert_eq!(doc["servers"][0]["url"], "http://localhost:8080");
    let paths = doc["paths"].as_object().unwrap();
    assert!(paths.contains_key("/api/v1/shorten"));
    assert!(paths.keys().all(|path| !path.starts_with("/short")));
    assert!(doc["components"].get("securitySchemes").is_none());
    assert!(doc["paths"]["/api/v1/shorten"]["post"]["responses"]
        .get("201")
        .is_some());
    assert!(doc["paths"]["/api/v1/shorten"]["post"]["responses"]
        .get("400")
        .is_some());
    assert!(doc["paths"]["/api/v1/shorten"]["post"]["responses"]
        .get("503")
        .is_some());

    let (yaml_status, yaml_headers, yaml_body) = call("GET", "/v3/api-docs.yaml", None).await;
    assert_eq!(yaml_status, StatusCode::OK);
    assert!(yaml_headers
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap()
        .contains("yaml"));
    let yaml = String::from_utf8(yaml_body).unwrap();
    assert!(yaml.contains("URL Shortener API"));
    assert!(yaml.contains("/api/v1/shorten"));
    assert!(!yaml.contains("/short/{shortCode}"));

    let (asset_status, _, _) = call("GET", "/swagger-ui/swagger-ui.css", None).await;
    assert_eq!(asset_status, StatusCode::OK);
}

#[derive(Clone)]
struct StubRepo;

impl UrlMappingRepository for StubRepo {
    async fn find_by_long_url_prefix(&self, _prefix: &str) -> Result<Vec<UrlMapping>, AppError> {
        Ok(Vec::new())
    }

    async fn find_by_short_code(&self, short_code: &str) -> Result<Option<UrlMapping>, AppError> {
        if short_code == "down" {
            Err(AppError::Unavailable("down".to_string()))
        } else {
            Ok(None)
        }
    }

    async fn insert_with_encoded_id(
        &self,
        long_url: &str,
        _placeholder_code: &str,
        created_at: chrono::DateTime<chrono::Utc>,
        expires_at: chrono::DateTime<chrono::Utc>,
        encode_id: fn(u64) -> String,
    ) -> Result<UrlMapping, AppError> {
        Ok(UrlMapping {
            id: 56_800_235,
            long_url: long_url.to_string(),
            short_code: encode_id(56_800_235),
            created_at,
            expires_at,
        })
    }

    async fn delete_expired_before(
        &self,
        _cutoff: chrono::DateTime<chrono::Utc>,
        _limit: i64,
    ) -> Result<u64, AppError> {
        Ok(2)
    }
}

async fn seed_expired(code: &str, long_url: &str) {
    let db = database().await.lock().await;
    let config = test_config(db.port).await;
    let pool = schema::connect(&config).await.unwrap();
    sqlx::query(
        "INSERT INTO url_mappings (long_url, short_code, created_at, expires_at) VALUES (?, ?, UTC_TIMESTAMP() - INTERVAL 2 DAY, UTC_TIMESTAMP() - INTERVAL 1 DAY)",
    )
    .bind(long_url)
    .bind(code)
    .execute(&pool)
    .await
    .unwrap();
}
