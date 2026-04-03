pub mod attachments;
pub mod auth;
pub mod calendar;
pub mod contact_groups;
pub mod contacts;
pub mod display_preferences;
pub mod drafts;
pub mod folder_mgmt;
pub mod folders;
pub mod health;
pub mod identities;
pub mod messages;
pub mod notification_preferences;
pub mod quota;
pub mod search;
pub mod send;
pub mod tags;

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::ConnectInfo;
use axum::http::Request;
use axum::routing::{delete, get, patch, post, put};
use axum::{Extension, Router, middleware};
use tower_governor::GovernorError;
use tower_governor::GovernorLayer;
use tower_governor::governor::GovernorConfigBuilder;
use tower_governor::key_extractor::KeyExtractor;
use axum::http::{header, HeaderName, HeaderValue, Method};
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

use crate::auth::csrf::csrf_protection;
use crate::auth::middleware::auth_guard;
use crate::auth::session::SessionStore;
use crate::config::AppConfig;
use crate::imap::client::ImapClient;
use crate::realtime::events::EventBus;
use crate::realtime::idle::IdleManager;
use crate::smtp::client::SmtpClient;

/// Per-IP key extractor that supports trusted reverse proxies.
///
/// When the peer IP is in the configured `trusted_proxies` list, the
/// leftmost `X-Forwarded-For` value is used as the client IP. Otherwise
/// the direct peer IP is used. Falls back to loopback when
/// `ConnectInfo<SocketAddr>` is unavailable (e.g. in unit tests).
#[derive(Debug, Clone)]
struct ProxyAwareIpExtractor {
    trusted_proxies: Vec<IpAddr>,
}

impl KeyExtractor for ProxyAwareIpExtractor {
    type Key = IpAddr;

    fn extract<T>(&self, req: &Request<T>) -> Result<Self::Key, GovernorError> {
        let peer_ip = req
            .extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .map(|ci| ci.0.ip())
            .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST));

        if self.trusted_proxies.contains(&peer_ip)
            && let Some(forwarded) = req.headers().get("x-forwarded-for").and_then(|v| v.to_str().ok())
            && let Some(first) = forwarded.split(',').next()
            && let Ok(client_ip) = first.trim().parse::<IpAddr>()
        {
            return Ok(client_ip);
        }

        Ok(peer_ip)
    }
}

