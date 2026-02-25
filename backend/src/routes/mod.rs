pub mod attachments;
pub mod auth;
pub mod drafts;
pub mod folders;
pub mod health;
pub mod messages;
pub mod send;

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::ConnectInfo;
use axum::http::Request;
use axum::routing::{delete, get, patch, post};
use axum::{Extension, Router, middleware};
use tower_governor::GovernorError;
use tower_governor::GovernorLayer;
use tower_governor::governor::GovernorConfigBuilder;
use tower_governor::key_extractor::KeyExtractor;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

use crate::auth::csrf::csrf_protection;
use crate::auth::middleware::auth_guard;
use crate::auth::session::SessionStore;
use crate::config::AppConfig;
use crate::imap::client::ImapClient;
use crate::smtp::client::SmtpClient;

/// Per-IP key extractor that falls back to the loopback address when
/// `ConnectInfo<SocketAddr>` is unavailable (e.g. in unit tests using
/// `oneshot`).  In production the server is started with
/// `into_make_service_with_connect_info::<SocketAddr>()` so the real
/// peer IP is always present.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PeerIpKeyExtractorFallback;

impl KeyExtractor for PeerIpKeyExtractorFallback {
    type Key = IpAddr;

    fn extract<T>(&self, req: &Request<T>) -> Result<Self::Key, GovernorError> {
        // Try ConnectInfo<SocketAddr> first (production path).
        let ip = req
            .extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .map(|ci: &ConnectInfo<SocketAddr>| ci.0.ip());

        Ok(ip.unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST)))
    }
}

