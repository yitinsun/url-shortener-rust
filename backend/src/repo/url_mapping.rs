use chrono::{DateTime, Utc};
use sqlx::MySqlPool;

use crate::error::AppError;

pub const DELETE_EXPIRED_SQL: &str = "DELETE FROM url_mappings WHERE expires_at < ? LIMIT ?";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UrlMapping {
    pub id: i64,
    pub long_url: String,
    pub short_code: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

pub trait UrlMappingRepository {
    fn find_by_long_url_prefix(
        &self,
        prefix: &str,
    ) -> impl std::future::Future<Output = Result<Vec<UrlMapping>, AppError>> + Send;

    fn find_by_short_code(
        &self,
        short_code: &str,
    ) -> impl std::future::Future<Output = Result<Option<UrlMapping>, AppError>> + Send;

    /// Insert a row and set `short_code` from the assigned id in one transaction.
    fn insert_with_encoded_id(
        &self,
        long_url: &str,
        placeholder_code: &str,
        created_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
        encode_id: fn(u64) -> String,
    ) -> impl std::future::Future<Output = Result<UrlMapping, AppError>> + Send;

    fn delete_expired_before(
        &self,
        cutoff: DateTime<Utc>,
        limit: i64,
    ) -> impl std::future::Future<Output = Result<u64, AppError>> + Send;
}

#[derive(Clone)]
pub struct SqlxUrlMappingRepository {
    pool: MySqlPool,
}

impl SqlxUrlMappingRepository {
    pub fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }
}

impl UrlMappingRepository for SqlxUrlMappingRepository {
    async fn find_by_long_url_prefix(&self, prefix: &str) -> Result<Vec<UrlMapping>, AppError> {
        let rows = sqlx::query_as::<_, (i64, String, String, DateTime<Utc>, DateTime<Utc>)>(
            "SELECT id, long_url, short_code, created_at, expires_at FROM url_mappings WHERE LEFT(long_url, 255) = ?",
        )
        .bind(prefix)
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(rows.into_iter().map(row_from_tuple).collect())
    }

    async fn find_by_short_code(&self, short_code: &str) -> Result<Option<UrlMapping>, AppError> {
        let row = sqlx::query_as::<_, (i64, String, String, DateTime<Utc>, DateTime<Utc>)>(
            "SELECT id, long_url, short_code, created_at, expires_at FROM url_mappings WHERE short_code = ?",
        )
        .bind(short_code)
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(row.map(row_from_tuple))
    }

    async fn insert_with_encoded_id(
        &self,
        long_url: &str,
        placeholder_code: &str,
        created_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
        encode_id: fn(u64) -> String,
    ) -> Result<UrlMapping, AppError> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        sqlx::query(
            "INSERT INTO url_mappings (long_url, short_code, created_at, expires_at) VALUES (?, ?, ?, ?)",
        )
        .bind(long_url)
        .bind(placeholder_code)
        .bind(created_at)
        .bind(expires_at)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        let (id,): (u64,) = sqlx::query_as("SELECT LAST_INSERT_ID()")
            .fetch_one(&mut *tx)
            .await
            .map_err(db_err)?;
        let id = i64::try_from(id).map_err(|_| AppError::Unavailable("assigned id does not fit i64".to_string()))?;
        let short_code = encode_id(id as u64);
        sqlx::query("UPDATE url_mappings SET short_code = ? WHERE id = ?")
            .bind(&short_code)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        tx.commit().await.map_err(db_err)?;
        Ok(UrlMapping {
            id,
            long_url: long_url.to_string(),
            short_code,
            created_at,
            expires_at,
        })
    }

    async fn delete_expired_before(&self, cutoff: DateTime<Utc>, limit: i64) -> Result<u64, AppError> {
        let result = sqlx::query(DELETE_EXPIRED_SQL)
            .bind(cutoff)
            .bind(limit)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(result.rows_affected())
    }
}

fn row_from_tuple(
    (id, long_url, short_code, created_at, expires_at): (i64, String, String, DateTime<Utc>, DateTime<Utc>),
) -> UrlMapping {
    UrlMapping {
        id,
        long_url,
        short_code,
        created_at,
        expires_at,
    }
}

fn db_err(err: sqlx::Error) -> AppError {
    AppError::Unavailable(err.to_string())
}
