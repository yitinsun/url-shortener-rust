use chrono::{DateTime, Duration, Utc};
use url::Url;

use crate::encoding::base62;
use crate::error::AppError;
use crate::repo::url_mapping::{UrlMapping, UrlMappingRepository};
use crate::web::dto::ShortenResponse;

const MAX_URL_LENGTH: usize = 2048;
const DEFAULT_TTL_DAYS: i64 = 730;
const MIN_TTL_DAYS: i64 = 1;
const MAX_TTL_DAYS: i64 = 3650;

#[derive(Clone)]
pub struct UrlShortenerService<R> {
    pub(crate) repo: R,
    base_url: String,
}

impl<R> UrlShortenerService<R> {
    pub fn new(repo: R, base_url: impl Into<String>) -> Self {
        Self {
            repo,
            base_url: base_url.into().trim_end_matches('/').to_string(),
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

impl<R: UrlMappingRepository> UrlShortenerService<R> {
    pub async fn create(
        &self,
        long_url: &str,
        ttl_in_days: Option<i64>,
        now: DateTime<Utc>,
    ) -> Result<ShortenResponse, AppError> {
        let long_url = validate_long_url(long_url, &self.base_url)?;
        let ttl_days = resolve_ttl(ttl_in_days)?;
        let prefix = prefix_of(&long_url);
        let existing = self
            .repo
            .find_by_long_url_prefix(&prefix)
            .await?
            .into_iter()
            .find(|row| row.long_url == long_url && row.expires_at > now);
        if let Some(mapping) = existing {
            return Ok(self.to_response(mapping));
        }
        let expires_at = now + Duration::days(ttl_days);
        let mapping = self
            .repo
            .insert_with_encoded_id(&long_url, &placeholder_code(), now, expires_at, base62::encode)
            .await?;
        Ok(self.to_response(mapping))
    }

    pub async fn resolve(&self, short_code: &str, now: DateTime<Utc>) -> Result<String, AppError> {
        let Some(mapping) = self.repo.find_by_short_code(short_code).await? else {
            return Err(AppError::NotFound);
        };
        if mapping.expires_at <= now {
            return Err(AppError::NotFound);
        }
        Ok(mapping.long_url)
    }

    fn to_response(&self, mapping: UrlMapping) -> ShortenResponse {
        ShortenResponse {
            long_url: mapping.long_url,
            short_url: format!("{}/short/{}", self.base_url, mapping.short_code),
            short_code: mapping.short_code,
            expires_at: mapping.expires_at,
        }
    }
}

fn validate_long_url(long_url: &str, base_url: &str) -> Result<String, AppError> {
    if long_url.trim().is_empty() {
        return Err(AppError::BadRequest("longUrl is required".to_string()));
    }
    let trimmed = long_url.trim().to_string();
    if trimmed.len() > MAX_URL_LENGTH {
        return Err(AppError::BadRequest("longUrl exceeds 2048 characters".to_string()));
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.contains("<script") || lower.starts_with("javascript:") || lower.starts_with("data:") {
        return Err(AppError::BadRequest(
            "longUrl contains a disallowed payload".to_string(),
        ));
    }
    let parsed = Url::parse(&trimmed).map_err(|_| AppError::BadRequest("longUrl is not a valid URI".to_string()))?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err(AppError::BadRequest("longUrl must be HTTP or HTTPS".to_string()));
    }
    let host = parsed
        .host_str()
        .filter(|host| !host.is_empty())
        .ok_or_else(|| AppError::BadRequest("longUrl must include a host".to_string()))?;
    let base = Url::parse(base_url).map_err(|_| AppError::Unavailable("APP_BASE_URL is invalid".to_string()))?;
    if let Some(app_host) = base.host_str() {
        if host.eq_ignore_ascii_case(app_host) {
            return Err(AppError::BadRequest(
                "longUrl must not point at this service".to_string(),
            ));
        }
    }
    Ok(trimmed)
}

fn resolve_ttl(ttl_in_days: Option<i64>) -> Result<i64, AppError> {
    match ttl_in_days {
        None => Ok(DEFAULT_TTL_DAYS),
        Some(days) if (MIN_TTL_DAYS..=MAX_TTL_DAYS).contains(&days) => Ok(days),
        Some(_) => Err(AppError::BadRequest(
            "ttlInDays must be between 1 and 3650".to_string(),
        )),
    }
}

fn prefix_of(long_url: &str) -> String {
    long_url.chars().take(255).collect()
}

fn placeholder_code() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let mixed = n.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let mut code = base62::encode(mixed);
    code.truncate(10);
    if code.len() < 10 {
        code = format!("{code:0>10}");
    }
    code
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use chrono::{TimeZone, Utc};

    use super::UrlShortenerService;
    use crate::error::AppError;
    use crate::repo::url_mapping::{UrlMapping, UrlMappingRepository};

    #[derive(Default)]
    struct MemRepo {
        rows: Mutex<Vec<UrlMapping>>,
        inserts: Mutex<u32>,
        next_id: Mutex<i64>,
    }

    impl MemRepo {
        fn with_row(row: UrlMapping) -> Self {
            let next = row.id + 1;
            Self {
                rows: Mutex::new(vec![row]),
                inserts: Mutex::new(0),
                next_id: Mutex::new(next),
            }
        }
    }

    impl UrlMappingRepository for MemRepo {
        async fn find_by_long_url_prefix(&self, prefix: &str) -> Result<Vec<UrlMapping>, AppError> {
            Ok(self
                .rows
                .lock()
                .unwrap()
                .iter()
                .filter(|row| row.long_url.chars().take(255).collect::<String>() == prefix)
                .cloned()
                .collect())
        }

        async fn find_by_short_code(&self, short_code: &str) -> Result<Option<UrlMapping>, AppError> {
            Ok(self
                .rows
                .lock()
                .unwrap()
                .iter()
                .find(|row| row.short_code == short_code)
                .cloned())
        }

        async fn insert_with_encoded_id(
            &self,
            long_url: &str,
            _placeholder_code: &str,
            created_at: chrono::DateTime<Utc>,
            expires_at: chrono::DateTime<Utc>,
            encode_id: fn(u64) -> String,
        ) -> Result<UrlMapping, AppError> {
            *self.inserts.lock().unwrap() += 1;
            let mut next = self.next_id.lock().unwrap();
            if *next == 0 {
                *next = 56_800_235;
            }
            let id = *next;
            *next += 1;
            let mapping = UrlMapping {
                id,
                long_url: long_url.to_string(),
                short_code: encode_id(id as u64),
                created_at,
                expires_at,
            };
            self.rows.lock().unwrap().push(mapping.clone());
            Ok(mapping)
        }

        async fn delete_expired_before(
            &self,
            _cutoff: chrono::DateTime<Utc>,
            _limit: i64,
        ) -> Result<u64, AppError> {
            Ok(0)
        }
    }

    fn now() -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 11, 16, 12, 0, 0).unwrap()
    }

