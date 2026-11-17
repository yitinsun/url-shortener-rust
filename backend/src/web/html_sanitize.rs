use axum::body::Body;
use axum::extract::Request;
use axum::http::{header, Method};
use axum::middleware::{from_fn, Next};
use axum::response::{IntoResponse, Response};
use axum::Router;

use crate::error::AppError;
use crate::web::html_sanitizer::HtmlSanitizer;

/// Runs after [`crate::web::legal_path::LegalPathLayer`] and rejects HTML in `longUrl`.
#[derive(Clone, Copy, Debug, Default)]
pub struct HtmlSanitizeLayer;

impl HtmlSanitizeLayer {
    pub fn new() -> Self {
        Self
    }

    pub fn apply<S>(self, router: Router<S>) -> Router<S>
    where
        S: Clone + Send + Sync + 'static,
    {
        router.layer(from_fn(sanitize_long_url))
    }
}

pub fn inspects(method: &Method, path: &str, content_type: Option<&str>) -> bool {
    method == Method::POST
        && path == "/api/v1/shorten"
        && content_type
            .unwrap_or("")
            .to_ascii_lowercase()
            .contains("application/json")
}

pub fn finding_in_body(body: &str) -> Option<AppError> {
    if body.trim().is_empty() {
        return None;
    }
    let Ok(root) = serde_json::from_str::<serde_json::Value>(body) else {
        return None;
    };
    let Some(long_url) = root.get("longUrl").and_then(serde_json::Value::as_str) else {
        return None;
    };
    if HtmlSanitizer::looks_unsafe(long_url) {
        Some(AppError::disallowed_html())
    } else {
        None
    }
}

async fn sanitize_long_url(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let path = request.uri().path().to_owned();
    let content_type = request
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    if !inspects(&method, &path, content_type.as_deref()) {
        return next.run(request).await;
    }

    let (parts, body) = request.into_parts();
    let Ok(bytes) = axum::body::to_bytes(body, 1024 * 1024).await else {
        return next.run(Request::from_parts(parts, Body::empty())).await;
    };
    let text = String::from_utf8_lossy(&bytes);
    if let Some(error) = finding_in_body(&text) {
        return error.into_response();
    }
    next.run(Request::from_parts(parts, Body::from(bytes))).await
}

#[cfg(test)]
mod tests {
    use super::{finding_in_body, inspects, HtmlSanitizeLayer};
    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode};
    use axum::routing::post;
    use axum::Json;
    use axum::Router;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    #[test]
    fn inspection_follows_the_post_json_contract() {
        assert!(inspects(
            &Method::POST,
            "/api/v1/shorten",
            Some("application/json; charset=utf-8")
        ));
        assert!(inspects(&Method::POST, "/api/v1/shorten", Some("APPLICATION/JSON")));
        assert!(!inspects(&Method::GET, "/api/v1/shorten", Some("application/json")));
        assert!(!inspects(&Method::POST, "/short/abc", Some("application/json")));
        assert!(!inspects(&Method::POST, "/api/v1/shorten", None));
        assert!(!inspects(&Method::POST, "/api/v1/shorten", Some("text/plain")));
        assert!(finding_in_body("").is_none());
        assert!(finding_in_body("   ").is_none());
        assert!(finding_in_body("not-json").is_none());
        assert!(finding_in_body("{}").is_none());
        assert!(finding_in_body(r#"{"longUrl":1}"#).is_none());
        assert!(finding_in_body(r#"{"longUrl":"https://example.com/path"}"#).is_none());
        let finding = finding_in_body(r#"{"longUrl":"<img src=x>"}"#).unwrap();
        assert_eq!(finding.to_string(), "longUrl contains disallowed HTML");
    }

    #[tokio::test]
    async fn layer_rejects_markup_and_keeps_a_plain_url() {
        async fn reached(Json(body): Json<serde_json::Value>) -> String {
            body["longUrl"].as_str().unwrap_or("").to_string()
        }

        let app = HtmlSanitizeLayer::new().apply(Router::new().route("/api/v1/shorten", post(reached)));

        let clean = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/shorten")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"longUrl":"https://example.com/path"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(clean.status(), StatusCode::OK);

        let markup = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/shorten")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"longUrl":"<script>alert(1)</script>"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(markup.status(), StatusCode::BAD_REQUEST);
        let html = String::from_utf8(markup.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap();
        assert!(html.contains("disallowed HTML"));
        assert!(html.contains("\"status\":400"));
    }
}
