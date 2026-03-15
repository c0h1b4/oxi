use std::sync::Arc;

use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use super::session::SessionStore;

pub const BROWSER_COOKIE: &str = "oxi_browser";
pub const ACTIVE_ACCOUNT_HEADER: &str = "X-Active-Account";

const UNAUTHORIZED_BODY: &str = r#"{"error":{"code":"UNAUTHORIZED","message":"Invalid or expired session","status":401}}"#;

fn unauthorized() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        [("content-type", "application/json")],
        UNAUTHORIZED_BODY,
    )
        .into_response()
}

fn account_expired(account_id: &str) -> Response {
    let body = format!(
        r#"{{"error":{{"code":"ACCOUNT_EXPIRED","message":"Account session has expired","status":401,"account_id":"{}"}}}}"#,
        account_id
    );
    (
        StatusCode::UNAUTHORIZED,
        [("content-type", "application/json")],
        body,
    )
        .into_response()
}

fn extract_cookie_value(req: &Request, cookie_name: &str) -> Option<String> {
    for value in req.headers().get_all("cookie") {
        let Ok(header_str) = value.to_str() else {
            continue;
        };
        for segment in header_str.split(';') {
            let trimmed = segment.trim();
            let prefix = format!("{}=", cookie_name);
            if let Some(cookie_value) = trimmed.strip_prefix(&prefix) {
                let cookie_value = cookie_value.trim();
                if !cookie_value.is_empty() {
                    return Some(cookie_value.to_string());
                }
            }
        }
    }
    None
}

fn extract_session_cookie(req: &Request, account_id: &str) -> Option<String> {
    let cookie_name = format!("oxi_session_{}", account_id);
    extract_cookie_value(req, &cookie_name)
}

fn extract_active_account_header(req: &Request) -> Option<String> {
    req.headers()
        .get(ACTIVE_ACCOUNT_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn extract_active_account_query(req: &Request) -> Option<String> {
    let uri = req.uri();
    let query = uri.query()?;
    for pair in query.split('&') {
        if let Some(value) = pair.strip_prefix("account_id=") {
            let decoded = urlencoding_decode(value);
            if !decoded.is_empty() {
                return Some(decoded);
            }
        }
    }
    None
}

fn urlencoding_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                result.push(byte as char);
            } else {
                result.push('%');
                result.push_str(&hex);
            }
        } else if c == '+' {
            result.push(' ');
        } else {
            result.push(c);
        }
    }
    result
}

