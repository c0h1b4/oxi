mod config;
#[allow(dead_code)]
mod error;
mod auth;
mod routes;

use std::sync::Arc;
use std::time::Duration;

use config::AppConfig;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

use crate::auth::session::SessionStore;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize structured JSON logging with env filter.
    // Default to INFO level; override with RUST_LOG env var.
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(fmt::layer().json())
        .init();

    // Load configuration via figment (serde defaults + env vars).
    let config = Arc::new(AppConfig::load()?);

    // Create the in-memory session store with the configured timeout.
    let store = Arc::new(SessionStore::new(Duration::from_secs(
        config.session_timeout_hours * 3600,
    )));

    // Spawn a background task that periodically purges expired sessions.
    {
        let store = Arc::clone(&store);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300));
            loop {
                interval.tick().await;
                store.purge_expired();
                tracing::debug!("Purged expired sessions");
            }
        });
    }

    // Build the application router with auth, session, and static file serving.
    let app = routes::create_router(config.clone(), store);

    // Bind to the configured host and port.
    let bind_addr = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;

    tracing::info!(
        host = %config.host,
        port = %config.port,
        static_dir = %config.static_dir,
        "oxi-email server starting"
    );

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;

    Ok(())
}
