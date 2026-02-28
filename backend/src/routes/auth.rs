use std::sync::Arc;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::Deserialize;

use crate::auth::imap_auth::{self, AuthResult};
use crate::auth::middleware::SESSION_COOKIE;
use crate::auth::session::{SessionState, SessionStore};
use crate::auth::user_data;
use crate::config::AppConfig;
use crate::db;

/// JSON body expected on `POST /api/auth/login`.
#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
    #[serde(default)]
    pub remember: bool,
}


/// Build a `Set-Cookie` header value for the session cookie.
fn session_cookie(token: &str, max_age_secs: u64, secure: bool) -> String {
    let secure_flag = if secure { " Secure;" } else { "" };
    format!(
        "{}={};{} HttpOnly; SameSite=Strict; Path=/; Max-Age={}",
        SESSION_COOKIE, token, secure_flag, max_age_secs
    )
}

/// Build a `Set-Cookie` header value that clears the session cookie.
fn clearing_cookie(secure: bool) -> String {
    let secure_flag = if secure { " Secure;" } else { "" };
    format!(
        "{}=;{} HttpOnly; SameSite=Strict; Path=/; Max-Age=0",
        SESSION_COOKIE, secure_flag
    )
}

/// Extract the session token from the `Cookie` header.
fn extract_session_token(headers: &axum::http::HeaderMap) -> Option<String> {
    for value in headers.get_all("cookie") {
        let Ok(header_str) = value.to_str() else {
            continue;
        };
        for segment in header_str.split(';') {
            let trimmed = segment.trim();
            if let Some(token) = trimmed.strip_prefix(&format!("{SESSION_COOKIE}=")) {
                let token = token.trim();
                if !token.is_empty() {
                    return Some(token.to_string());
                }
            }
        }
    }
    None
}

/// `POST /api/auth/login`
///
/// Validates the user's credentials against the configured IMAP server.
/// On success, creates a session, provisions the user's data directory,
/// and returns a session cookie.
pub async fn login(
    Extension(store): Extension<Arc<SessionStore>>,
    Extension(config): Extension<Arc<AppConfig>>,
    Json(body): Json<LoginRequest>,
) -> Response {
    // Validate fields not empty.
    if body.email.trim().is_empty() || body.password.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            [("content-type", "application/json")],
            serde_json::json!({
                "error": {
                    "code": "BAD_REQUEST",
                    "message": "Email and password are required",
                    "status": 400
                }
            })
            .to_string(),
        )
            .into_response();
    }

    // Check that IMAP host is configured.
    let imap_host = match &config.imap_host {
        Some(host) => host.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                [("content-type", "application/json")],
                serde_json::json!({
                    "error": {
                        "code": "SERVICE_UNAVAILABLE",
                        "message": "Mail server not configured",
                        "status": 503
                    }
                })
                .to_string(),
            )
                .into_response();
        }
    };

    // Validate credentials against IMAP server.
    let result = imap_auth::validate_imap_credentials(
        &imap_host,
        config.imap_port,
        config.tls_enabled,
        &body.email,
        &body.password,
    )
    .await;

    match result {
        AuthResult::Success => {
            // Hash email and provision user data directory.
            let user_hash = user_data::hash_email(&body.email);
            if let Err(e) = user_data::provision_user_data(&config.data_dir, &user_hash) {
                tracing::error!(error = %e, "failed to provision user data directory");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    [("content-type", "application/json")],
                    serde_json::json!({
                        "error": {
                            "code": "INTERNAL_ERROR",
                            "message": "Failed to provision user data",
                            "status": 500
                        }
                    })
                    .to_string(),
                )
                    .into_response();
            }

            // Auto-create a default identity if the user doesn't have one yet.
            if let Ok(conn) = db::pool::open_user_db(&config.data_dir, &user_hash) {
                match db::identities::has_identities(&conn) {
                    Ok(false) => {
                        let default_identity = db::identities::CreateIdentity {
                            email: body.email.clone(),
                            display_name: String::new(),
                            signature_html: String::new(),
                            is_default: true,
                        };
                        if let Err(e) = db::identities::create_identity(&conn, &default_identity) {
                            tracing::warn!(error = %e, "Failed to auto-create default identity");
                        }
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "Failed to check for existing identities");
                    }
                    _ => {} // Already has identities, nothing to do.
                }
            }

            // Create session.
            const REMEMBER_ME_HOURS: u64 = 30 * 24; // 30 days
            let session_hours = if body.remember {
                REMEMBER_ME_HOURS
            } else {
                config.session_timeout_hours
            };
            let timeout_override = if body.remember {
                Some(std::time::Duration::from_secs(session_hours * 3600))
            } else {
                None
            };
            let token = store.insert(body.email.clone(), body.password, user_hash, timeout_override);
            let max_age = session_hours * 3600;
            let secure = config.environment != "development";
            let cookie = session_cookie(&token, max_age, secure);

            (
                StatusCode::CREATED,
                [
                    ("content-type", "application/json"),
                    ("set-cookie", &cookie),
                ],
                serde_json::json!({ "user": { "email": body.email } }).to_string(),
            )
                .into_response()
        }
        AuthResult::InvalidCredentials => (
            StatusCode::UNAUTHORIZED,
            [("content-type", "application/json")],
            serde_json::json!({
                "error": {
                    "code": "UNAUTHORIZED",
                    "message": "Invalid email or password",
                    "status": 401
                }
            })
            .to_string(),
        )
            .into_response(),
        AuthResult::ServerUnreachable(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            [("content-type", "application/json")],
            serde_json::json!({
                "error": {
                    "code": "SERVICE_UNAVAILABLE",
                    "message": "Cannot reach mail server",
                    "status": 503
                }
            })
            .to_string(),
        )
            .into_response(),
    }
}