    fn service(repo: MemRepo) -> UrlShortenerService<MemRepo> {
        UrlShortenerService::new(repo, "http://localhost:8080")
    }

    #[tokio::test]
    async fn missing_short_code_is_not_found() {
        let service = service(MemRepo::default());
        let err = service.resolve("missing", now()).await.unwrap_err();
        assert!(matches!(err, AppError::NotFound));
    }

    #[tokio::test]
    async fn expired_mapping_is_not_found() {
        let expired = UrlMapping {
            id: 56_800_235,
            long_url: "https://example.com/old".to_string(),
            short_code: "abcde".to_string(),
            created_at: now() - chrono::Duration::days(10),
            expires_at: now() - chrono::Duration::seconds(1),
        };
        let service = service(MemRepo::with_row(expired));
        let err = service.resolve("abcde", now()).await.unwrap_err();
        assert!(matches!(err, AppError::NotFound));
    }

    #[tokio::test]
    async fn active_long_url_does_not_insert_again() {
        let repo = MemRepo::default();
        let service = service(repo);
        let first = service
            .create("https://example.com/same", Some(10), now())
            .await
            .unwrap();
        let second = service
            .create("https://example.com/same", Some(99), now())
            .await
            .unwrap();
        assert_eq!(first.short_code, second.short_code);
        assert_eq!(*service.repo.inserts.lock().unwrap(), 1);
    }

