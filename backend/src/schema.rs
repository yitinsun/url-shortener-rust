use sqlx::mysql::MySqlPoolOptions;
use sqlx::MySqlPool;

use crate::config::Config;
use crate::error::AppError;

const SCHEMA: &str = include_str!("../sql/schema.sql");

pub async fn connect(config: &Config) -> Result<MySqlPool, AppError> {
    if config.apply_schema {
        let bootstrap = pool(&config.bootstrap_url()).await?;
        exec_ignore_exists(&bootstrap, database_statement()).await?;
        bootstrap.close().await;
    }
    let pool = pool(&config.database_url()).await?;
    if config.apply_schema {
        exec_ignore_exists(&pool, table_statement()).await?;
    }
    Ok(pool)
}

async fn pool(url: &str) -> Result<MySqlPool, AppError> {
    MySqlPoolOptions::new()
        .max_connections(8)
        .after_connect(|conn, _meta| {
            Box::pin(async move {
                sqlx::query("SET time_zone = '+00:00'").execute(&mut *conn).await?;
                sqlx::query("SET NAMES utf8mb4 COLLATE utf8mb4_unicode_ci")
                    .execute(&mut *conn)
                    .await?;
                Ok(())
            })
        })
        .connect(url)
        .await
        .map_err(db_err)
}

fn database_statement() -> &'static str {
    statement_starting_with("CREATE DATABASE")
}

fn table_statement() -> &'static str {
    statement_starting_with("CREATE TABLE")
}

fn statement_starting_with(prefix: &str) -> &'static str {
    statements()
        .into_iter()
        .find(|statement| statement.to_ascii_uppercase().starts_with(prefix))
        .unwrap_or_else(|| panic!("schema.sql is missing {prefix}"))
}

fn statements() -> Vec<&'static str> {
    SCHEMA
        .split(';')
        .map(str::trim)
        .filter(|statement| !statement.is_empty())
        .collect()
}

async fn exec_ignore_exists(pool: &MySqlPool, statement: &'static str) -> Result<(), AppError> {
    match sqlx::query(statement).execute(pool).await {
        Ok(_) => Ok(()),
        Err(err) if already_exists(&err) => Ok(()),
        Err(err) => Err(db_err(err)),
    }
}

fn already_exists(err: &sqlx::Error) -> bool {
    match err {
        sqlx::Error::Database(db) => {
            let code = db.code().unwrap_or_default();
            code == "1007" || code == "1050" || code == "42S01" || db.message().contains("already exists")
        }
        _ => false,
    }
}

fn db_err(err: sqlx::Error) -> AppError {
    AppError::Unavailable(err.to_string())
}
