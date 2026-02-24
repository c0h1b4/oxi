pub mod auth;
pub mod health;

use std::path::Path;
use std::sync::Arc;

use axum::routing::{get, post};
use axum::{Extension, Router, middleware};
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

use crate::auth::csrf::csrf_protection;
use crate::auth::middleware::auth_guard;
use crate::auth::session::SessionStore;
use crate::config::AppConfig;

/// Assembles all application routes into an Axum Router.
///
/// Route layout:
/// - `GET  /api/health`       — health check (public, no middleware)
/// - `POST /api/auth/login`   — login (public, CSRF only)
/// - `GET  /api/auth/session` — get session (auth_guard + CSRF)
/// - `POST /api/auth/logout`  — logout (auth_guard + CSRF)
///
/// All other paths serve static files from `config.static_dir`.
/// Non-matching static paths fall back to `index.html` (SPA routing).
///
/// Middleware layers:
/// - CORS (permissive defaults in development)
/// - tower-http tracing
/// - CSRF protection on auth routes
/// - auth_guard on protected routes
pub fn create_router(config: Arc<AppConfig>, store: Arc<SessionStore>) -> Router {
    // Public auth route (CSRF only, no auth required).
    let public_auth = Router::new()
        .route("/login", post(auth::login))
        .layer(middleware::from_fn(csrf_protection));

    // Protected auth routes (auth_guard + CSRF).
    let protected_auth = Router::new()
        .route("/session", get(auth::get_session))
        .route("/logout", post(auth::logout))
        .layer(middleware::from_fn(auth_guard))
        .layer(middleware::from_fn(csrf_protection));

    let auth_router = Router::new()
        .merge(public_auth)
        .merge(protected_auth);

    let api_router = Router::new()
        .route("/health", get(health::health_check))
        .nest("/auth", auth_router);

    let index_path = Path::new(&config.static_dir).join("index.html");
    let static_service = ServeDir::new(&config.static_dir).fallback(ServeFile::new(index_path));

    let router = Router::new()
        .nest("/api", api_router)
        .fallback_service(static_service)
        .layer(Extension(store))
        .layer(Extension(config.clone()))
        .layer(TraceLayer::new_for_http());

    if config.environment == "development" {
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
    use std::time::Duration;
    use tempfile::TempDir;
    use tower::ServiceExt;

    /// Helper: create a test AppConfig with the given static dir.
    fn test_config(static_dir: &str) -> Arc<AppConfig> {
        Arc::new(AppConfig {
            host: "127.0.0.1".to_string(),
            port: 3001,
            imap_host: None,
            imap_port: 993,
            smtp_host: None,
            smtp_port: 587,
            tls_enabled: true,
            data_dir: "/tmp/oxi-test".to_string(),
            session_timeout_hours: 24,
            static_dir: static_dir.to_string(),
            environment: "development".to_string(),
        })
    }

    /// Helper: create a fresh SessionStore for tests.
    fn test_store() -> Arc<SessionStore> {
        Arc::new(SessionStore::new(Duration::from_secs(3600)))
    }

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
        let config = test_config(dir.path().to_str().unwrap());
        let store = test_store();
        let app = create_router(config, store);

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
        let config = test_config(dir.path().to_str().unwrap());
        let store = test_store();
        let app = create_router(config, store);

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
        let config = test_config(dir.path().to_str().unwrap());
        let store = test_store();
        let app = create_router(config, store);

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
        let config = test_config(dir.path().to_str().unwrap());
        let store = test_store();
        let app = create_router(config, store);

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
        let config = test_config(dir.path().to_str().unwrap());
        let store = test_store();
        let app = create_router(config, store);

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

    #[tokio::test]
    async fn login_without_csrf_header_returns_403() {
        let dir = setup_static_dir();
        let config = test_config(dir.path().to_str().unwrap());
        let store = test_store();
        let app = create_router(config, store);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/auth/login")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"email":"test@test.com","password":"pass"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn login_no_imap_host_returns_503() {
        let dir = setup_static_dir();
        let config = test_config(dir.path().to_str().unwrap());
        let store = test_store();
        let app = create_router(config, store);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/auth/login")
                    .header("content-type", "application/json")
                    .header("x-requested-with", "XMLHttpRequest")
                    .body(Body::from(
                        r#"{"email":"test@test.com","password":"pass"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

        let body = response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"]["code"], "SERVICE_UNAVAILABLE");
        assert_eq!(json["error"]["message"], "Mail server not configured");
    }

    #[tokio::test]
    async fn login_empty_email_returns_400() {
        let dir = setup_static_dir();
        let mut cfg = (*test_config(dir.path().to_str().unwrap())).clone();
        cfg.imap_host = Some("127.0.0.1".to_string());
        let config = Arc::new(cfg);
        let store = test_store();
        let app = create_router(config, store);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/auth/login")
                    .header("content-type", "application/json")
                    .header("x-requested-with", "XMLHttpRequest")
                    .body(Body::from(r#"{"email":"","password":"pass"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"]["code"], "BAD_REQUEST");
    }

    #[tokio::test]
    async fn login_empty_password_returns_400() {
        let dir = setup_static_dir();
        let mut cfg = (*test_config(dir.path().to_str().unwrap())).clone();
        cfg.imap_host = Some("127.0.0.1".to_string());
        let config = Arc::new(cfg);
        let store = test_store();
        let app = create_router(config, store);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/auth/login")
                    .header("content-type", "application/json")
                    .header("x-requested-with", "XMLHttpRequest")
                    .body(Body::from(
                        r#"{"email":"test@test.com","password":""}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn login_unreachable_imap_returns_503() {
        let dir = setup_static_dir();
        let mut cfg = (*test_config(dir.path().to_str().unwrap())).clone();
        cfg.imap_host = Some("127.0.0.1".to_string());
        cfg.imap_port = 19999; // Nothing listening here
        cfg.tls_enabled = false;
        let config = Arc::new(cfg);
        let store = test_store();
        let app = create_router(config, store);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/auth/login")
                    .header("content-type", "application/json")
                    .header("x-requested-with", "XMLHttpRequest")
                    .body(Body::from(
                        r#"{"email":"test@test.com","password":"pass"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

        let body = response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"]["code"], "SERVICE_UNAVAILABLE");
        assert_eq!(json["error"]["message"], "Cannot reach mail server");
    }

    #[tokio::test]
    async fn get_session_without_auth_returns_401() {
        let dir = setup_static_dir();
        let config = test_config(dir.path().to_str().unwrap());
        let store = test_store();
        let app = create_router(config, store);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/auth/session")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn get_session_with_valid_session_returns_200() {
        let dir = setup_static_dir();
        let config = test_config(dir.path().to_str().unwrap());
        let store = test_store();
        let token = store.insert(
            "alice@example.com".to_string(),
            "pass".to_string(),
            "hash".to_string(),
        );
        let app = create_router(config, store);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/auth/session")
                    .header("cookie", format!("oxi_session={token}"))
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
        assert_eq!(json["user"]["email"], "alice@example.com");
    }

    #[tokio::test]
    async fn logout_without_auth_returns_401() {
        let dir = setup_static_dir();
        let config = test_config(dir.path().to_str().unwrap());
        let store = test_store();
        let app = create_router(config, store);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/auth/logout")
                    .header("x-requested-with", "XMLHttpRequest")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn logout_with_valid_session_returns_200() {
        let dir = setup_static_dir();
        let config = test_config(dir.path().to_str().unwrap());
        let store = test_store();
        let token = store.insert(
            "alice@example.com".to_string(),
            "pass".to_string(),
            "hash".to_string(),
        );
        let app = create_router(config, store.clone());

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/auth/logout")
                    .header("cookie", format!("oxi_session={token}"))
                    .header("x-requested-with", "XMLHttpRequest")
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
        assert_eq!(json["status"], "logged_out");

        // Session should be removed from the store.
        assert!(store.get(&token).is_none());
    }

    #[tokio::test]
    async fn logout_clears_cookie() {
        let dir = setup_static_dir();
        let config = test_config(dir.path().to_str().unwrap());
        let store = test_store();
        let token = store.insert(
            "alice@example.com".to_string(),
            "pass".to_string(),
            "hash".to_string(),
        );
        let app = create_router(config, store);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/auth/logout")
                    .header("cookie", format!("oxi_session={token}"))
                    .header("x-requested-with", "XMLHttpRequest")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let set_cookie = response
            .headers()
            .get("set-cookie")
            .expect("should have set-cookie header")
            .to_str()
            .unwrap();
        assert!(set_cookie.contains("oxi_session=;"));
        assert!(set_cookie.contains("Max-Age=0"));
    }
}