/// Assembles all application routes into an Axum Router.
///
/// Route layout:
/// - `GET  /api/health`                        — health check (public)
/// - `POST /api/auth/login`                    — login (public, CSRF only)
/// - `GET  /api/auth/session`                  — get session (auth_guard + CSRF)
/// - `POST /api/auth/logout`                   — logout (auth_guard + CSRF)
/// - `GET  /api/folders`                       — list folders (auth_guard + CSRF)
/// - `GET  /api/folders/:folder/messages`      — list messages (auth_guard + CSRF)
/// - `GET  /api/messages/:folder/:uid`         — get message detail (auth_guard + CSRF)
/// - `PATCH /api/messages/:folder/:uid/flags`  — update flags (auth_guard + CSRF)
/// - `GET  /api/messages/:folder/:uid/attachments/:attachment_id` — download attachment (auth_guard + CSRF)
/// - `POST /api/messages/move`                 — move message (auth_guard + CSRF)
/// - `DELETE /api/messages/:folder/:uid`       — delete message (auth_guard + CSRF)
///
/// All other paths serve static files from `config.static_dir`.
/// Non-matching static paths fall back to `index.html` (SPA routing).
///
/// Middleware layers:
/// - CORS (permissive defaults in development)
/// - tower-http tracing
/// - CSRF protection on auth routes
/// - auth_guard on protected routes
pub fn create_router(
    config: Arc<AppConfig>,
    store: Arc<SessionStore>,
    imap_client: Arc<dyn ImapClient>,
    smtp_client: Arc<dyn SmtpClient>,
) -> Router {
    // Rate-limit login: replenish 1 token every 12 s, burst of 5.
    let governor_conf = GovernorConfigBuilder::default()
        .key_extractor(PeerIpKeyExtractorFallback)
        .period(Duration::from_secs(12))
        .burst_size(5)
        .finish()
        .expect("valid governor config");

    // Public auth route: GovernorLayer (outermost) -> CSRF -> handler.
    let public_auth = Router::new()
        .route("/login", post(auth::login))
        .layer(middleware::from_fn(csrf_protection))
        .layer(GovernorLayer::new(governor_conf));

    // Protected auth routes (auth_guard + CSRF).
    let protected_auth = Router::new()
        .route("/session", get(auth::get_session))
        .route("/logout", post(auth::logout))
        .layer(middleware::from_fn(auth_guard))
        .layer(middleware::from_fn(csrf_protection));

    let auth_router = Router::new()
        .merge(public_auth)
        .merge(protected_auth);

    // Protected data routes (auth_guard + CSRF).
    let protected_data = Router::new()
        .route("/folders", get(folders::list_folders))
        .route("/folders/{folder}/messages", get(messages::list_messages))
        .route("/messages/{folder}/{uid}", get(messages::get_message))
        .route(
            "/messages/{folder}/{uid}/flags",
            patch(messages::update_flags),
        )
        .route(
            "/messages/{folder}/{uid}/attachments/{attachment_id}",
            get(messages::download_attachment),
        )
        .route("/messages/move", post(messages::move_message_handler))
        .route("/messages/send", post(send::send_message_handler))
        .route(
            "/messages/{folder}/{uid}",
            delete(messages::delete_message_handler),
        )
        .route("/drafts", post(drafts::upsert_draft_handler))
        .route("/drafts", get(drafts::list_drafts_handler))
        .route("/drafts/{id}", get(drafts::get_draft_handler))
        .route("/drafts/{id}", delete(drafts::delete_draft_handler))
        .route(
            "/drafts/{draft_id}/attachments",
            post(attachments::upload_attachment),
        )
        .route(
            "/drafts/{draft_id}/attachments/{attachment_id}",
            delete(attachments::delete_attachment),
        )
        .layer(middleware::from_fn(auth_guard))
        .layer(middleware::from_fn(csrf_protection));

    let api_router = Router::new()
        .route("/health", get(health::health_check))
        .nest("/auth", auth_router)
        .merge(protected_data);

    let index_path = Path::new(&config.static_dir).join("index.html");
    let static_service = ServeDir::new(&config.static_dir).fallback(ServeFile::new(index_path));

    let router = Router::new()
        .nest("/api", api_router)
        .fallback_service(static_service)
        .layer(Extension(smtp_client))
        .layer(Extension(imap_client))
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

    use crate::imap::client::mock::MockImapClient;
    use crate::imap::client::{
        EmailAddress, ImapAttachment, ImapError, ImapFolder, ImapMessageBody, ImapMessageHeader,
    };
    use crate::smtp::client::SmtpError;
    use crate::smtp::client::mock::MockSmtpClient;

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

    /// Helper: create a test AppConfig with IMAP host configured and a custom data dir.
    fn test_config_with_imap(static_dir: &str, data_dir: &str) -> Arc<AppConfig> {
        Arc::new(AppConfig {
            host: "127.0.0.1".to_string(),
            port: 3001,
            imap_host: Some("imap.example.com".to_string()),
            imap_port: 993,
            smtp_host: None,
            smtp_port: 587,
            tls_enabled: true,
            data_dir: data_dir.to_string(),
            session_timeout_hours: 24,
            static_dir: static_dir.to_string(),
            environment: "development".to_string(),
        })
    }

    /// Helper: create a test AppConfig with IMAP + SMTP hosts configured.
    fn test_config_with_smtp(static_dir: &str, data_dir: &str) -> Arc<AppConfig> {
        Arc::new(AppConfig {
            host: "127.0.0.1".to_string(),
            port: 3001,
            imap_host: Some("imap.example.com".to_string()),
            imap_port: 993,
            smtp_host: Some("smtp.example.com".to_string()),
            smtp_port: 587,
            tls_enabled: true,
            data_dir: data_dir.to_string(),
            session_timeout_hours: 24,
            static_dir: static_dir.to_string(),
            environment: "development".to_string(),
        })
    }

    /// Helper: create a fresh SessionStore for tests.
    fn test_store() -> Arc<SessionStore> {
        Arc::new(SessionStore::new(Duration::from_secs(3600)))
    }

    /// Helper: create a default mock IMAP client.
    fn test_imap_client() -> Arc<dyn ImapClient> {
        Arc::new(MockImapClient::new())
    }

    /// Helper: create a default mock SMTP client.
    fn test_smtp_client() -> Arc<dyn SmtpClient> {
        Arc::new(MockSmtpClient::new())
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

    /// Helper: provision a user database so that route handlers can open it.
    /// Migrations are applied automatically by `open_user_db`.
    fn provision_user_db(data_dir: &str, user_hash: &str) {
        let _conn = crate::db::pool::open_user_db(data_dir, user_hash).unwrap();
    }

    // -----------------------------------------------------------------------
    // Existing tests (updated to pass imap_client)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn api_health_works_with_static_fallback() {
        let dir = setup_static_dir();
        let config = test_config(dir.path().to_str().unwrap());
        let store = test_store();
        let app = create_router(config, store, test_imap_client(), test_smtp_client());

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
        let app = create_router(config, store, test_imap_client(), test_smtp_client());

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
        let app = create_router(config, store, test_imap_client(), test_smtp_client());

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
        let app = create_router(config, store, test_imap_client(), test_smtp_client());

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
        let app = create_router(config, store, test_imap_client(), test_smtp_client());

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
        let app = create_router(config, store, test_imap_client(), test_smtp_client());

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
        let app = create_router(config, store, test_imap_client(), test_smtp_client());

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
        let app = create_router(config, store, test_imap_client(), test_smtp_client());

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
        let app = create_router(config, store, test_imap_client(), test_smtp_client());

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
        let app = create_router(config, store, test_imap_client(), test_smtp_client());

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
        let app = create_router(config, store, test_imap_client(), test_smtp_client());

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
        let app = create_router(config, store, test_imap_client(), test_smtp_client());

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
        let app = create_router(config, store, test_imap_client(), test_smtp_client());

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
        let app = create_router(config, store.clone(), test_imap_client(), test_smtp_client());

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
        let app = create_router(config, store, test_imap_client(), test_smtp_client());

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

    // -----------------------------------------------------------------------
    // New tests for folders and messages routes
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn get_folders_returns_200_with_folder_list() {
        let static_dir = setup_static_dir();
        let data_dir = TempDir::new().unwrap();
        let config = test_config_with_imap(
            static_dir.path().to_str().unwrap(),
            data_dir.path().to_str().unwrap(),
        );
        let store = test_store();
        let token = store.insert(
            "alice@example.com".to_string(),
            "pass".to_string(),
            "testhash".to_string(),
        );

        // Provision user database.
        provision_user_db(data_dir.path().to_str().unwrap(), "testhash");

        let mock = MockImapClient::new().with_folders(vec![
            ImapFolder {
                name: "INBOX".to_string(),
                delimiter: Some("/".to_string()),
                attributes: vec!["\\HasNoChildren".to_string()],
            },
            ImapFolder {
                name: "Sent".to_string(),
                delimiter: Some("/".to_string()),
                attributes: vec![],
            },
        ]);
        let imap_client: Arc<dyn ImapClient> = Arc::new(mock);
        let app = create_router(config, store, imap_client, test_smtp_client());

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/folders")
                    .header("cookie", format!("oxi_session={token}"))
                    .header("x-requested-with", "XMLHttpRequest")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        let folders = json["folders"].as_array().unwrap();
        assert_eq!(folders.len(), 2);
        assert_eq!(folders[0]["name"], "INBOX");
        assert_eq!(folders[1]["name"], "Sent");
    }

    #[tokio::test]
    async fn get_folders_returns_503_when_imap_fails() {
        let static_dir = setup_static_dir();
        let data_dir = TempDir::new().unwrap();
        let config = test_config_with_imap(
            static_dir.path().to_str().unwrap(),
            data_dir.path().to_str().unwrap(),
        );
        let store = test_store();
        let token = store.insert(
            "alice@example.com".to_string(),
            "pass".to_string(),
            "testhash".to_string(),
        );

        let mock = MockImapClient::new()
            .with_error(ImapError::ConnectionFailed("test failure".to_string()));
        let imap_client: Arc<dyn ImapClient> = Arc::new(mock);
        let app = create_router(config, store, imap_client, test_smtp_client());

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/folders")
                    .header("cookie", format!("oxi_session={token}"))
                    .header("x-requested-with", "XMLHttpRequest")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn get_folders_returns_401_without_auth() {
        let static_dir = setup_static_dir();
        let data_dir = TempDir::new().unwrap();
        let config = test_config_with_imap(
            static_dir.path().to_str().unwrap(),
            data_dir.path().to_str().unwrap(),
        );
        let store = test_store();
        let app = create_router(config, store, test_imap_client(), test_smtp_client());

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/folders")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn get_messages_returns_200_with_paginated_list() {
        let static_dir = setup_static_dir();
        let data_dir = TempDir::new().unwrap();
        let config = test_config_with_imap(
            static_dir.path().to_str().unwrap(),
            data_dir.path().to_str().unwrap(),
        );
        let store = test_store();
        let token = store.insert(
            "alice@example.com".to_string(),
            "pass".to_string(),
            "testhash".to_string(),
        );

        provision_user_db(data_dir.path().to_str().unwrap(), "testhash");

        let mock = MockImapClient::new().with_headers(vec![
            ImapMessageHeader {
                uid: 1,
                subject: Some("Hello World".to_string()),
                from: vec![EmailAddress {
                    name: Some("Alice".to_string()),
                    address: "alice@example.com".to_string(),
                }],
                to: vec![EmailAddress {
                    name: None,
                    address: "bob@example.com".to_string(),
                }],
                date: Some("2024-01-01T10:00:00Z".to_string()),
                flags: vec!["\\Seen".to_string()],
                has_attachments: false,
            },
            ImapMessageHeader {
                uid: 2,
                subject: Some("Second message".to_string()),
                from: vec![EmailAddress {
                    name: Some("Bob".to_string()),
                    address: "bob@example.com".to_string(),
                }],
                to: vec![EmailAddress {
                    name: None,
                    address: "alice@example.com".to_string(),
                }],
                date: Some("2024-01-02T10:00:00Z".to_string()),
                flags: vec![],
                has_attachments: false,
            },
        ]);
        let imap_client: Arc<dyn ImapClient> = Arc::new(mock);
        let app = create_router(config, store, imap_client, test_smtp_client());

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/folders/INBOX/messages?page=0&per_page=50")
                    .header("cookie", format!("oxi_session={token}"))
                    .header("x-requested-with", "XMLHttpRequest")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["total_count"], 2);
        assert_eq!(json["page"], 0);
        assert_eq!(json["per_page"], 50);

        let messages = json["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 2);
    }

    #[tokio::test]
    async fn get_message_returns_sanitized_html() {
        let static_dir = setup_static_dir();
        let data_dir = TempDir::new().unwrap();
        let config = test_config_with_imap(
            static_dir.path().to_str().unwrap(),
            data_dir.path().to_str().unwrap(),
        );
        let store = test_store();
        let token = store.insert(
            "alice@example.com".to_string(),
            "pass".to_string(),
            "testhash".to_string(),
        );

        provision_user_db(data_dir.path().to_str().unwrap(), "testhash");

        // First, we need the message header in cache (fetch_headers first).
        let mock = MockImapClient::new()
            .with_headers(vec![ImapMessageHeader {
                uid: 42,
                subject: Some("Test Subject".to_string()),
                from: vec![EmailAddress {
                    name: Some("Alice".to_string()),
                    address: "alice@example.com".to_string(),
                }],
                to: vec![EmailAddress {
                    name: None,
                    address: "bob@example.com".to_string(),
                }],
                date: Some("2024-01-01T10:00:00Z".to_string()),
                flags: vec!["\\Seen".to_string()],
                has_attachments: false,
            }])
            .with_bodies(vec![ImapMessageBody {
                uid: 42,
                text_plain: Some("Hello plain text".to_string()),
                text_html: Some(
                    "<p>Hello</p><script>alert('xss')</script><b>bold</b>".to_string(),
                ),
                attachments: vec![],
                raw_headers: String::new(),
            }]);
        let imap_client: Arc<dyn ImapClient> = Arc::new(mock);
        let app = create_router(config.clone(), store.clone(), imap_client.clone(), test_smtp_client());

        // First, populate the message cache by listing messages.
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/folders/INBOX/messages")
                    .header("cookie", format!("oxi_session={token}"))
                    .header("x-requested-with", "XMLHttpRequest")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Now get the full message.
        let app2 = create_router(config, store, imap_client, test_smtp_client());
        let response = app2
            .oneshot(
                Request::builder()
                    .uri("/api/messages/INBOX/42")
                    .header("cookie", format!("oxi_session={token}"))
                    .header("x-requested-with", "XMLHttpRequest")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["uid"], 42);
        assert_eq!(json["subject"], "Test Subject");

        // Script tag should be stripped by ammonia.
        let html = json["html"].as_str().unwrap();
        assert!(!html.contains("script"));
        assert!(html.contains("<b>bold</b>"));
        assert!(html.contains("<p>Hello</p>"));

        // Plain text should be preserved.
        assert_eq!(json["text"], "Hello plain text");

        // Flags should be an array.
        assert!(json["flags"].is_array());

        // to_addresses should be an array.
        assert!(json["to_addresses"].is_array());
    }

    #[tokio::test]
    async fn update_flags_returns_200() {
        let static_dir = setup_static_dir();
        let data_dir = TempDir::new().unwrap();
        let config = test_config_with_imap(
            static_dir.path().to_str().unwrap(),
            data_dir.path().to_str().unwrap(),
        );
        let store = test_store();
        let token = store.insert(
            "alice@example.com".to_string(),
            "pass".to_string(),
            "testhash".to_string(),
        );

        provision_user_db(data_dir.path().to_str().unwrap(), "testhash");

        // Seed a message in the cache.
        let conn = crate::db::pool::open_user_db(
            data_dir.path().to_str().unwrap(),
            "testhash",
        )
        .unwrap();
        crate::db::folders::upsert_folder(&conn, "INBOX", None, None, "", true, 0, 0, 0, 0)
            .unwrap();
        crate::db::messages::upsert_message(
            &conn, "INBOX", 1, None, None, None, "Test", "a@b.com", "A", "[]", "[]",
            "2024-01-01", "", 0, false, "",
        )
        .unwrap();
        drop(conn);

        let mock = MockImapClient::new();
        let imap_client: Arc<dyn ImapClient> = Arc::new(mock);
        let app = create_router(config, store, imap_client, test_smtp_client());

        let response = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri("/api/messages/INBOX/1/flags")
                    .header("cookie", format!("oxi_session={token}"))
                    .header("x-requested-with", "XMLHttpRequest")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"flags":["\\Seen","\\Flagged"],"add":true}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn move_message_returns_200() {
        let static_dir = setup_static_dir();
        let data_dir = TempDir::new().unwrap();
        let config = test_config_with_imap(
            static_dir.path().to_str().unwrap(),
            data_dir.path().to_str().unwrap(),
        );
        let store = test_store();
        let token = store.insert(
            "alice@example.com".to_string(),
            "pass".to_string(),
            "testhash".to_string(),
        );

        provision_user_db(data_dir.path().to_str().unwrap(), "testhash");

        // Seed a message in the cache.
        let conn = crate::db::pool::open_user_db(
            data_dir.path().to_str().unwrap(),
            "testhash",
        )
        .unwrap();
        crate::db::folders::upsert_folder(&conn, "INBOX", None, None, "", true, 0, 0, 0, 0)
            .unwrap();
        crate::db::messages::upsert_message(
            &conn, "INBOX", 42, None, None, None, "Test", "a@b.com", "A", "[]", "[]",
            "2024-01-01", "", 0, false, "",
        )
        .unwrap();
        drop(conn);

        let mock = MockImapClient::new();
        let imap_client: Arc<dyn ImapClient> = Arc::new(mock);
        let app = create_router(config, store, imap_client, test_smtp_client());

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/messages/move")
                    .header("cookie", format!("oxi_session={token}"))
                    .header("x-requested-with", "XMLHttpRequest")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"from_folder":"INBOX","to_folder":"Archive","uid":42}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn delete_message_returns_200() {
        let static_dir = setup_static_dir();
        let data_dir = TempDir::new().unwrap();
        let config = test_config_with_imap(
            static_dir.path().to_str().unwrap(),
            data_dir.path().to_str().unwrap(),
        );
        let store = test_store();
        let token = store.insert(
            "alice@example.com".to_string(),
            "pass".to_string(),
            "testhash".to_string(),
        );

        provision_user_db(data_dir.path().to_str().unwrap(), "testhash");

        // Seed a message in the cache.
        let conn = crate::db::pool::open_user_db(
            data_dir.path().to_str().unwrap(),
            "testhash",
        )
        .unwrap();
        crate::db::folders::upsert_folder(&conn, "INBOX", None, None, "", true, 0, 0, 0, 0)
            .unwrap();
        crate::db::messages::upsert_message(
            &conn, "INBOX", 7, None, None, None, "Test", "a@b.com", "A", "[]", "[]",
            "2024-01-01", "", 0, false, "",
        )
        .unwrap();
        drop(conn);

        let mock = MockImapClient::new();
        let imap_client: Arc<dyn ImapClient> = Arc::new(mock);
        let app = create_router(config, store, imap_client, test_smtp_client());

        let response = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/api/messages/INBOX/7")
                    .header("cookie", format!("oxi_session={token}"))
                    .header("x-requested-with", "XMLHttpRequest")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    // -----------------------------------------------------------------------
    // Attachment download tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn download_attachment_returns_binary_with_correct_headers() {
        let static_dir = setup_static_dir();
        let data_dir = TempDir::new().unwrap();
        let config = test_config_with_imap(
            static_dir.path().to_str().unwrap(),
            data_dir.path().to_str().unwrap(),
        );
        let store = test_store();
        let token = store.insert(
            "alice@example.com".to_string(),
            "pass".to_string(),
            "testhash".to_string(),
        );

        provision_user_db(data_dir.path().to_str().unwrap(), "testhash");

        let attachment_data: Vec<u8> = vec![0xDE, 0xAD, 0xBE, 0xEF];
        let mock = MockImapClient::new().with_bodies(vec![ImapMessageBody {
            uid: 42,
            text_plain: Some("text".to_string()),
            text_html: None,
            attachments: vec![ImapAttachment {
                filename: Some("document.pdf".to_string()),
                content_type: "application/pdf".to_string(),
                size: 4,
                data: attachment_data.clone(),
                content_id: None,
            }],
            raw_headers: String::new(),
        }]);
        let imap_client: Arc<dyn ImapClient> = Arc::new(mock);
        let app = create_router(config, store, imap_client, test_smtp_client());

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/messages/INBOX/42/attachments/0")
                    .header("cookie", format!("oxi_session={token}"))
                    .header("x-requested-with", "XMLHttpRequest")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        // Verify content-type header.
        let ct = response
            .headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap();
        assert_eq!(ct, "application/pdf");

        // Verify content-disposition header.
        let cd = response
            .headers()
            .get("content-disposition")
            .unwrap()
            .to_str()
            .unwrap();
        assert!(cd.contains("attachment"));
        assert!(cd.contains("document.pdf"));

        // Verify body bytes match the attachment data.
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(body.as_ref(), &attachment_data);
    }

    #[tokio::test]
    async fn download_attachment_returns_404_for_invalid_index() {
        let static_dir = setup_static_dir();
        let data_dir = TempDir::new().unwrap();
        let config = test_config_with_imap(
            static_dir.path().to_str().unwrap(),
            data_dir.path().to_str().unwrap(),
        );
        let store = test_store();
        let token = store.insert(
            "alice@example.com".to_string(),
            "pass".to_string(),
            "testhash".to_string(),
        );

        provision_user_db(data_dir.path().to_str().unwrap(), "testhash");

        let mock = MockImapClient::new().with_bodies(vec![ImapMessageBody {
            uid: 42,
            text_plain: Some("text".to_string()),
            text_html: None,
            attachments: vec![ImapAttachment {
                filename: Some("document.pdf".to_string()),
                content_type: "application/pdf".to_string(),
                size: 4,
                data: vec![0xDE, 0xAD, 0xBE, 0xEF],
                content_id: None,
            }],
            raw_headers: String::new(),
        }]);
        let imap_client: Arc<dyn ImapClient> = Arc::new(mock);
        let app = create_router(config, store, imap_client, test_smtp_client());

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/messages/INBOX/42/attachments/99")
                    .header("cookie", format!("oxi_session={token}"))
                    .header("x-requested-with", "XMLHttpRequest")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn download_attachment_returns_400_for_non_numeric_id() {
        let static_dir = setup_static_dir();
        let data_dir = TempDir::new().unwrap();
        let config = test_config_with_imap(
            static_dir.path().to_str().unwrap(),
            data_dir.path().to_str().unwrap(),
        );
        let store = test_store();
        let token = store.insert(
            "alice@example.com".to_string(),
            "pass".to_string(),
            "testhash".to_string(),
        );

        provision_user_db(data_dir.path().to_str().unwrap(), "testhash");

        let mock = MockImapClient::new().with_bodies(vec![ImapMessageBody {
            uid: 42,
            text_plain: Some("text".to_string()),
            text_html: None,
            attachments: vec![ImapAttachment {
                filename: Some("document.pdf".to_string()),
                content_type: "application/pdf".to_string(),
                size: 4,
                data: vec![0xDE, 0xAD, 0xBE, 0xEF],
                content_id: None,
            }],
            raw_headers: String::new(),
        }]);
        let imap_client: Arc<dyn ImapClient> = Arc::new(mock);
        let app = create_router(config, store, imap_client, test_smtp_client());

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/messages/INBOX/42/attachments/abc")
                    .header("cookie", format!("oxi_session={token}"))
                    .header("x-requested-with", "XMLHttpRequest")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    // -----------------------------------------------------------------------
    // Send message endpoint tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn send_returns_200_on_success() {
        let static_dir = setup_static_dir();
        let data_dir = TempDir::new().unwrap();
        let config = test_config_with_smtp(
            static_dir.path().to_str().unwrap(),
            data_dir.path().to_str().unwrap(),
        );
        let store = test_store();
        let token = store.insert(
            "alice@example.com".to_string(),
            "pass".to_string(),
            "testhash".to_string(),
        );

        provision_user_db(data_dir.path().to_str().unwrap(), "testhash");

        let mock_smtp: Arc<dyn SmtpClient> = Arc::new(MockSmtpClient::new());
        let mock_imap: Arc<dyn ImapClient> = Arc::new(MockImapClient::new());
        let app = create_router(config, store, mock_imap, mock_smtp);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/messages/send")
                    .header("cookie", format!("oxi_session={token}"))
                    .header("x-requested-with", "XMLHttpRequest")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"to":["bob@example.com"],"subject":"Hello","text_body":"Hi Bob"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "sent");
        assert!(json["message_id"].as_str().is_some());
    }

    #[tokio::test]
    async fn send_returns_400_without_recipients() {
        let static_dir = setup_static_dir();
        let data_dir = TempDir::new().unwrap();
        let config = test_config_with_smtp(
            static_dir.path().to_str().unwrap(),
            data_dir.path().to_str().unwrap(),
        );
        let store = test_store();
        let token = store.insert(
            "alice@example.com".to_string(),
            "pass".to_string(),
            "testhash".to_string(),
        );

        provision_user_db(data_dir.path().to_str().unwrap(), "testhash");

        let app = create_router(config, store, test_imap_client(), test_smtp_client());

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/messages/send")
                    .header("cookie", format!("oxi_session={token}"))
                    .header("x-requested-with", "XMLHttpRequest")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"to":[],"subject":"Hello","text_body":"Hi"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"]["code"], "BAD_REQUEST");
    }

    #[tokio::test]
    async fn send_returns_400_with_empty_body_and_subject() {
        let static_dir = setup_static_dir();
        let data_dir = TempDir::new().unwrap();
        let config = test_config_with_smtp(
            static_dir.path().to_str().unwrap(),
            data_dir.path().to_str().unwrap(),
        );
        let store = test_store();
        let token = store.insert(
            "alice@example.com".to_string(),
            "pass".to_string(),
            "testhash".to_string(),
        );

        provision_user_db(data_dir.path().to_str().unwrap(), "testhash");

        let app = create_router(config, store, test_imap_client(), test_smtp_client());

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/messages/send")
                    .header("cookie", format!("oxi_session={token}"))
                    .header("x-requested-with", "XMLHttpRequest")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"to":["bob@example.com"],"subject":"","text_body":""}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn send_returns_503_when_smtp_not_configured() {
        let static_dir = setup_static_dir();
        let data_dir = TempDir::new().unwrap();
        // Use config WITHOUT smtp_host
        let config = test_config_with_imap(
            static_dir.path().to_str().unwrap(),
            data_dir.path().to_str().unwrap(),
        );
        let store = test_store();
        let token = store.insert(
            "alice@example.com".to_string(),
            "pass".to_string(),
            "testhash".to_string(),
        );

        provision_user_db(data_dir.path().to_str().unwrap(), "testhash");

        let app = create_router(config, store, test_imap_client(), test_smtp_client());

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/messages/send")
                    .header("cookie", format!("oxi_session={token}"))
                    .header("x-requested-with", "XMLHttpRequest")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"to":["bob@example.com"],"subject":"Hello","text_body":"Hi"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"]["code"], "SERVICE_UNAVAILABLE");
    }

    #[tokio::test]
    async fn send_returns_503_when_smtp_fails() {
        let static_dir = setup_static_dir();
        let data_dir = TempDir::new().unwrap();
        let config = test_config_with_smtp(
            static_dir.path().to_str().unwrap(),
            data_dir.path().to_str().unwrap(),
        );
        let store = test_store();
        let token = store.insert(
            "alice@example.com".to_string(),
            "pass".to_string(),
            "testhash".to_string(),
        );

        provision_user_db(data_dir.path().to_str().unwrap(), "testhash");

        let failing_smtp: Arc<dyn SmtpClient> = Arc::new(
            MockSmtpClient::new()
                .with_error(SmtpError::SendFailed("relay denied".to_string())),
        );
        let app = create_router(config, store, test_imap_client(), failing_smtp);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/messages/send")
                    .header("cookie", format!("oxi_session={token}"))
                    .header("x-requested-with", "XMLHttpRequest")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"to":["bob@example.com"],"subject":"Hello","text_body":"Hi"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json["error"]["message"]
            .as_str()
            .unwrap()
            .contains("relay denied"));
    }
}
