pub mod health;

use axum::Router;
use axum::routing::get;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

/// Assembles all application routes into an Axum Router.
///
/// Currently registers:
/// - `GET /api/v1/health` — health check endpoint
///
/// Middleware layers:
/// - CORS (permissive defaults for development)
/// - tower-http tracing
pub fn create_router() -> Router {
    let api_v1 = Router::new().route("/health", get(health::health_check));

    Router::new()
        .nest("/api/v1", api_v1)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
}