/// `GET /api/auth/session`
///
/// Returns the current user's session information. Requires authentication
/// (the `auth_guard` middleware injects `SessionState` into extensions).
pub async fn get_session(Extension(session): Extension<SessionState>) -> Response {
    (
        StatusCode::OK,
        [("content-type", "application/json")],
        serde_json::json!({ "user": { "email": session.email } }).to_string(),
    )
        .into_response()
}

/// `POST /api/auth/logout`
///
/// Removes the current session from the store and clears the session cookie.
/// Requires authentication.
pub async fn logout(
    Extension(store): Extension<Arc<SessionStore>>,
    Extension(config): Extension<Arc<AppConfig>>,
    headers: axum::http::HeaderMap,
) -> Response {
    // Extract token from cookie and remove from store.
    if let Some(token) = extract_session_token(&headers) {
        store.remove(&token);
    }

    let secure = config.environment != "development";
    let cookie = clearing_cookie(secure);

    (
        StatusCode::OK,
        [
            ("content-type", "application/json"),
            ("set-cookie", &cookie),
        ],
        serde_json::json!({ "status": "logged_out" }).to_string(),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;
    use std::time::Duration;

    #[derive(Serialize)]
    struct UserResponse {
        user: UserInfo,
    }

    #[derive(Serialize)]
    struct UserInfo {
        email: String,
    }

    #[derive(Serialize)]
    struct LogoutResponse {
        status: &'static str,
    }

    #[test]
    fn session_cookie_format_secure() {
        let cookie = session_cookie("abc123", 86400, true);
        assert!(cookie.contains("oxi_session=abc123"));
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("Secure"));
        assert!(cookie.contains("SameSite=Strict"));
        assert!(cookie.contains("Path=/"));
        assert!(cookie.contains("Max-Age=86400"));
    }

    #[test]
    fn session_cookie_format_no_secure() {
        let cookie = session_cookie("abc123", 86400, false);
        assert!(cookie.contains("oxi_session=abc123"));
        assert!(cookie.contains("HttpOnly"));
        assert!(!cookie.contains("Secure"));
        assert!(cookie.contains("SameSite=Strict"));
    }

    #[test]
    fn clearing_cookie_format() {
        let cookie = clearing_cookie(true);
        assert!(cookie.contains("oxi_session=;"));
        assert!(cookie.contains("Max-Age=0"));
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("Secure"));
        assert!(cookie.contains("SameSite=Strict"));
        assert!(cookie.contains("Path=/"));
    }

    #[test]
    fn clearing_cookie_format_no_secure() {
        let cookie = clearing_cookie(false);
        assert!(cookie.contains("oxi_session=;"));
        assert!(!cookie.contains("Secure"));
        assert!(cookie.contains("HttpOnly"));
    }

    #[test]
    fn extract_token_from_cookie_header() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert("cookie", "oxi_session=mytoken123".parse().unwrap());
        assert_eq!(
            extract_session_token(&headers),
            Some("mytoken123".to_string())
        );
    }

    #[test]
    fn extract_token_among_multiple_cookies() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            "cookie",
            "theme=dark; oxi_session=abc; lang=en".parse().unwrap(),
        );
        assert_eq!(extract_session_token(&headers), Some("abc".to_string()));
    }

    #[test]
    fn extract_token_missing_returns_none() {
        let headers = axum::http::HeaderMap::new();
        assert_eq!(extract_session_token(&headers), None);
    }

    #[test]
    fn extract_token_wrong_name_returns_none() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert("cookie", "other=value".parse().unwrap());
        assert_eq!(extract_session_token(&headers), None);
    }

    // Integration-style tests for the handlers are covered via the router
    // tests in routes/mod.rs, which mount the full middleware stack.

    #[test]
    fn user_response_serialization() {
        let resp = UserResponse {
            user: UserInfo {
                email: "test@example.com".to_string(),
            },
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["user"]["email"], "test@example.com");
    }

    #[test]
    fn logout_response_serialization() {
        let resp = LogoutResponse {
            status: "logged_out",
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["status"], "logged_out");
    }

    // Verify the SessionStore is used correctly via helper test
    #[test]
    fn store_insert_and_remove_roundtrip() {
        let store = SessionStore::new(Duration::from_secs(3600));
        let token = store.insert(
            "user@test.com".to_string(),
            "pass".to_string(),
            "hash".to_string(),
            None,
        );
        assert!(store.get(&token).is_some());
        store.remove(&token);
        assert!(store.get(&token).is_none());
    }
}
