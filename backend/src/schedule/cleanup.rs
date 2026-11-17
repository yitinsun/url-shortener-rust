use std::time::Duration;

pub use crate::repo::url_mapping::DELETE_EXPIRED_SQL;

/// Hourly pass. Each pass deletes at most this many expired rows.
pub const DELETE_BATCH: i64 = 500;

pub const CLEANUP_INTERVAL: Duration = Duration::from_secs(60 * 60);

pub async fn delete_expired<R>(repo: &R, cutoff: chrono::DateTime<chrono::Utc>) -> Result<u64, crate::error::AppError>
where
    R: crate::repo::url_mapping::UrlMappingRepository,
{
    repo.delete_expired_before(cutoff, DELETE_BATCH).await
}

pub fn spawn<R>(repo: R) -> tokio::task::JoinHandle<()>
where
    R: crate::repo::url_mapping::UrlMappingRepository + Clone + Send + Sync + 'static,
{
    spawn_with_interval(repo, CLEANUP_INTERVAL)
}

pub fn spawn_with_interval<R>(repo: R, every: Duration) -> tokio::task::JoinHandle<()>
where
    R: crate::repo::url_mapping::UrlMappingRepository + Clone + Send + Sync + 'static,
{
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(every);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        ticker.tick().await;
        loop {
            ticker.tick().await;
            let cutoff = chrono::Utc::now();
            match delete_expired(&repo, cutoff).await {
                Ok(removed) if removed > 0 => {
                    tracing::info!(removed, "removed expired url mappings");
                }
                Ok(_) => {}
                Err(err) => tracing::error!(error = %err, "expired mapping cleanup failed"),
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use chrono::{DateTime, TimeZone, Utc};

    use super::{delete_expired, spawn_with_interval, DELETE_BATCH, DELETE_EXPIRED_SQL};
    use crate::error::AppError;
    use crate::repo::url_mapping::{UrlMapping, UrlMappingRepository};

    #[derive(Clone, Default)]
    struct RecordingRepo {
        calls: Arc<Mutex<Vec<(DateTime<Utc>, i64)>>>,
        results: Arc<Mutex<Vec<Result<u64, AppError>>>>,
    }

    impl UrlMappingRepository for RecordingRepo {
        async fn find_by_long_url_prefix(&self, _prefix: &str) -> Result<Vec<UrlMapping>, AppError> {
            Ok(Vec::new())
        }

        async fn find_by_short_code(&self, _short_code: &str) -> Result<Option<UrlMapping>, AppError> {
            Ok(None)
        }

        async fn insert_with_encoded_id(
            &self,
            _long_url: &str,
            _placeholder_code: &str,
            _created_at: DateTime<Utc>,
            _expires_at: DateTime<Utc>,
            _encode_id: fn(u64) -> String,
        ) -> Result<UrlMapping, AppError> {
            Err(AppError::Unavailable("unused".to_string()))
        }

        async fn delete_expired_before(&self, cutoff: DateTime<Utc>, limit: i64) -> Result<u64, AppError> {
            self.calls.lock().unwrap().push((cutoff, limit));
            self.results.lock().unwrap().pop().unwrap_or(Ok(0))
        }
    }

    #[tokio::test]
    async fn cleanup_binds_a_cutoff_and_uses_placeholders() {
        assert_eq!(DELETE_EXPIRED_SQL.matches('?').count(), 2);
        assert!(!DELETE_EXPIRED_SQL.contains("format!"));
        assert!(!DELETE_EXPIRED_SQL.contains('+'));
        let cutoff = Utc.with_ymd_and_hms(2026, 11, 16, 0, 0, 0).unwrap();
        let repo = RecordingRepo::default();
        delete_expired(&repo, cutoff).await.unwrap();
        let calls = repo.calls.lock().unwrap();
        assert_eq!(calls.as_slice(), &[(cutoff, DELETE_BATCH)]);
    }

    #[tokio::test]
    async fn spawn_covers_removed_rows_errors_and_a_quiet_pass() {
        let repo = RecordingRepo::default();
        {
            let mut results = repo.results.lock().unwrap();
            results.push(Ok(0));
            results.push(Err(AppError::Unavailable("db down".to_string())));
            results.push(Ok(4));
        }
        let handle = spawn_with_interval(repo.clone(), std::time::Duration::from_millis(5));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while repo.calls.lock().unwrap().len() < 3 && std::time::Instant::now() < deadline {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        handle.abort();
        assert!(repo.calls.lock().unwrap().len() >= 3);
    }
}
