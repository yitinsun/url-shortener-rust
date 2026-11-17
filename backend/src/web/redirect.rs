use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use chrono::Utc;

use crate::error::{friendly_not_found, AppError};
use crate::repo::url_mapping::UrlMappingRepository;
use crate::service::url_shortener::UrlShortenerService;

pub async fn redirect<R>(
    State(service): State<UrlShortenerService<R>>,
    Path(short_code): Path<String>,
) -> Response
where
    R: UrlMappingRepository + Clone + Send + Sync + 'static,
{
    match service.resolve(&short_code, Utc::now()).await {
        Ok(long_url) => (
            StatusCode::FOUND,
            [(header::LOCATION, long_url)],
        )
            .into_response(),
        Err(AppError::NotFound) => friendly_not_found(&format!("/short/{short_code}")),
        Err(other) => other.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::redirect;
    use crate::error::AppError;
    use crate::repo::url_mapping::{UrlMapping, UrlMappingRepository};
    use crate::service::url_shortener::UrlShortenerService;
    use axum::extract::{Path, State};
    use axum::http::StatusCode;
    use chrono::{DateTime, Utc};

    #[derive(Clone)]
    struct Down;

    impl UrlMappingRepository for Down {
        async fn find_by_long_url_prefix(&self, _prefix: &str) -> Result<Vec<UrlMapping>, AppError> {
            Err(AppError::Unavailable("down".to_string()))
        }

        async fn find_by_short_code(&self, _short_code: &str) -> Result<Option<UrlMapping>, AppError> {
            Err(AppError::Unavailable("down".to_string()))
        }

        async fn insert_with_encoded_id(
            &self,
            _long_url: &str,
            _placeholder_code: &str,
            _created_at: DateTime<Utc>,
            _expires_at: DateTime<Utc>,
            _encode_id: fn(u64) -> String,
        ) -> Result<UrlMapping, AppError> {
            Err(AppError::Unavailable("down".to_string()))
        }

        async fn delete_expired_before(&self, _cutoff: DateTime<Utc>, _limit: i64) -> Result<u64, AppError> {
            Err(AppError::Unavailable("down".to_string()))
        }
    }

    #[tokio::test]
    async fn database_error_becomes_service_unavailable() {
        let response = redirect(
            State(UrlShortenerService::new(Down, "http://localhost:8080")),
            Path("abc".to_string()),
        )
        .await;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}
