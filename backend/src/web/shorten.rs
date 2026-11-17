use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use chrono::Utc;

use crate::error::{AppError, ErrorResponse};
use crate::repo::url_mapping::UrlMappingRepository;
use crate::service::url_shortener::UrlShortenerService;
use crate::web::dto::{ShortenRequest, ShortenResponse};

#[utoipa::path(
    post,
    path = "/api/v1/shorten",
    operation_id = "shorten",
    tag = "Shorten",
    summary = "Create a short URL",
    request_body = ShortenRequest,
    responses(
        (status = 201, description = "Mapping created, or an existing active mapping returned", body = ShortenResponse),
        (status = 400, description = "Invalid body (URL, TTL, script payload, or self-loop)", body = ErrorResponse),
        (status = 503, description = "Database unavailable", body = ErrorResponse)
    )
)]
pub async fn shorten<R>(
    State(service): State<UrlShortenerService<R>>,
    Json(body): Json<ShortenRequest>,
) -> Result<(StatusCode, Json<ShortenResponse>), AppError>
where
    R: UrlMappingRepository + Clone + Send + Sync + 'static,
{
    let created = service
        .create(&body.long_url, body.ttl_in_days, Utc::now())
        .await?;
    Ok((StatusCode::CREATED, Json(created)))
}
