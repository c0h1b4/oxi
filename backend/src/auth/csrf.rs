use axum::extract::Request;
use axum::http::{Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

/// Methods that mutate state and therefore require CSRF protection.
const STATE_CHANGING_METHODS: [Method; 4] = [
    Method::POST,
    Method::PUT,
    Method::DELETE,
    Method::PATCH,
];

/// Axum middleware that enforces CSRF protection on state-changing requests.
///
/// For POST, PUT, DELETE, and PATCH requests the `X-Requested-With` header
/// must be present (any non-empty value is accepted). GET, HEAD, OPTIONS, and
/// all other safe methods pass through without the check.
///
/// This works in concert with `SameSite=Strict` session cookies: the cookie
/// attribute prevents the browser from sending credentials on cross-origin
/// requests, while this header check blocks cross-origin form submissions
/// (browsers do not allow custom headers in simple/form requests).
pub async fn csrf_protection(request: Request, next: Next) -> Response {
    if STATE_CHANGING_METHODS.contains(request.method())
        && !request.headers().contains_key("x-requested-with")
    {
        let body = serde_json::json!({
            "error": {
                "code": "CSRF_REJECTED",
                "message": "Missing X-Requested-With header",
                "status": 403
            }
        });
        return (
            StatusCode::FORBIDDEN,
            [("content-type", "application/json")],
            body.to_string(),
        )
            .into_response();
    }

    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::routing::get;
    use axum::Router;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    /// Build a minimal router with CSRF middleware applied.
    fn app() -> Router {
        Router::new()
            .route("/test", get(|| async { "ok" }).post(|| async { "ok" }))
            .layer(axum::middleware::from_fn(csrf_protection))
    }

    /// Helper: send a request and return the status code and parsed JSON body.
    async fn send(
        method: Method,
        with_header: bool,
    ) -> (StatusCode, serde_json::Value) {
        let mut builder = Request::builder().method(method).uri("/test");

        if with_header {
            builder = builder.header("x-requested-with", "XMLHttpRequest");
        }

        let request = builder.body(Body::empty()).unwrap();
        let response = app().oneshot(request).await.unwrap();
        let status = response.status();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = if body.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_slice(&body).unwrap_or(serde_json::Value::String(
                String::from_utf8_lossy(&body).to_string(),
            ))
        };
        (status, json)
    }

    #[tokio::test]
    async fn get_passes_without_header() {
        let (status, _) = send(Method::GET, false).await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn post_without_header_returns_403() {
        let (status, json) = send(Method::POST, false).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(json["error"]["code"], "CSRF_REJECTED");
        assert_eq!(json["error"]["message"], "Missing X-Requested-With header");
        assert_eq!(json["error"]["status"], 403);
    }

    #[tokio::test]
    async fn post_with_header_passes_through() {
        let (status, _) = send(Method::POST, true).await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn put_without_header_returns_403() {
        let app = Router::new()
            .route("/test", axum::routing::put(|| async { "ok" }))
            .layer(axum::middleware::from_fn(csrf_protection));

        let request = Request::builder()
            .method(Method::PUT)
            .uri("/test")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn delete_without_header_returns_403() {
        let app = Router::new()
            .route("/test", axum::routing::delete(|| async { "ok" }))
            .layer(axum::middleware::from_fn(csrf_protection));

        let request = Request::builder()
            .method(Method::DELETE)
            .uri("/test")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn patch_without_header_returns_403() {
        let app = Router::new()
            .route("/test", axum::routing::patch(|| async { "ok" }))
            .layer(axum::middleware::from_fn(csrf_protection));

        let request = Request::builder()
            .method(Method::PATCH)
            .uri("/test")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn head_passes_without_header() {
        let app = Router::new()
            .route("/test", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn(csrf_protection));

        let request = Request::builder()
            .method(Method::HEAD)
            .uri("/test")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn options_passes_without_header() {
        let app = Router::new()
            .route("/test", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn(csrf_protection));

        let request = Request::builder()
            .method(Method::OPTIONS)
            .uri("/test")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        // OPTIONS may return 200 or other codes depending on router config,
        // but it must NOT return 403 from CSRF middleware.
        assert_ne!(response.status(), StatusCode::FORBIDDEN);
    }
}
