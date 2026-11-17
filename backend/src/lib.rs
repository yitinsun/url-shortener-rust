pub mod config;
pub mod encoding;
pub mod error;
pub mod repo;
pub mod schema;
pub mod schedule;
pub mod service;
pub mod web;

use std::net::SocketAddr;

use tokio::net::TcpListener;

use crate::config::Config;
use crate::repo::url_mapping::SqlxUrlMappingRepository;
use crate::service::url_shortener::UrlShortenerService;

pub async fn serve() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = Config::from_env().map_err(|err| err.to_string())?;
    serve_with(config).await
}

pub async fn serve_with(config: Config) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt().try_init().ok();

    let bind: SocketAddr = config
        .bind_addr
        .parse()
        .map_err(|_| format!("BIND_ADDR is invalid: {}", config.bind_addr))?;
    let pool = schema::connect(&config).await.map_err(|err| err.to_string())?;
    let repo = SqlxUrlMappingRepository::new(pool);
    schedule::cleanup::spawn(repo.clone());
    let service = UrlShortenerService::new(repo, config.base_url.clone());
    let app = web::application(
        service,
        config.openapi_enabled.then_some(config.base_url.as_str()),
    );
    tracing::info!(%bind, env = ?config.env, "url shortener listening");
    let listener = TcpListener::bind(bind).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
