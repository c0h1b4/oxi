mod config;
#[allow(dead_code)]
mod error;
mod routes;

use config::AppConfig;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize structured JSON logging with env filter.
    // Default to INFO level; override with RUST_LOG env var.
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(fmt::layer().json())
        .init();

    // Load configuration via figment (serde defaults + env vars).
    let config = AppConfig::load()?;

    // Build the application router with static file serving.
    let app = routes::create_router(&config.static_dir, &config.environment);

    // Bind to the configured host and port.
    let bind_addr = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;

    tracing::info!(
        host = %config.host,
        port = %config.port,
        static_dir = %config.static_dir,
        "oxi-email server starting"
    );

    axum::serve(listener, app).await?;

    Ok(())
}