/// SPA fallback handler that tries `{path}.html` before serving `index.html`.
///
/// Next.js static export produces files like `login.html` for `/login` routes,
/// but `ServeDir` doesn't try the `.html` extension automatically. This handler
/// runs for any request that doesn't match a static file, mapping clean URLs
/// (e.g. `/login` → `login.html`) with `index.html` as the final catch-all for
/// client-side routing.
async fn spa_fallback(
    uri: axum::http::Uri,
    Extension(config): Extension<Arc<AppConfig>>,
) -> axum::response::Response {
    use axum::http::{header, StatusCode};
    use axum::response::IntoResponse;

    let static_dir = Path::new(&config.static_dir);
    let path = uri.path().trim_start_matches('/');

    // Try {path}.html (e.g. /login -> login.html)
    if !path.is_empty() && !path.contains('.') {
        let html_path = static_dir.join(format!("{path}.html"));
        if html_path.is_file()
            && let Ok(contents) = tokio::fs::read(&html_path).await
        {
            return (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                contents,
            )
                .into_response();
        }
    }

    // Fall back to index.html (SPA catch-all)
    let index_path = static_dir.join("index.html");
    match tokio::fs::read(&index_path).await {
        Ok(contents) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
            contents,
        )
            .into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
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
        .key_extractor(ProxyAwareIpExtractor {
            trusted_proxies: config.parsed_trusted_proxies(),
        })
        .period(Duration::from_secs(12))
        .burst_size(5)
        .finish()
        .expect("valid governor config");

    // Public auth route: GovernorLayer (outermost) -> CSRF -> handler.
    let public_auth = Router::new()
        .route("/login", post(auth::login))
        .layer(middleware::from_fn(csrf_protection))
        .layer(GovernorLayer::new(governor_conf));

    // Browser-bound routes (no auth_guard, no rate limit).
    // These routes only need the browser cookie, not full auth.
    let browser_routes = Router::new()
        .route("/accounts", get(auth::list_accounts))
        .route("/accounts/{id}", delete(auth::remove_account).layer(middleware::from_fn(csrf_protection)));

    // Protected auth routes (auth_guard + CSRF).
    let protected_auth = Router::new()
        .route("/session", get(auth::get_session))
        .route("/logout", post(auth::logout))
        .layer(middleware::from_fn(auth_guard))
        .layer(middleware::from_fn(csrf_protection));

    let auth_router = Router::new()
        .merge(public_auth)
        .merge(browser_routes)
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
            "/contact-groups",
            get(contact_groups::list_groups_handler).post(contact_groups::create_group_handler),
        )
        .route(
            "/contact-groups/{id}",
            put(contact_groups::update_group_handler).delete(contact_groups::delete_group_handler),
        )
        .route(
            "/contact-groups/{id}/members",
            get(contact_groups::list_members_handler).post(contact_groups::add_member_handler),
        )
        .route(
            "/contact-groups/{id}/members/{contact_id}",
            delete(contact_groups::remove_member_handler),
        )
        .route(
            "/tags",
            get(tags::list_tags_handler).post(tags::create_tag_handler),
        )
        .route(
            "/tags/{id}",
            put(tags::update_tag_handler).delete(tags::delete_tag_handler),
        )
        .route(
            "/tags/{id}/messages",
            post(tags::tag_message_handler).get(tags::list_tag_messages_handler),
        )
        .route(
            "/tags/{id}/messages/bulk",
            post(tags::bulk_tag_handler),
        )
        .route(
            "/tags/{id}/messages/{folder}/{uid}",
            delete(tags::untag_message_handler),
        )
        .route(
            "/messages/{folder}/{uid}/tags",
            get(tags::get_message_tags_handler),
        )
        .route(
            "/contacts",
            get(contacts::list_contacts_handler).post(contacts::create_contact_handler),
        )
        .route("/contacts/export", get(contacts::export_contacts_handler))
        .route("/contacts/import", post(contacts::import_contacts_handler))
        .route(
            "/contacts/autocomplete/all",
            get(contacts::autocomplete_all_handler),
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
            "/contacts/{id}/export",
            get(contacts::export_single_contact_handler),
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
            "/settings/display",
            get(display_preferences::get_display_preferences)
                .put(display_preferences::update_display_preferences),
        )
        .route(
            "/settings/notifications",
            get(notification_preferences::get_notification_preferences)
                .put(notification_preferences::update_notification_preferences),
        )
        .route(
            "/calendar/events",
            get(calendar::list_events).post(calendar::create_event),
        )
        .route(
            "/calendar/events/import-ics",
            post(calendar::import_ics),
        )
        .route(
            "/calendar/events/{id}",
            get(calendar::get_event)
                .put(calendar::update_event)
                .delete(calendar::delete_event),
        )
        .route(
            "/calendar/settings",
            get(calendar::get_calendar_settings)
                .put(calendar::update_calendar_settings),
        )
        .route(
            "/calendar/meeting-templates",
            get(calendar::list_meeting_templates)
                .post(calendar::create_meeting_template),
        )
        .route(
            "/calendar/meeting-templates/{id}",
            delete(calendar::delete_meeting_template),
        )
        .route("/quota", get(quota::get_quota))
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

    let inner = if config.serve_static {
        let static_service = ServeDir::new(&config.static_dir);
        let spa_router = Router::new()
            .fallback(spa_fallback)
            .layer(Extension(config.clone()));
        Router::new()
            .nest("/api", api_router)
            .fallback_service(static_service.fallback(spa_router))
    } else {
        Router::new().nest("/api", api_router)
    };

    // If BASE_PATH is set (e.g. "/oxi"), nest the entire app under that prefix.
    let router = match config.base_path.as_deref() {
        Some(bp) if !bp.is_empty() => Router::new().nest(bp, inner),
        _ => inner,
    };

    let router = router
        .layer(Extension(idle_manager))
        .layer(Extension(event_bus))
        .layer(Extension(smtp_client))
        .layer(Extension(search_engine))
        .layer(Extension(imap_client))
        .layer(Extension(store))
        .layer(Extension(config.clone()))
        .layer(TraceLayer::new_for_http());

    if config.environment == "development" {
        if let Some(ref origin) = config.cors_origin {
            let cors = CorsLayer::new()
                .allow_origin(origin.parse::<HeaderValue>().unwrap())
                .allow_credentials(true)
                .allow_methods([
                    Method::GET,
                    Method::POST,
                    Method::PUT,
                    Method::PATCH,
                    Method::DELETE,
                ])
                .allow_headers([
                    header::CONTENT_TYPE,
                    HeaderName::from_static("x-requested-with"),
                    HeaderName::from_static("x-active-account"),
                ]);
            router.layer(cors)
        } else {
            router.layer(CorsLayer::permissive())
        }
    } else {
        router
    }
}

#[cfg(test)]
mod tests;