    #[tokio::test]
    async fn omitted_ttl_is_730_days() {
        let service = service(MemRepo::default());
        let created = service
            .create("https://example.com/default-ttl", None, now())
            .await
            .unwrap();
        let expected = now() + chrono::Duration::days(730);
        assert_eq!(created.expires_at, expected);
    }

    #[tokio::test]
    async fn self_loop_is_rejected() {
        let service = service(MemRepo::default());
        let err = service
            .create("http://localhost:8080/own", None, now())
            .await
            .unwrap_err();
        assert!(err.is_bad_request());
        assert_eq!(*service.repo.inserts.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn oversize_url_is_rejected() {
        let service = service(MemRepo::default());
        let url = format!("https://example.com/{}", "a".repeat(2048));
        assert!(url.len() > 2048);
        let err = service.create(&url, None, now()).await.unwrap_err();
        assert!(err.is_bad_request());
    }

    #[tokio::test]
    async fn validation_messages_match_the_contract() {
        let service = service(MemRepo::default());
        let cases = [
            ("", "longUrl is required"),
            ("   ", "longUrl is required"),
            ("javascript:alert(1)", "longUrl contains a disallowed payload"),
            ("data:text/html,hi", "longUrl contains a disallowed payload"),
            ("https://example.com/<script>", "longUrl contains a disallowed payload"),
            ("not a url", "longUrl is not a valid URI"),
            ("ftp://example.com/file", "longUrl must be HTTP or HTTPS"),
        ];
        for (input, message) in cases {
            let err = service.create(input, None, now()).await.unwrap_err();
            assert_eq!(err.to_string(), message, "{input}");
        }
        let low = service.create("https://example.com/ttl", Some(0), now()).await.unwrap_err();
        assert_eq!(low.to_string(), "ttlInDays must be between 1 and 3650");
        let high = service
            .create("https://example.com/ttl", Some(3651), now())
            .await
            .unwrap_err();
        assert_eq!(high.to_string(), "ttlInDays must be between 1 and 3650");
        let edge = service
            .create("https://example.com/edge", Some(1), now())
            .await
            .unwrap();
        assert_eq!(edge.expires_at, now() + chrono::Duration::days(1));
    }

    #[tokio::test]
    async fn expired_long_url_is_inserted_again() {
        let expired = UrlMapping {
            id: 10,
            long_url: "https://example.com/again".to_string(),
            short_code: "old".to_string(),
            created_at: now() - chrono::Duration::days(3),
            expires_at: now() - chrono::Duration::seconds(1),
        };
        let service = service(MemRepo::with_row(expired));
        let created = service
            .create("https://example.com/again", Some(3650), now())
            .await
            .unwrap();
        assert_ne!(created.short_code, "old");
        assert_eq!(*service.repo.inserts.lock().unwrap(), 1);
        assert!(service.base_url().starts_with("http://localhost:8080"));
    }

    #[tokio::test]
    async fn repository_failures_surface_as_unavailable() {
        let service = UrlShortenerService::new(FailingRepo, "http://localhost:8080/");
        let err = service
            .create("https://example.com/down", None, now())
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Unavailable(_)));
        let resolve = service.resolve("code", now()).await.unwrap_err();
        assert!(matches!(resolve, AppError::Unavailable(_)));
        let bad_base = UrlShortenerService::new(MemRepo::default(), "://bad");
        let base_err = bad_base
            .create("https://example.com/ok", None, now())
            .await
            .unwrap_err();
        assert!(matches!(base_err, AppError::Unavailable(_)));
    }

    struct FailingRepo;

    impl UrlMappingRepository for FailingRepo {
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
            _created_at: chrono::DateTime<Utc>,
            _expires_at: chrono::DateTime<Utc>,
            _encode_id: fn(u64) -> String,
        ) -> Result<UrlMapping, AppError> {
            Err(AppError::Unavailable("down".to_string()))
        }

        async fn delete_expired_before(
            &self,
            _cutoff: chrono::DateTime<Utc>,
            _limit: i64,
        ) -> Result<u64, AppError> {
            Err(AppError::Unavailable("down".to_string()))
        }
    }
}
