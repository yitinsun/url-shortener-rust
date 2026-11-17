use axum::extract::Request;
use axum::http::{Method, Uri};
use axum::middleware::{from_fn, Next};
use axum::response::Response;
use axum::Router;

use crate::error::friendly_not_found;

/// Allowlist for the public URL shortener. Anything else is a 404.
pub struct LegalPaths;

impl LegalPaths {
    pub fn allows(method: &Method, path: &str) -> bool {
        if method == Method::POST && path == "/api/v1/shorten" {
            return true;
        }
        if method != Method::GET {
            return false;
        }
        if path == "/swagger-ui.html" || path == "/v3/api-docs" || path == "/v3/api-docs.yaml" {
            return true;
        }
        if path.starts_with("/swagger-ui/") || path.starts_with("/v3/api-docs/") {
            return true;
        }
        is_short_code(path)
    }
}

fn is_short_code(path: &str) -> bool {
    let Some(code) = path.strip_prefix("/short/") else {
        return false;
    };
    (1..=10).contains(&code.len()) && code.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

/// Tower layer installed with [`Router::layer`] on the finished router.
#[derive(Clone, Copy, Debug, Default)]
pub struct LegalPathLayer;

impl LegalPathLayer {
    pub fn new() -> Self {
        Self
    }

    /// Legal requests continue into the inner service. Every other request
    /// ends as the shared HTML 404 and never reaches a redirect or JSON handler.
    pub fn apply<S>(self, router: Router<S>) -> Router<S>
    where
        S: Clone + Send + Sync + 'static,
    {
        router.fallback(unmatched).layer(from_fn(enforce))
    }
}

async fn enforce(request: Request, next: Next) -> Response {
    let path = request.uri().path().to_owned();
    if LegalPaths::allows(request.method(), &path) {
        next.run(request).await
    } else {
        friendly_not_found(&path)
    }
}

async fn unmatched(uri: Uri) -> Response {
    friendly_not_found(uri.path())
}

#[cfg(test)]
mod tests {
    use super::{LegalPathLayer, LegalPaths};
    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode};
    use axum::routing::{get, post};
    use axum::Router;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    #[test]
    fn allowlist_matches_the_playbook() {
        let cases = [
            (Method::POST, "/api/v1/shorten", true),
            (Method::GET, "/api/v1/shorten", false),
            (Method::GET, "/short/b9K2x1", true),
            (Method::GET, "/short/0", true),
            (Method::GET, "/short/1234567890", true),
            (Method::GET, "/short/", false),
            (Method::GET, "/short/12345678901", false),
            (Method::GET, "/short/abc_def", false),
            (Method::POST, "/short/abc", false),
            (Method::GET, "/b9K2x1", false),
            (Method::GET, "/", false),
            (Method::GET, "/index.html", false),
            (Method::GET, "/foo", false),
            (Method::GET, "/swagger-ui.html", true),
            (Method::GET, "/swagger-ui/swagger-ui.css", true),
            (Method::GET, "/swagger-ui", false),
            (Method::GET, "/v3/api-docs", true),
            (Method::GET, "/v3/api-docs.yaml", true),
            (Method::GET, "/v3/api-docs/extra", true),
            (Method::PUT, "/v3/api-docs", false),
            (Method::GET, "/actuator/health", false),
            (Method::GET, "/metrics", false),
        ];
        for (method, path, allowed) in cases {
            assert_eq!(
                LegalPaths::allows(&method, path),
                allowed,
                "{method} {path}"
            );
        }
    }

    #[tokio::test]
    async fn layer_blocks_illegal_requests_before_the_handler() {
        async fn reached() -> &'static str {
            "reached"
        }

        let app = LegalPathLayer::new().apply(
            Router::new()
                .route("/api/v1/shorten", post(reached))
                .route("/short/{short_code}", get(reached)),
        );

        let created = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/shorten")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(created.status(), StatusCode::OK);

        let redirect = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/short/abc")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(redirect.status(), StatusCode::OK);

        let wrong_method = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/v1/shorten")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(wrong_method.status(), StatusCode::NOT_FOUND);
        let html = String::from_utf8(
            wrong_method
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .to_vec(),
        )
        .unwrap();
        assert!(html.contains("<h1>Link not found</h1>"));
        assert!(html.contains("<p>/api/v1/shorten</p>"));
        assert!(html.contains("POST /api/v1/shorten"));
        assert!(html.contains("/swagger-ui.html"));
        assert!(html.contains("/short/{code}"));
        assert!(!html.contains("reached"));
        assert!(!html.contains("Method Not Allowed"));

        let unknown = app
            .oneshot(Request::builder().uri("/b9K2x1").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(unknown.status(), StatusCode::NOT_FOUND);
    }
}
