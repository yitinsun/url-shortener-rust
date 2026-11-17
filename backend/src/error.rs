use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    BadRequest(String),
    #[error("short link was not found")]
    NotFound,
    #[error("{0}")]
    Unavailable(String),
}

#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponse {
    pub status: u16,
    pub error: String,
    pub message: String,
    pub short_code: Option<String>,
}

impl AppError {
    pub fn is_bad_request(&self) -> bool {
        matches!(self, Self::BadRequest(_))
    }

    pub fn disallowed_html() -> Self {
        AppError::BadRequest("longUrl contains disallowed HTML".to_string())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            AppError::NotFound => friendly_not_found("").into_response(),
            AppError::BadRequest(message) => json_error(StatusCode::BAD_REQUEST, "Bad Request", message),
            AppError::Unavailable(message) => json_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "Service Unavailable",
                message,
            ),
        }
    }
}

fn json_error(status: StatusCode, error: &str, message: String) -> Response {
    let body = ErrorResponse {
        status: status.as_u16(),
        error: error.to_string(),
        message,
        short_code: None,
    };
    (status, Json(body)).into_response()
}

pub fn friendly_not_found(path: &str) -> Response {
    let html = format!(
        r#"<!DOCTYPE html><html lang="en"><head><meta charset="utf-8"><title>Link not found</title></head><body><h1>Link not found</h1><p>This short link is missing or has expired.</p><p>{path}</p><nav><a href="/api/v1/shorten">POST /api/v1/shorten</a> <a href="/swagger-ui.html">/swagger-ui.html</a> <a href="/short/{{code}}">/short/{{code}}</a></nav></body></html>"#,
        path = escape_html(path),
    );
    (
        StatusCode::NOT_FOUND,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        html,
    )
        .into_response()
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::{friendly_not_found, AppError};
    use axum::response::IntoResponse;

    #[test]
    fn each_error_maps_to_its_status() {
        let bad = AppError::BadRequest("longUrl is required".to_string());
        assert!(bad.is_bad_request());
        assert_eq!(bad.into_response().status(), axum::http::StatusCode::BAD_REQUEST);
        assert!(!AppError::NotFound.is_bad_request());
        assert_eq!(
            AppError::NotFound.into_response().status(),
            axum::http::StatusCode::NOT_FOUND
        );
        assert_eq!(
            AppError::Unavailable("down".to_string()).into_response().status(),
            axum::http::StatusCode::SERVICE_UNAVAILABLE
        );
        let html = friendly_not_found(r#"a&b<c>"quoted""#);
        assert_eq!(html.status(), axum::http::StatusCode::NOT_FOUND);
        assert!(format!("{html:?}").contains("Response"));
    }
}
