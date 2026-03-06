use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use axum::extract::WebSocketUpgrade;
use axum::response::{IntoResponse, Response};
use axum::Extension;
use futures::{SinkExt, StreamExt};
use tokio::sync::broadcast;

use crate::auth::session::{SessionState, SessionStore};
use crate::config::AppConfig;
use crate::imap::client::{ImapClient, ImapCredentials};
use crate::realtime::events::EventBus;
use crate::realtime::idle::IdleManager;
use crate::search::engine::SearchEngine;

/// Extract the session token from the `Cookie` header string.
fn extract_session_token(cookie_header: &str) -> Option<String> {
    for segment in cookie_header.split(';') {
        let trimmed = segment.trim();
        if let Some(token) = trimmed.strip_prefix("oxi_session=") {
            let token = token.trim();
            if !token.is_empty() {
                return Some(token.to_string());
            }
        }
    }
    None
}

/// `GET /api/ws` — WebSocket upgrade handler.
///
/// Authenticates via the session cookie (extracted from the upgrade request
/// headers) and then upgrades to a WebSocket connection that receives
/// real-time mail events.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    headers: axum::http::HeaderMap,
    Extension(store): Extension<Arc<SessionStore>>,
    Extension(config): Extension<Arc<AppConfig>>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Extension(idle_manager): Extension<Arc<IdleManager>>,
    Extension(imap_client): Extension<Arc<dyn ImapClient>>,
    Extension(search_engine): Extension<Arc<SearchEngine>>,
) -> Response {
    // Authenticate from cookie.
    let session = headers
        .get_all("cookie")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find_map(extract_session_token)
        .and_then(|token| store.get(&token));

    let Some(session) = session else {
        return (
            axum::http::StatusCode::UNAUTHORIZED,
            "Missing or invalid session",
        )
            .into_response();
    };

    let imap_creds = config.imap_host.as_ref().map(|host| ImapCredentials {
        host: host.clone(),
        port: config.imap_port,
        tls: config.tls_enabled,
        email: session.email.clone(),
        password: session.password.clone(),
    });

    ws.on_upgrade(move |socket| {
        handle_socket(socket, session, config, event_bus, idle_manager, imap_client, imap_creds, search_engine)
    })
}

/// Handle an authenticated WebSocket connection.
///
/// Subscribes to the user's EventBus channel and forwards events as JSON.
/// Also starts IMAP IDLE for INBOX when the connection is established.
async fn handle_socket(
    socket: WebSocket,
    session: SessionState,
    config: Arc<AppConfig>,
    event_bus: Arc<EventBus>,
    idle_manager: Arc<IdleManager>,
    imap_client: Arc<dyn ImapClient>,
    imap_creds: Option<ImapCredentials>,
    search_engine: Arc<SearchEngine>,
) {
    let user_hash = session.user_hash.clone();

    tracing::info!(user = %session.email, "WebSocket connected");

    // Subscribe to the user's event channel.
    let mut rx = event_bus.subscribe(&user_hash).await;

    // Start IMAP IDLE for INBOX if IMAP is configured.
    // Also start the periodic sync loop for flag/deletion reconciliation.
    let sync_handle = if let Some(ref creds) = imap_creds {
        idle_manager
            .start_idle(
                user_hash.clone(),
                "INBOX".to_string(),
                creds.clone(),
                event_bus.clone(),
                config.clone(),
            )
            .await;

        let handle = tokio::spawn(super::sync::sync_loop(
            user_hash.clone(),
            creds.clone(),
            config,
            imap_client,
            event_bus.clone(),
            search_engine,
        ));
        Some(handle)
    } else {
        None
    };

    let (mut ws_tx, mut ws_rx) = socket.split();

    // Ping interval for keepalive.
    let mut ping_interval = tokio::time::interval(std::time::Duration::from_secs(30));

    loop {
        tokio::select! {
            // Forward events from the bus to the WebSocket client.
            event = rx.recv() => {
                match event {
                    Ok(mail_event) => {
                        if let Ok(json) = serde_json::to_string(&mail_event)
                            && ws_tx.send(Message::Text(json.into())).await.is_err() {
                                break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(user = %session.email, skipped = n, "WebSocket client lagged");
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }

            // Handle incoming messages from the client (or detect disconnect).
            msg = ws_rx.next() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Pong(_))) => {}
                    Some(Ok(_)) => {} // Ignore other messages from client.
                    Some(Err(_)) => break,
                }
            }

            // Send periodic pings.
            _ = ping_interval.tick() => {
                if ws_tx.send(Message::Ping(vec![].into())).await.is_err() {
                    break;
                }
            }
        }
    }

    tracing::info!(user = %session.email, "WebSocket disconnected");

    // Stop the periodic sync task.
    if let Some(handle) = sync_handle {
        handle.abort();
    }

    // Stop IDLE tasks and clean up the event channel.
    idle_manager.stop_all(&user_hash).await;
    event_bus.cleanup(&user_hash).await;
}
