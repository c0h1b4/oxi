pub mod health;

use std::path::Path;

use axum::Router;
use axum::routing::get;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

/// Assembles all application routes into an Axum Router.
///
/// Currently registers:
/// - `GET /api/health` — health check endpoint
/// - All other paths serve static files from `static_dir`
/// - Non-matching static paths fall back to `index.html` (SPA routing)
///
/// Middleware layers:
/// - CORS (permissive defaults for development)
/// - tower-http tracing
pub fn create_router(static_dir: &str, environment: &str) -> Router {
    let api_router = Router::new().route("/health", get(health::health_check));

    let index_path = Path::new(static_dir).join("index.html");
    let static_service = ServeDir::new(static_dir).fallback(ServeFile::new(index_path));

    let router = Router::new()
        .nest("/api", api_router)
        .fallback_service(static_service)
        .layer(TraceLayer::new_for_http());

    if environment == "development" {
        router.layer(CorsLayer::permissive())
    } else {
        router
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use std::fs;
    use tempfile::TempDir;
    use tower::ServiceExt;

    /// Helper: create a temporary static directory with an index.html.
    fn setup_static_dir() -> TempDir {
        let dir = TempDir::new().expect("should create temp dir");
        fs::write(
            dir.path().join("index.html"),
            "<!DOCTYPE html><html><body>SPA</body></html>",
        )
        .expect("should write index.html");
        dir
    }

    #[tokio::test]
    async fn api_health_works_with_static_fallback() {
        let dir = setup_static_dir();
        let app = create_router(dir.path().to_str().unwrap(), "development");

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "ok");
    }

    #[tokio::test]
    async fn root_serves_index_html() {
        let dir = setup_static_dir();
        let app = create_router(dir.path().to_str().unwrap(), "development");

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes();
        let html = String::from_utf8(body.to_vec()).unwrap();
        assert!(html.contains("SPA"));
    }

    #[tokio::test]
    async fn unknown_path_falls_back_to_index_html() {
        let dir = setup_static_dir();
        let app = create_router(dir.path().to_str().unwrap(), "development");

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/login")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes();
        let html = String::from_utf8(body.to_vec()).unwrap();
        assert!(html.contains("SPA"));
    }

    #[tokio::test]
    async fn static_file_is_served_directly() {
        let dir = setup_static_dir();
        fs::write(dir.path().join("style.css"), "body { color: red; }").unwrap();
        let app = create_router(dir.path().to_str().unwrap(), "development");

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/style.css")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes();
        let css = String::from_utf8(body.to_vec()).unwrap();
        assert!(css.contains("color: red"));
    }

    #[tokio::test]
    async fn nested_spa_path_falls_back_to_index() {
        let dir = setup_static_dir();
        let app = create_router(dir.path().to_str().unwrap(), "development");

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/inbox/some-message-id")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes();
        let html = String::from_utf8(body.to_vec()).unwrap();
        assert!(html.contains("SPA"));
    }
}
