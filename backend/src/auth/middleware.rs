use std::sync::Arc;

use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use super::session::SessionStore;

/// Name of the cookie that carries the session token.
pub const SESSION_COOKIE: &str = "oxi_session";

/// JSON body returned on authentication failure.
const UNAUTHORIZED_BODY: &str = r#"{"error":{"code":"UNAUTHORIZED","message":"Invalid or expired session","status":401}}"#;

/// Build the 401 rejection response.
fn unauthorized() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        [("content-type", "application/json")],
        UNAUTHORIZED_BODY,
    )
        .into_response()
}

/// Extract the value of `oxi_session` from the `Cookie` header.
///
/// Iterates over all `Cookie` headers, splits each by `;`, trims
/// whitespace, and looks for a segment starting with `oxi_session=`.
fn extract_session_cookie(req: &Request) -> Option<String> {
    for value in req.headers().get_all("cookie") {
        let Ok(header_str) = value.to_str() else {
            continue;
        };
        for segment in header_str.split(';') {
            let trimmed = segment.trim();
            if let Some(token) = trimmed.strip_prefix("oxi_session=") {
                let token = token.trim();
                if !token.is_empty() {
                    return Some(token.to_string());
                }
            }
        }
    }
    None
}

/// Auth-guard middleware compatible with [`axum::middleware::from_fn`].
///
/// 1. Extracts `Arc<SessionStore>` from request extensions.
/// 2. Parses `oxi_session=<token>` from the `Cookie` header.
/// 3. Validates the token against the store (which also refreshes the
///    sliding-window expiry).
/// 4. On success: inserts the [`SessionState`] into request extensions
///    so downstream handlers can access it via `Extension<SessionState>`.
/// 5. On failure: returns a `401 Unauthorized` JSON response.
pub async fn auth_guard(mut req: Request, next: Next) -> Response {
    // Retrieve the session store from request extensions.
    let store = match req.extensions().get::<Arc<SessionStore>>() {
        Some(s) => Arc::clone(s),
        None => return unauthorized(),
    };

    // Extract the session cookie.
    let token = match extract_session_cookie(&req) {
        Some(t) => t,
        None => return unauthorized(),
    };

    // Validate the token (also refreshes sliding window).
    let session = match store.get(&token) {
        Some(s) => s,
        None => return unauthorized(),
    };

    // Inject the session state for downstream handlers.
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
    use crate::auth::session::{SessionState, SessionStore};

    /// Helper: build a router with auth_guard applied, backed by the given store.
    fn guarded_router(store: Arc<SessionStore>) -> Router {
        let handler = |Extension(session): Extension<SessionState>| async move {
            serde_json::json!({ "email": session.email }).to_string().into_response()
        };

        Router::new()
            .route("/protected", get(handler))
            .layer(middleware::from_fn(auth_guard))
            .layer(Extension(store))
    }

    /// Helper: send a request and return status + body JSON.
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

    #[tokio::test]
    async fn no_cookie_returns_401() {
        let store = Arc::new(SessionStore::new(Duration::from_secs(3600)));
        let router = guarded_router(store);

        let req = Request::builder()
            .uri("/protected")
            .body(Body::empty())
            .unwrap();

        let (status, json) = send(router, req).await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(json["error"]["code"], "UNAUTHORIZED");
        assert_eq!(json["error"]["message"], "Invalid or expired session");
        assert_eq!(json["error"]["status"], 401);
    }

    #[tokio::test]
    async fn invalid_token_returns_401() {
        let store = Arc::new(SessionStore::new(Duration::from_secs(3600)));
        let router = guarded_router(store);

        let req = Request::builder()
            .uri("/protected")
            .header("cookie", "oxi_session=bogus-token-value")
            .body(Body::empty())
            .unwrap();

        let (status, json) = send(router, req).await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(json["error"]["code"], "UNAUTHORIZED");
        assert_eq!(json["error"]["message"], "Invalid or expired session");
        assert_eq!(json["error"]["status"], 401);
    }

    #[tokio::test]
    async fn valid_session_returns_200_with_email() {
        let store = Arc::new(SessionStore::new(Duration::from_secs(3600)));
        let token = store.insert(
            "alice@example.com".into(),
            "hunter2".into(),
            "abc123".into(),
        None,
        );
        let router = guarded_router(store);

        let req = Request::builder()
            .uri("/protected")
            .header("cookie", format!("oxi_session={token}"))
            .body(Body::empty())
            .unwrap();

        let (status, json) = send(router, req).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["email"], "alice@example.com");
    }

    #[tokio::test]
    async fn expired_session_returns_401() {
        let store = Arc::new(SessionStore::new(Duration::from_millis(50)));
        let token = store.insert(
            "bob@example.com".into(),
            "pass".into(),
            "hash".into(),
        None,
        );

        // Wait for the session to expire.
        thread::sleep(Duration::from_millis(100));

        let router = guarded_router(store);

        let req = Request::builder()
            .uri("/protected")
            .header("cookie", format!("oxi_session={token}"))
            .body(Body::empty())
            .unwrap();

        let (status, json) = send(router, req).await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(json["error"]["code"], "UNAUTHORIZED");
        assert_eq!(json["error"]["message"], "Invalid or expired session");
        assert_eq!(json["error"]["status"], 401);
    }

    #[tokio::test]
    async fn cookie_among_multiple_cookies() {
        let store = Arc::new(SessionStore::new(Duration::from_secs(3600)));
        let token = store.insert(
            "multi@example.com".into(),
            "pass".into(),
            "hash".into(),
        None,
        );
        let router = guarded_router(store);

        let req = Request::builder()
            .uri("/protected")
            .header(
                "cookie",
                format!("theme=dark; oxi_session={token}; lang=en"),
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
        let token = store.insert(
            "wrong@example.com".into(),
            "pass".into(),
            "hash".into(),
        None,
        );
        let router = guarded_router(store);

        let req = Request::builder()
            .uri("/protected")
            .header("cookie", format!("other_cookie={token}"))
            .body(Body::empty())
            .unwrap();

        let (status, _) = send(router, req).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
}
