pub mod attachments;
pub mod auth;
pub mod contacts;
pub mod drafts;
pub mod folder_mgmt;
pub mod folders;
pub mod health;
pub mod identities;
pub mod messages;
pub mod notification_preferences;
pub mod search;
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
use crate::realtime::events::EventBus;
use crate::realtime::idle::IdleManager;
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
    search_engine: Arc<crate::search::engine::SearchEngine>,
    event_bus: Arc<EventBus>,
    idle_manager: Arc<IdleManager>,
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
        .route(
            "/folders",
            get(folders::list_folders).post(folder_mgmt::create_folder),
        )
        .route(
            "/folders/{name}",
            patch(folder_mgmt::rename_folder).delete(folder_mgmt::delete_folder),
        )
        .route(
            "/folders/{name}/subscribe",
            patch(folder_mgmt::subscribe_folder),
        )
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
        .route(
            "/drafts/{draft_id}/attachments/{attachment_id}/content",
            get(attachments::get_attachment_content),
        )
        .route("/search", get(search::search_messages))
        .route(
            "/contacts",
            get(contacts::list_contacts_handler).post(contacts::create_contact_handler),
        )
        .route(
            "/contacts/autocomplete",
            get(contacts::autocomplete_handler),
        )
        .route(
            "/contacts/{id}",
            get(contacts::get_contact_handler).delete(contacts::delete_contact_handler),
        )
        .route(
            "/identities",
            get(identities::list_identities_handler).post(identities::create_identity_handler),
        )
        .route(
            "/identities/{id}",
            get(identities::get_identity_handler)
                .put(identities::update_identity_handler)
                .delete(identities::delete_identity_handler),
        )
        .route(
            "/settings/notifications",
            get(notification_preferences::get_notification_preferences)
                .put(notification_preferences::update_notification_preferences),
        )
        .layer(middleware::from_fn(auth_guard))
        .layer(middleware::from_fn(csrf_protection));

    // WebSocket route — auth is handled inside the handler via cookie,
    // so it bypasses CSRF and auth_guard middleware.
    let ws_route = Router::new()
        .route("/ws", get(crate::realtime::ws::ws_handler));

    let api_router = Router::new()
        .route("/health", get(health::health_check))
        .nest("/auth", auth_router)
        .merge(ws_route)
        .merge(protected_data);

    let index_path = Path::new(&config.static_dir).join("index.html");
    let static_service = ServeDir::new(&config.static_dir).fallback(ServeFile::new(index_path));

    let router = Router::new()
        .nest("/api", api_router)
        .fallback_service(static_service)
        .layer(Extension(idle_manager))
        .layer(Extension(event_bus))
        .layer(Extension(smtp_client))
        .layer(Extension(search_engine))
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
mod tests;
