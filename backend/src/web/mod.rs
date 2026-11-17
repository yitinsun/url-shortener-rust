use axum::routing::{get, post};
use axum::Router;

use crate::repo::url_mapping::UrlMappingRepository;
use crate::service::url_shortener::UrlShortenerService;

pub mod dto;
pub mod html_sanitize;
pub mod html_sanitizer;
pub mod legal_path;
pub mod openapi;
pub mod public_access;
pub mod redirect;
mod shorten;

pub fn router<R>(service: UrlShortenerService<R>) -> Router
where
    R: UrlMappingRepository + Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/api/v1/shorten", post(shorten::shorten::<R>))
        .route("/short/{short_code}", get(redirect::redirect::<R>))
        .with_state(service)
}

pub fn with_openapi<S>(router: Router<S>, base_url: &str) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    router.merge(openapi::routes(base_url))
}

pub fn application<R>(service: UrlShortenerService<R>, openapi_base: Option<&str>) -> Router
where
    R: UrlMappingRepository + Clone + Send + Sync + 'static,
{
    let mut app = router(service);
    if let Some(base) = openapi_base {
        app = with_openapi(app, base);
    }
    let app = html_sanitize::HtmlSanitizeLayer::new().apply(app);
    let app = public_access::PublicAccessLayer::new().apply(app);
    legal_path::LegalPathLayer::new().apply(app)
}
