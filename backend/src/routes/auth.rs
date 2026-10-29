use std::sync::Arc;

use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::Deserialize;

use crate::auth::imap_auth::{self, AuthResult};
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

fn same_site_policy(config: &AppConfig) -> &'static str {
    if config.environment == "development" && config.cors_origin.is_some() {
        "Lax"
    } else {
        "Strict"
    }
}

fn cookie_path(config: &AppConfig) -> &str {
    config.normalized_base_path()
}

fn browser_cookie(
    browser_id: &str,
    max_age_secs: u64,
    secure: bool,
    same_site: &str,
    path: &str,
) -> String {
    let secure_flag = if secure { " Secure;" } else { "" };
    format!(
        "{}={};{} SameSite={}; Path={}; Max-Age={}",
        BROWSER_COOKIE, browser_id, secure_flag, same_site, path, max_age_secs
    )
}

fn account_session_cookie(
    account_id: &str,
    token: &str,
    max_age_secs: u64,
    secure: bool,
    same_site: &str,
    path: &str,
) -> String {
    let secure_flag = if secure { " Secure;" } else { "" };
    format!(
        "oxi_session_{}={};{} HttpOnly; SameSite={}; Path={}; Max-Age={}",
        account_id, token, secure_flag, same_site, path, max_age_secs
    )
}

fn clearing_browser_cookie(secure: bool, same_site: &str, path: &str) -> String {
    let secure_flag = if secure { " Secure;" } else { "" };
    format!(
        "{}=;{} HttpOnly; SameSite={}; Path={}; Max-Age=0",
        BROWSER_COOKIE, secure_flag, same_site, path
    )
}

