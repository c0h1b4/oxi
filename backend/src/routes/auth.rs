use std::sync::Arc;

use axum::extract::Path;
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

pub const BROWSER_COOKIE: &str = "oxi_browser";

/// JSON body expected on `POST /api/auth/login`.
#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
    #[serde(default)]
    pub remember: bool,
    pub imap_host: Option<String>,
    pub imap_port: Option<u16>,
    #[serde(default = "default_tls")]
    pub imap_tls: bool,
    pub smtp_host: Option<String>,
    pub smtp_port: Option<u16>,
    #[serde(default = "default_tls")]
    pub smtp_tls: bool,
    pub browser_id: Option<String>,
}

fn default_tls() -> bool {
    true
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

fn browser_cookie(browser_id: &str, max_age_secs: u64, secure: bool) -> String {
    let secure_flag = if secure { " Secure;" } else { "" };
    format!(
        "{}={};{} HttpOnly; SameSite=Strict; Path=/; Max-Age={}",
        BROWSER_COOKIE, browser_id, secure_flag, max_age_secs
    )
}

fn account_session_cookie(account_id: &str, token: &str, max_age_secs: u64, secure: bool) -> String {
    let secure_flag = if secure { " Secure;" } else { "" };
    format!(
        "oxi_session_{}={};{} HttpOnly; SameSite=Strict; Path=/; Max-Age={}",
        account_id, token, secure_flag, max_age_secs
    )
}

fn clearing_browser_cookie(secure: bool) -> String {
    let secure_flag = if secure { " Secure;" } else { "" };
    format!(
        "{}=;{} HttpOnly; SameSite=Strict; Path=/; Max-Age=0",
        BROWSER_COOKIE, secure_flag
    )
}

fn clearing_account_cookie(account_id: &str, secure: bool) -> String {
    let secure_flag = if secure { " Secure;" } else { "" };
    format!(
        "oxi_session_{}=;{} HttpOnly; SameSite=Strict; Path=/; Max-Age=0",
        account_id, secure_flag
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

fn extract_browser_id(headers: &axum::http::HeaderMap) -> Option<String> {
    headers
        .get_all("cookie")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find_map(|cookie| {
            for segment in cookie.split(';') {
                let trimmed = segment.trim();
                if let Some(id) = trimmed.strip_prefix("oxi_browser=") {
                    let id = id.trim();
                    if !id.is_empty() {
                        return Some(id.to_string());
                    }
                }
            }
            None
        })
}

/// `POST /api/auth/login`
///
/// Validates the user's credentials against the configured IMAP server.
/// On success, creates a session, provisions the user's data directory,
/// and returns session cookies.
pub async fn login(
    Extension(store): Extension<Arc<SessionStore>>,
    Extension(config): Extension<Arc<AppConfig>>,
    Json(body): Json<LoginRequest>,
) -> Response {
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

    let imap_host = body.imap_host.clone().or(config.imap_host.clone());
    let Some(imap_host) = imap_host else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            [("content-type", "application/json")],
            serde_json::json!({
                "error": {
                    "code": "SERVICE_UNAVAILABLE",
                    "message": "IMAP server not configured",
                    "status": 503
                }
            })
            .to_string(),
        )
            .into_response();
    };

    let smtp_host = body.smtp_host.clone().or(config.smtp_host.clone());
    let smtp_host = smtp_host.unwrap_or_else(|| imap_host.clone());

    let imap_port = body.imap_port.unwrap_or(config.imap_port);
    let imap_tls = body.imap_tls;
    let smtp_port = body.smtp_port.unwrap_or(config.smtp_port);
    let smtp_tls = body.smtp_tls;

    let result = imap_auth::validate_imap_credentials(
        &imap_host,
        imap_port,
        imap_tls,
        &body.email,
        &body.password,
    )
    .await;

    match result {
        AuthResult::Success => {
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
                    _ => {}
                }
            }

            const REMEMBER_ME_HOURS: u64 = 30 * 24;
            let session_hours = if body.remember {
                REMEMBER_ME_HOURS
            } else {
                config.session_timeout_hours
            };

            let browser_id = body.browser_id.unwrap_or_else(|| store.create_browser());

            let (token, account_id) = store.add_account_to_browser(
                &browser_id,
                body.email.clone(),
                body.password,
                user_hash,
                imap_host.clone(),
                imap_port,
                imap_tls,
                smtp_host.clone(),
                smtp_port,
                smtp_tls,
            );

            let max_age = session_hours * 3600;
            let secure = config.environment != "development";
            let browser_cookie_header = browser_cookie(&browser_id, max_age, secure);
            let session_cookie_header = account_session_cookie(&account_id, &token, max_age, secure);

            (
                StatusCode::CREATED,
                [
                    ("content-type", "application/json"),
                    ("set-cookie", &browser_cookie_header),
                    ("set-cookie", &session_cookie_header),
                ],
                serde_json::json!({
                    "account": {
                        "id": account_id,
                        "email": body.email,
                        "imapHost": imap_host,
                        "smtpHost": smtp_host
                    }
                })
                .to_string(),
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
        AuthResult::ServerUnreachable(msg) => (
            StatusCode::SERVICE_UNAVAILABLE,
            [("content-type", "application/json")],
            serde_json::json!({
                "error": {
                    "code": "SERVER_UNREACHABLE",
                    "message": msg,
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

pub async fn list_accounts(
    Extension(store): Extension<Arc<SessionStore>>,
    Extension(_config): Extension<Arc<AppConfig>>,
    headers: axum::http::HeaderMap,
) -> Response {
    let browser_id = extract_browser_id(&headers);

    let Some(browser_id) = browser_id else {
        let empty_accounts: Vec<serde_json::Value> = vec![];
        return (
            StatusCode::OK,
            [("content-type", "application/json")],
            serde_json::json!({ "accounts": empty_accounts }).to_string(),
        )
            .into_response();
    };

    let accounts: Vec<serde_json::Value> = store
        .get_browser_accounts(&browser_id)
        .into_iter()
        .map(|session| {
            serde_json::json!({
                "id": session.account_id,
                "email": session.email,
                "imapHost": session.imap_host,
                "smtpHost": session.smtp_host
            })
        })
        .collect();

    (
        StatusCode::OK,
        [("content-type", "application/json")],
        serde_json::json!({ "accounts": accounts }).to_string(),
    )
        .into_response()
}

pub async fn remove_account(
    Extension(store): Extension<Arc<SessionStore>>,
    Extension(config): Extension<Arc<AppConfig>>,
    Path(account_id): Path<String>,
    headers: axum::http::HeaderMap,
) -> Response {
    let browser_id = extract_browser_id(&headers);

    let Some(browser_id) = browser_id else {
        return (
            StatusCode::UNAUTHORIZED,
            [("content-type", "application/json")],
            serde_json::json!({
                "error": {
                    "code": "UNAUTHORIZED",
                    "message": "No browser session",
                    "status": 401
                }
            })
            .to_string(),
        )
            .into_response();
    };

    let browser_accounts = store.get_browser_accounts(&browser_id);
    let account_belongs_to_browser = browser_accounts
        .iter()
        .any(|s| s.account_id == account_id);

    if !account_belongs_to_browser {
        return (
            StatusCode::FORBIDDEN,
            [("content-type", "application/json")],
            serde_json::json!({
                "error": {
                    "code": "FORBIDDEN",
                    "message": "Account does not belong to this browser session",
                    "status": 403
                }
            })
            .to_string(),
        )
            .into_response();
    }

    store.remove_account(&account_id);

    let secure = config.environment != "development";
    let cookie = clearing_account_cookie(&account_id, secure);

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

/// `POST /api/auth/logout`
///
/// Removes the current session from the store and clears the session cookie.
/// Requires authentication.
pub async fn logout(
    Extension(store): Extension<Arc<SessionStore>>,
    Extension(config): Extension<Arc<AppConfig>>,
    headers: axum::http::HeaderMap,
) -> Response {
    let browser_id = extract_browser_id(&headers);

    let mut cookies_to_clear: Vec<String> = Vec::new();

    if let Some(ref browser_id) = browser_id {
        let accounts = store.get_browser_accounts(browser_id);
        for account in accounts {
            cookies_to_clear.push(clearing_account_cookie(
                &account.account_id,
                config.environment != "development",
            ));
        }
        store.remove_browser(browser_id);
    }

    let secure = config.environment != "development";
    cookies_to_clear.push(clearing_browser_cookie(secure));

    let response = (
        StatusCode::OK,
        [("content-type", "application/json")],
        serde_json::json!({ "status": "logged_out" }).to_string(),
    )
        .into_response();

    let mut response = response;
    let headers = response.headers_mut();
    for cookie in cookies_to_clear {
        if let Ok(header_value) = cookie.parse() {
            headers.append("set-cookie", header_value);
        }
    }

    response
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