pub async fn auth_guard(mut req: Request, next: Next) -> Response {
    let store = match req.extensions().get::<Arc<SessionStore>>() {
        Some(s) => Arc::clone(s),
        None => return unauthorized(),
    };

    let browser_id = match extract_cookie_value(&req, BROWSER_COOKIE) {
        Some(b) => b,
        None => return unauthorized(),
    };

    let account_id = match extract_active_account_header(&req).or_else(|| extract_active_account_query(&req)) {
        Some(a) => a,
        None => return unauthorized(),
    };

    let token = match extract_session_cookie(&req, &account_id) {
        Some(t) => t,
        None => return account_expired(&account_id),
    };

    let session = match store.get_account_session(&browser_id, &account_id, &token) {
        Some(s) => s,
        None => return account_expired(&account_id),
    };

    req.extensions_mut().insert(session);

    next.run(req).await
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::middleware;
    use axum::response::IntoResponse;
    use axum::routing::get;
    use axum::{Extension, Router};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use super::*;
    use crate::auth::session::{AccountSession, SessionStore};

    fn guarded_router(store: Arc<SessionStore>) -> Router {
        let handler = |Extension(session): Extension<AccountSession>| async move {
            serde_json::json!({ "email": session.email }).to_string().into_response()
        };

        Router::new()
            .route("/protected", get(handler))
            .layer(middleware::from_fn(auth_guard))
            .layer(Extension(store))
    }

    async fn send(
        router: Router,
        req: Request<Body>,
    ) -> (StatusCode, serde_json::Value) {
        let resp = router.oneshot(req).await.expect("request should succeed");
        let status = resp.status();
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value =
            serde_json::from_slice(&bytes).expect("body should be valid JSON");
        (status, json)
    }

    struct TestSession {
        browser_id: String,
        account_id: String,
        token: String,
    }

    fn create_test_session(store: &SessionStore, email: &str, password: &str, user_hash: &str) -> TestSession {
        let browser_id = store.create_browser();
        let (token, account_id) = store.add_account_to_browser(
            &browser_id,
            email.to_string(),
            password.to_string(),
            user_hash.to_string(),
            "imap.example.com".to_string(),
            993,
            true,
            "smtp.example.com".to_string(),
            587,
            true,
        );
        TestSession { browser_id, account_id, token }
    }

    fn build_auth_headers(session: &TestSession) -> String {
        format!(
            "oxi_browser={}; oxi_session_{}={}",
            session.browser_id, session.account_id, session.token
        )
    }

    #[tokio::test]
    async fn no_cookies_returns_401() {
        let store = Arc::new(SessionStore::new(Duration::from_secs(3600)));
        let router = guarded_router(store);

        let req = Request::builder()
            .uri("/protected")
            .body(Body::empty())
            .unwrap();

        let (status, json) = send(router, req).await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(json["error"]["code"], "UNAUTHORIZED");
    }

    #[tokio::test]
    async fn missing_browser_cookie_returns_401() {
        let store = Arc::new(SessionStore::new(Duration::from_secs(3600)));
        let session = create_test_session(&store, "alice@example.com", "hunter2", "abc123");
        let router = guarded_router(Arc::clone(&store));

        let req = Request::builder()
            .uri("/protected")
            .header(ACTIVE_ACCOUNT_HEADER, &session.account_id)
            .header("cookie", format!("oxi_session_{}={}", session.account_id, session.token))
            .body(Body::empty())
            .unwrap();

        let (status, json) = send(router, req).await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(json["error"]["code"], "UNAUTHORIZED");
    }

    #[tokio::test]
    async fn missing_active_account_header_returns_401() {
        let store = Arc::new(SessionStore::new(Duration::from_secs(3600)));
        let session = create_test_session(&store, "alice@example.com", "hunter2", "abc123");
        let router = guarded_router(Arc::clone(&store));

        let req = Request::builder()
            .uri("/protected")
            .header("cookie", format!("oxi_browser={}", session.browser_id))
            .body(Body::empty())
            .unwrap();

        let (status, json) = send(router, req).await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(json["error"]["code"], "UNAUTHORIZED");
    }

    #[tokio::test]
    async fn missing_session_cookie_returns_account_expired() {
        let store = Arc::new(SessionStore::new(Duration::from_secs(3600)));
        let session = create_test_session(&store, "alice@example.com", "hunter2", "abc123");
        let router = guarded_router(Arc::clone(&store));

        let req = Request::builder()
            .uri("/protected")
            .header(ACTIVE_ACCOUNT_HEADER, &session.account_id)
            .header("cookie", format!("oxi_browser={}", session.browser_id))
            .body(Body::empty())
            .unwrap();

        let (status, json) = send(router, req).await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(json["error"]["code"], "ACCOUNT_EXPIRED");
        assert_eq!(json["error"]["account_id"], session.account_id);
    }

    #[tokio::test]
    async fn invalid_token_returns_account_expired() {
        let store = Arc::new(SessionStore::new(Duration::from_secs(3600)));
        let session = create_test_session(&store, "alice@example.com", "hunter2", "abc123");
        let router = guarded_router(Arc::clone(&store));

        let req = Request::builder()
            .uri("/protected")
            .header(ACTIVE_ACCOUNT_HEADER, &session.account_id)
            .header("cookie", format!(
                "oxi_browser={}; oxi_session_{}=invalid-token",
                session.browser_id, session.account_id
            ))
            .body(Body::empty())
            .unwrap();

        let (status, json) = send(router, req).await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(json["error"]["code"], "ACCOUNT_EXPIRED");
    }

    #[tokio::test]
    async fn valid_session_returns_200_with_email() {
        let store = Arc::new(SessionStore::new(Duration::from_secs(3600)));
        let session = create_test_session(&store, "alice@example.com", "hunter2", "abc123");
        let router = guarded_router(Arc::clone(&store));

        let req = Request::builder()
            .uri("/protected")
            .header(ACTIVE_ACCOUNT_HEADER, &session.account_id)
            .header("cookie", build_auth_headers(&session))
            .body(Body::empty())
            .unwrap();

        let (status, json) = send(router, req).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["email"], "alice@example.com");
    }

    #[tokio::test]
    async fn wrong_browser_returns_account_expired() {
        let store = Arc::new(SessionStore::new(Duration::from_secs(3600)));
        let session = create_test_session(&store, "alice@example.com", "hunter2", "abc123");
        let wrong_browser_id = store.create_browser();
        let router = guarded_router(Arc::clone(&store));

        let req = Request::builder()
            .uri("/protected")
            .header(ACTIVE_ACCOUNT_HEADER, &session.account_id)
            .header("cookie", format!(
                "oxi_browser={}; oxi_session_{}={}",
                wrong_browser_id, session.account_id, session.token
            ))
            .body(Body::empty())
            .unwrap();

        let (status, json) = send(router, req).await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(json["error"]["code"], "ACCOUNT_EXPIRED");
    }

    #[tokio::test]
    async fn expired_session_returns_account_expired() {
        let store = Arc::new(SessionStore::new(Duration::from_millis(50)));
        let session = create_test_session(&store.as_ref(), "bob@example.com", "pass", "hash");

        thread::sleep(Duration::from_millis(100));

        let router = guarded_router(store);

        let req = Request::builder()
            .uri("/protected")
            .header(ACTIVE_ACCOUNT_HEADER, &session.account_id)
            .header("cookie", build_auth_headers(&session))
            .body(Body::empty())
            .unwrap();

        let (status, json) = send(router, req).await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(json["error"]["code"], "ACCOUNT_EXPIRED");
    }

    #[tokio::test]
    async fn cookies_among_multiple_cookies() {
        let store = Arc::new(SessionStore::new(Duration::from_secs(3600)));
        let session = create_test_session(&store, "multi@example.com", "pass", "hash");
        let router = guarded_router(Arc::clone(&store));

        let req = Request::builder()
            .uri("/protected")
            .header(ACTIVE_ACCOUNT_HEADER, &session.account_id)
            .header(
                "cookie",
                format!(
                    "theme=dark; oxi_browser={}; lang=en; oxi_session_{}={}",
                    session.browser_id, session.account_id, session.token
                ),
            )
            .body(Body::empty())
            .unwrap();

        let (status, json) = send(router, req).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["email"], "multi@example.com");
    }

    #[tokio::test]
    async fn wrong_cookie_name_returns_401() {
        let store = Arc::new(SessionStore::new(Duration::from_secs(3600)));
        let session = create_test_session(&store, "wrong@example.com", "pass", "hash");
        let router = guarded_router(Arc::clone(&store));

        let req = Request::builder()
            .uri("/protected")
            .header(ACTIVE_ACCOUNT_HEADER, &session.account_id)
            .header("cookie", format!(
                "other_browser={}; oxi_session_{}={}",
                session.browser_id, session.account_id, session.token
            ))
            .body(Body::empty())
            .unwrap();

        let (status, _) = send(router, req).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn wrong_account_in_header_returns_account_expired() {
        let store = Arc::new(SessionStore::new(Duration::from_secs(3600)));
        let session1 = create_test_session(&store, "user1@example.com", "pass", "hash1");
        let session2 = create_test_session(&store, "user2@example.com", "pass", "hash2");
        let router = guarded_router(Arc::clone(&store));

        let req = Request::builder()
            .uri("/protected")
            .header(ACTIVE_ACCOUNT_HEADER, &session2.account_id)
            .header("cookie", build_auth_headers(&session1))
            .body(Body::empty())
            .unwrap();

        let (status, json) = send(router, req).await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(json["error"]["code"], "ACCOUNT_EXPIRED");
    }

    #[tokio::test]
    async fn extract_cookie_value_finds_correct_cookie() {
        let req = Request::builder()
            .uri("/protected")
            .header("cookie", "a=1; b=2; c=3")
            .body(Body::empty())
            .unwrap();

        assert_eq!(extract_cookie_value(&req, "a"), Some("1".to_string()));
        assert_eq!(extract_cookie_value(&req, "b"), Some("2".to_string()));
        assert_eq!(extract_cookie_value(&req, "c"), Some("3".to_string()));
        assert_eq!(extract_cookie_value(&req, "d"), None);
    }

    #[tokio::test]
    async fn extract_active_account_header_works() {
        let req = Request::builder()
            .uri("/protected")
            .header(ACTIVE_ACCOUNT_HEADER, "account-123")
            .body(Body::empty())
            .unwrap();

        assert_eq!(extract_active_account_header(&req), Some("account-123".to_string()));
    }

    #[tokio::test]
    async fn extract_active_account_header_missing() {
        let req = Request::builder()
            .uri("/protected")
            .body(Body::empty())
            .unwrap();

        assert_eq!(extract_active_account_header(&req), None);
    }

    #[tokio::test]
    async fn extract_active_account_query_works() {
        let req = Request::builder()
            .uri("/protected?account_id=account-123")
            .body(Body::empty())
            .unwrap();

        assert_eq!(extract_active_account_query(&req), Some("account-123".to_string()));
    }

    #[tokio::test]
    async fn extract_active_account_query_with_encoding() {
        let req = Request::builder()
            .uri("/protected?account_id=abc%40example.com")
            .body(Body::empty())
            .unwrap();

        assert_eq!(extract_active_account_query(&req), Some("abc@example.com".to_string()));
    }

    #[tokio::test]
    async fn extract_active_account_query_missing() {
        let req = Request::builder()
            .uri("/protected?other=value")
            .body(Body::empty())
            .unwrap();

        assert_eq!(extract_active_account_query(&req), None);
    }

    #[tokio::test]
    async fn account_id_from_query_param_works() {
        let store = Arc::new(SessionStore::new(Duration::from_secs(3600)));
        let session = create_test_session(&store, "alice@example.com", "hunter2", "abc123");
        let router = guarded_router(Arc::clone(&store));

        let req = Request::builder()
            .uri(format!("/protected?account_id={}", session.account_id))
            .header("cookie", build_auth_headers(&session))
            .body(Body::empty())
            .unwrap();

        let (status, json) = send(router, req).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["email"], "alice@example.com");
    }

    #[tokio::test]
    async fn header_takes_precedence_over_query_param() {
        let store = Arc::new(SessionStore::new(Duration::from_secs(3600)));
        let session1 = create_test_session(&store, "user1@example.com", "pass", "hash1");
        let session2 = create_test_session(&store, "user2@example.com", "pass", "hash2");
        let router = guarded_router(Arc::clone(&store));

        let req = Request::builder()
            .uri(format!("/protected?account_id={}", session2.account_id))
            .header(ACTIVE_ACCOUNT_HEADER, &session1.account_id)
            .header("cookie", format!(
                "oxi_browser={}; oxi_session_{}={}",
                session1.browser_id, session1.account_id, session1.token
            ))
            .body(Body::empty())
            .unwrap();

        let (status, json) = send(router, req).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["email"], "user1@example.com");
    }
}