fn clearing_account_cookie(account_id: &str, secure: bool, same_site: &str, path: &str) -> String {
    let secure_flag = if secure { " Secure;" } else { "" };
    format!(
        "oxi_session_{}=;{} HttpOnly; SameSite={}; Path={}; Max-Age=0",
        account_id, secure_flag, same_site, path
    )
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

fn extract_session_cookies(headers: &axum::http::HeaderMap) -> Vec<(String, String)> {
    let prefix = "oxi_session_";
    let mut sessions = Vec::new();

    for cookie in headers
        .get_all("cookie")
        .iter()
        .filter_map(|v| v.to_str().ok())
    {
        for segment in cookie.split(';') {
            let trimmed = segment.trim();
            if let Some(rest) = trimmed.strip_prefix(prefix)
                && let Some(eq_pos) = rest.find('=')
            {
                let account_id = &rest[..eq_pos];
                let token = &rest[eq_pos + 1..];
                if !account_id.is_empty() && !token.is_empty() {
                    sessions.push((account_id.to_string(), token.to_string()));
                }
            }
        }
    }
    sessions
}

/// `POST /api/auth/login`
///
/// Validates the user's credentials against the configured IMAP server.
/// On success, creates a session, provisions the user's data directory,
/// and returns session cookies.
pub async fn login(
    Extension(store): Extension<Arc<SessionStore>>,
    Extension(config): Extension<Arc<AppConfig>>,
    headers: axum::http::HeaderMap,
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
            let provided_browser_id = body
                .browser_id
                .clone()
                .or_else(|| extract_browser_id(&headers));

            if let Some(ref browser_id) = provided_browser_id {
                let existing_accounts = store.get_browser_accounts(browser_id);
                let duplicate = existing_accounts
                    .iter()
                    .find(|a| a.email == body.email && a.imap_host == imap_host);

                if let Some(dup) = duplicate {
                    tracing::debug!(
                        email = %body.email,
                        imap_host = %imap_host,
                        existing_account_id = %dup.account_id,
                        "login: duplicate account detected"
                    );
                    return (
                        StatusCode::CONFLICT,
                        [("content-type", "application/json")],
                        serde_json::json!({
                            "error": {
                                "code": "DUPLICATE_ACCOUNT",
                                "message": "This account is already logged in",
                                "status": 409
                            }
                        })
                        .to_string(),
                    )
                        .into_response();
                }
            }

            let user_hash = user_data::hash_email(&body.email);
            if let Err(e) = user_data::provision_user_data(&config.data_dir, &user_hash).await {
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

            let database = match config.user_db(&user_hash).await {
                Ok(database) => database,
                Err(error) => {
                    tracing::error!(%error, "Failed to initialize user database");
                    return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({
                        "error": { "code": "SERVICE_UNAVAILABLE", "message": "User database unavailable", "status": 503 }
                    }))).into_response();
                }
            };
            let initialized = async {
                let transaction = database.begin_write().await?;
                if !db::identities::has_identities(&transaction).await? {
                    db::identities::create_identity(
                        &transaction,
                        &db::identities::CreateIdentity {
                            email: body.email.clone(),
                            display_name: String::new(),
                            signature_html: String::new(),
                            is_default: true,
                        },
                    )
                    .await?;
                }
                transaction.commit().await
            }
            .await;
            if let Err(error) = initialized {
                tracing::error!(%error, "Failed to initialize sender identity");
                return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({
                    "error": { "code": "SERVICE_UNAVAILABLE", "message": "User database unavailable", "status": 503 }
                }))).into_response();
            }

            const REMEMBER_ME_HOURS: u64 = 30 * 24;
            let session_hours = if body.remember {
                REMEMBER_ME_HOURS
            } else {
                config.session_timeout_hours
            };

            let browser_id = provided_browser_id
                .clone()
                .unwrap_or_else(|| store.create_browser());
            tracing::debug!(
                email = %body.email,
                provided_browser_id = ?provided_browser_id,
                effective_browser_id = %browser_id,
                "login: creating session"
            );

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
            let same_site = same_site_policy(&config);
            let path = cookie_path(&config);
            let browser_cookie_header =
                browser_cookie(&browser_id, max_age, secure, same_site, path);
            let session_cookie_header =
                account_session_cookie(&account_id, &token, max_age, secure, same_site, path);

            let response = (
                StatusCode::CREATED,
                [("content-type", "application/json")],
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
                .into_response();

            let mut response = response;
            let headers = response.headers_mut();
            if let Ok(header_value) = browser_cookie_header.parse() {
                headers.append("set-cookie", header_value);
            }
            if let Ok(header_value) = session_cookie_header.parse() {
                headers.append("set-cookie", header_value);
            }

            response
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
    Extension(config): Extension<Arc<AppConfig>>,
    headers: axum::http::HeaderMap,
) -> Response {
    let secure = config.environment != "development";
    let same_site = same_site_policy(&config);

    let browser_id = extract_browser_id(&headers);

    let Some(browser_id) = browser_id else {
        tracing::debug!("list_accounts: no browser_id cookie found");
        return (
            StatusCode::OK,
            [("content-type", "application/json")],
            serde_json::json!({ "accounts": [], "browserSessionExpired": true }).to_string(),
        )
            .into_response();
    };

    let browser_exists = store.browser_exists(&browser_id);
    if !browser_exists {
        tracing::debug!(browser_id = %browser_id, "list_accounts: browser_id not found on server");
        let clear_browser = clearing_browser_cookie(secure, same_site, cookie_path(&config));
        return (
            StatusCode::OK,
            [
                ("content-type", "application/json"),
                ("set-cookie", &clear_browser),
            ],
            serde_json::json!({ "accounts": [], "browserSessionExpired": true }).to_string(),
        )
            .into_response();
    }

    let session_cookies = extract_session_cookies(&headers);
    let session_map: std::collections::HashMap<String, String> =
        session_cookies.into_iter().collect();

    let browser_accounts = store.get_browser_accounts(&browser_id);
    let browser_account_ids: Vec<String> = browser_accounts
        .iter()
        .map(|s| s.account_id.clone())
        .collect();

    let mut valid_accounts: Vec<serde_json::Value> = Vec::new();
    let mut clear_cookies: Vec<String> = Vec::new();

    for session in &browser_accounts {
        if let Some(token) = session_map.get(&session.account_id) {
            if let Some(valid_session) = store.get(token) {
                valid_accounts.push(serde_json::json!({
                    "id": valid_session.account_id,
                    "email": valid_session.email,
                    "imapHost": valid_session.imap_host,
                    "smtpHost": valid_session.smtp_host
                }));
            } else {
                store.remove_account(&session.account_id);
                clear_cookies.push(clearing_account_cookie(
                    &session.account_id,
                    secure,
                    same_site,
                    cookie_path(&config),
                ));
            }
        } else {
            store.remove_account(&session.account_id);
        }
    }

    for account_id in session_map.keys() {
        if !browser_account_ids.contains(account_id) {
            clear_cookies.push(clearing_account_cookie(
                account_id,
                secure,
                same_site,
                cookie_path(&config),
            ));
        }
    }

    if valid_accounts.is_empty() {
        tracing::debug!(
            browser_id = %browser_id,
            "list_accounts: no valid accounts found"
        );
        let clear_browser = clearing_browser_cookie(secure, same_site, cookie_path(&config));
        let body = serde_json::json!({ "accounts": [], "browserSessionExpired": true }).to_string();

        let mut response = axum::response::Response::new(axum::body::Body::from(body));
        *response.status_mut() = StatusCode::OK;
        response.headers_mut().insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/json"),
        );
        response.headers_mut().append(
            axum::http::header::SET_COOKIE,
            axum::http::HeaderValue::from_str(&clear_browser).unwrap(),
        );
        for cookie in clear_cookies {
            response.headers_mut().append(
                axum::http::header::SET_COOKIE,
                axum::http::HeaderValue::from_str(&cookie).unwrap(),
            );
        }
        return response;
    }

    tracing::debug!(
        browser_id = %browser_id,
        account_count = valid_accounts.len(),
        accounts = ?valid_accounts.iter().filter_map(|a| a.get("email").and_then(|e| e.as_str())).collect::<Vec<_>>(),
        "list_accounts: returning valid accounts"
    );

    let body = serde_json::json!({ "accounts": valid_accounts }).to_string();
    let mut response = axum::response::Response::new(axum::body::Body::from(body));
    *response.status_mut() = StatusCode::OK;
    response.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("application/json"),
    );
    for cookie in clear_cookies {
        response.headers_mut().append(
            axum::http::header::SET_COOKIE,
            axum::http::HeaderValue::from_str(&cookie).unwrap(),
        );
    }
    response
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
    let account_belongs_to_browser = browser_accounts.iter().any(|s| s.account_id == account_id);

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
    let same_site = same_site_policy(&config);
    let cookie = clearing_account_cookie(&account_id, secure, same_site, cookie_path(&config));

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

    let secure = config.environment != "development";
    let same_site = same_site_policy(&config);

    if let Some(ref browser_id) = browser_id {
        let accounts = store.get_browser_accounts(browser_id);
        for account in accounts {
            cookies_to_clear.push(clearing_account_cookie(
                &account.account_id,
                secure,
                same_site,
                cookie_path(&config),
            ));
        }
        store.remove_browser(browser_id);
    }

    cookies_to_clear.push(clearing_browser_cookie(
        secure,
        same_site,
        cookie_path(&config),
    ));

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

    fn test_config(environment: &str, cors_origin: Option<&str>) -> AppConfig {
        AppConfig {
            host: "127.0.0.1".to_string(),
            port: 3001,
            imap_host: None,
            imap_port: 993,
            smtp_host: None,
            smtp_port: 587,
            tls_enabled: true,
            data_dir: "/data".to_string(),
            database_url: None,
            database_max_connections: 10,
            database: None,
            session_timeout_hours: 24,
            static_dir: "./static".to_string(),
            environment: environment.to_string(),
            base_path: None,
            serve_static: true,
            cors_origin: cors_origin.map(str::to_string),
            trusted_proxies: None,
        }
    }

    #[tokio::test]
    async fn login_reuses_scoped_browser_cookie_when_body_omits_browser_id() {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let imap = tokio::spawn(async move {
            for _ in 0..3 {
                let (stream, _) = listener.accept().await.unwrap();
                let mut stream = BufReader::new(stream);
                stream
                    .get_mut()
                    .write_all(b"* OK test IMAP ready\r\n")
                    .await
                    .unwrap();
                let mut command = String::new();
                stream.read_line(&mut command).await.unwrap();
                let tag = command.split_whitespace().next().unwrap();
                let response = format!("{tag} OK LOGIN completed\r\n");
                stream
                    .get_mut()
                    .write_all(response.as_bytes())
                    .await
                    .unwrap();
                command.clear();
                stream.read_line(&mut command).await.unwrap();
                let tag = command.split_whitespace().next().unwrap();
                let response = format!("* BYE logging out\r\n{tag} OK LOGOUT completed\r\n");
                stream
                    .get_mut()
                    .write_all(response.as_bytes())
                    .await
                    .unwrap();
            }
        });

        let data_dir = tempfile::tempdir().unwrap();
        let mut config = test_config("development", Some("http://localhost:3000"));
        config.base_path = Some("/oxi".to_string());
        config.data_dir = data_dir.path().to_str().unwrap().to_string();
        let config = Arc::new(config);
        let store = Arc::new(SessionStore::new(std::time::Duration::from_secs(3600)));
        let request = |email: &str| LoginRequest {
            email: email.to_string(),
            password: "password".to_string(),
            remember: false,
            imap_host: Some("127.0.0.1".to_string()),
            imap_port: Some(port),
            imap_tls: false,
            smtp_host: Some("127.0.0.1".to_string()),
            smtp_port: None,
            smtp_tls: false,
            browser_id: None,
        };

        let first = login(
            Extension(store.clone()),
            Extension(config.clone()),
            axum::http::HeaderMap::new(),
            Json(request("first@example.com")),
        )
        .await;
        assert_eq!(first.status(), StatusCode::CREATED);
        let cookie = first
            .headers()
            .get_all("set-cookie")
            .iter()
            .map(|value| value.to_str().unwrap())
            .find(|value| value.starts_with("oxi_browser="))
            .unwrap();
        assert!(cookie.contains("Path=/oxi;"));
        let cookie_pair = cookie.split(';').next().unwrap();
        let browser_id = cookie_pair.strip_prefix("oxi_browser=").unwrap();
        let mut headers = axum::http::HeaderMap::new();
        headers.insert("cookie", cookie_pair.parse().unwrap());

        let second = login(
            Extension(store.clone()),
            Extension(config.clone()),
            headers.clone(),
            Json(request("second@example.com")),
        )
        .await;
        assert_eq!(second.status(), StatusCode::CREATED);
        assert!(second.headers().get_all("set-cookie").iter().any(|value| {
            value
                .to_str()
                .unwrap()
                .starts_with(&format!("{cookie_pair};"))
        }));
        let accounts = store.get_browser_accounts(browser_id);
        assert_eq!(accounts.len(), 2);
        assert!(
            accounts
                .iter()
                .any(|account| account.email == "first@example.com")
        );
        assert!(
            accounts
                .iter()
                .any(|account| account.email == "second@example.com")
        );

        let duplicate = login(
            Extension(store.clone()),
            Extension(config),
            headers,
            Json(request("second@example.com")),
        )
        .await;
        assert_eq!(duplicate.status(), StatusCode::CONFLICT);
        assert_eq!(store.get_browser_accounts(browser_id).len(), 2);
        imap.await.unwrap();
    }

    #[test]
    fn issued_and_clearing_cookies_use_the_normalized_base_path() {
        for (base_path, expected_path) in [
            (None, "/"),
            (Some(""), "/"),
            (Some("/"), "/"),
            (Some("/oxi"), "/oxi"),
        ] {
            let mut config = test_config("development", None);
            config.base_path = base_path.map(str::to_string);
            let path = cookie_path(&config);
            let cookies = [
                browser_cookie("browser", 3600, false, "Strict", path),
                account_session_cookie("account", "token", 3600, false, "Strict", path),
                clearing_browser_cookie(false, "Strict", path),
                clearing_account_cookie("account", false, "Strict", path),
            ];

            for cookie in cookies {
                let path_attribute = cookie
                    .split(';')
                    .map(str::trim)
                    .find_map(|attribute| attribute.strip_prefix("Path="));
                assert_eq!(
                    path_attribute,
                    Some(expected_path),
                    "{base_path:?}: {cookie}"
                );
            }
        }
    }

    #[test]
    fn same_site_is_lax_for_development_cross_origin() {
        let config = test_config("development", Some("http://localhost:3000"));
        assert_eq!(same_site_policy(&config), "Lax");

        let cookie = browser_cookie(
            "browser",
            3600,
            false,
            same_site_policy(&config),
            cookie_path(&config),
        );
        assert!(cookie.contains("SameSite=Lax"));
    }

    #[test]
    fn same_site_stays_strict_outside_development() {
        let config = test_config("production", Some("https://app.example.com"));
        assert_eq!(same_site_policy(&config), "Strict");

        let cookie = browser_cookie(
            "browser",
            3600,
            true,
            same_site_policy(&config),
            cookie_path(&config),
        );
        assert!(cookie.contains("SameSite=Strict"));
    }
}
