use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ShortenRequest {
    pub long_url: String,
    pub ttl_in_days: Option<i64>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ShortenResponse {
    pub long_url: String,
    pub short_url: String,
    pub short_code: String,
    pub expires_at: DateTime<Utc>,
}
