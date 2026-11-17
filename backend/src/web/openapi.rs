use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse};
use axum::Router;
use utoipa::OpenApi;

use crate::error::ErrorResponse;
use crate::web::dto::{ShortenRequest, ShortenResponse};

#[derive(OpenApi)]
#[openapi(
    info(title = "URL Shortener API", version = "0.1.0"),
    paths(crate::web::shorten::shorten),
    components(schemas(ShortenRequest, ShortenResponse, ErrorResponse)),
    tags((name = "Shorten", description = "Create a short URL"))
)]
struct ApiDoc;

pub fn document(base_url: &str) -> utoipa::openapi::OpenApi {
    let mut doc = ApiDoc::openapi();
    doc.servers = Some(vec![utoipa::openapi::Server::new(base_url)]);
    doc
}

pub fn routes<S>(base_url: impl Into<String>) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    let doc = document(&base_url.into());
    let yaml = doc.to_yaml().unwrap_or_else(|err| format!("openapi yaml failed: {err}"));
    Router::new()
        .route("/swagger-ui.html", axum::routing::get(swagger_html))
        .route(
            "/v3/api-docs.yaml",
            axum::routing::get(move || {
                let yaml = yaml.clone();
                async move {
                    (
                        StatusCode::OK,
                        [(header::CONTENT_TYPE, "application/yaml; charset=utf-8")],
                        yaml,
                    )
                }
            }),
        )
        .merge(Router::<S>::from(
            utoipa_swagger_ui::SwaggerUi::new("/swagger-ui").url("/v3/api-docs", doc),
        ))
}

async fn swagger_html() -> impl IntoResponse {
    Html(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>URL Shortener API 0.1.0</title>
  <link rel="stylesheet" href="/swagger-ui/swagger-ui.css">
</head>
<body>
  <div id="swagger-ui"></div>
  <script src="/swagger-ui/swagger-ui-bundle.js"></script>
  <script src="/swagger-ui/swagger-ui-standalone-preset.js"></script>
  <script src="/swagger-ui/swagger-initializer.js"></script>
</body>
</html>"#,
    )
}
