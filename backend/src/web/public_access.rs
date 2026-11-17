use axum::extract::Request;
use axum::middleware::{from_fn, Next};
use axum::response::Response;
use axum::Router;

/// Documents the public routes. This demo does not issue an authentication challenge.
pub struct PublicAccess;

impl PublicAccess {
    pub fn is_documented(path: &str) -> bool {
        if path == "/api/v1/shorten"
            || path == "/swagger-ui.html"
            || path == "/v3/api-docs"
            || path == "/v3/api-docs.yaml"
        {
            return true;
        }
        path.starts_with("/short/")
            || path.starts_with("/swagger-ui/")
            || path.starts_with("/v3/api-docs/")
    }
}

/// Pass-through layer. CSRF, form login, HTTP basic, and logout are not enabled.
#[derive(Clone, Copy, Debug, Default)]
pub struct PublicAccessLayer;

impl PublicAccessLayer {
    pub fn new() -> Self {
        Self
    }

    pub fn apply<S>(self, router: Router<S>) -> Router<S>
    where
        S: Clone + Send + Sync + 'static,
    {
        router.layer(from_fn(allow_public))
    }
}

async fn allow_public(request: Request, next: Next) -> Response {
    let _documented = PublicAccess::is_documented(request.uri().path());
    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::{PublicAccess, PublicAccessLayer};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::routing::{get, post};
    use axum::Router;
    use tower::ServiceExt;

    #[test]
    fn documented_routes_are_public() {
        assert!(PublicAccess::is_documented("/api/v1/shorten"));
        assert!(PublicAccess::is_documented("/short/abc"));
        assert!(PublicAccess::is_documented("/swagger-ui.html"));
        assert!(PublicAccess::is_documented("/swagger-ui/swagger-ui.css"));
        assert!(PublicAccess::is_documented("/v3/api-docs"));
        assert!(PublicAccess::is_documented("/v3/api-docs.yaml"));
        assert!(PublicAccess::is_documented("/v3/api-docs/extra"));
        assert!(!PublicAccess::is_documented("/"));
        assert!(!PublicAccess::is_documented("/nope"));
        assert!(!PublicAccess::is_documented("/actuator/health"));
    }

    #[tokio::test]
    async fn authorization_header_does_not_challenge() {
        let app = PublicAccessLayer::new().apply(
            Router::new()
                .route("/api/v1/shorten", post(|| async { "created" }))
                .route("/nope", get(|| async { "nope" })),
        );
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/shorten")
                    .header("authorization", "Bearer demo")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers().get("www-authenticate").is_none());
    }
}
