use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

/// Structured JSON error envelope returned to clients.
#[derive(Debug, Serialize)]
struct ErrorEnvelope {
    error: ErrorBody,
}

/// Body of the error envelope.
#[derive(Debug, Serialize)]
struct ErrorBody {
    code: String,
    message: String,
    status: u16,
}

/// Application-level error type that converts into an Axum response
/// with a structured JSON error envelope.
#[derive(Debug)]
pub enum AppError {
    /// Internal server error (500).
    InternalError(String),
    /// Resource not found (404).
    NotFound(String),
    /// Unauthorized access (401).
    Unauthorized(String),
    /// Bad request (400).
    BadRequest(String),
    /// Service unavailable (503).
    ServiceUnavailable(String),
}

impl AppError {
    /// Returns the HTTP status code for this error variant.
    fn status_code(&self) -> StatusCode {
        match self {
            AppError::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::ServiceUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
        }
    }

    /// Returns the error code string for this error variant.
    fn error_code(&self) -> &'static str {
        match self {
            AppError::InternalError(_) => "INTERNAL_ERROR",
            AppError::NotFound(_) => "NOT_FOUND",
            AppError::Unauthorized(_) => "UNAUTHORIZED",
            AppError::BadRequest(_) => "BAD_REQUEST",
            AppError::ServiceUnavailable(_) => "SERVICE_UNAVAILABLE",
        }
    }

    /// Returns the human-readable message for this error variant.
    fn message(&self) -> &str {
        match self {
            AppError::InternalError(msg)
            | AppError::NotFound(msg)
            | AppError::Unauthorized(msg)
            | AppError::BadRequest(msg)
            | AppError::ServiceUnavailable(msg) => msg,
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let envelope = ErrorEnvelope {
            error: ErrorBody {
                code: self.error_code().to_string(),
                message: self.message().to_string(),
                status: status.as_u16(),
            },
        };

        let body = serde_json::to_string(&envelope).unwrap_or_else(|_| {
            r#"{"error":{"code":"INTERNAL_ERROR","message":"Failed to serialize error","status":500}}"#
                .to_string()
        });

        (status, [("content-type", "application/json")], body).into_response()
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.error_code(), self.message())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use http_body_util::BodyExt;

    async fn error_to_json(error: AppError) -> (StatusCode, serde_json::Value) {
        let response = error.into_response();
        let status = response.status();
        let body = Body::from(response.into_body())
            .collect()
            .await
            .expect("body should collect")
            .to_bytes();
        let json: serde_json::Value =
            serde_json::from_slice(&body).expect("body should be valid JSON");
        (status, json)
    }

    #[tokio::test]
    async fn internal_error_returns_500() {
        let (status, json) = error_to_json(AppError::InternalError("something broke".into())).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(json["error"]["code"], "INTERNAL_ERROR");
        assert_eq!(json["error"]["message"], "something broke");
        assert_eq!(json["error"]["status"], 500);
    }

    #[tokio::test]
    async fn not_found_returns_404() {
        let (status, json) = error_to_json(AppError::NotFound("mailbox missing".into())).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(json["error"]["code"], "NOT_FOUND");
        assert_eq!(json["error"]["message"], "mailbox missing");
        assert_eq!(json["error"]["status"], 404);
    }

    #[tokio::test]
    async fn unauthorized_returns_401() {
        let (status, json) = error_to_json(AppError::Unauthorized("bad token".into())).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(json["error"]["code"], "UNAUTHORIZED");
        assert_eq!(json["error"]["message"], "bad token");
        assert_eq!(json["error"]["status"], 401);
    }

    #[tokio::test]
    async fn bad_request_returns_400() {
        let (status, json) = error_to_json(AppError::BadRequest("invalid input".into())).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json["error"]["code"], "BAD_REQUEST");
        assert_eq!(json["error"]["message"], "invalid input");
        assert_eq!(json["error"]["status"], 400);
    }

    #[tokio::test]
    async fn service_unavailable_returns_503() {
        let (status, json) =
            error_to_json(AppError::ServiceUnavailable("IMAP down".into())).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(json["error"]["code"], "SERVICE_UNAVAILABLE");
        assert_eq!(json["error"]["message"], "IMAP down");
        assert_eq!(json["error"]["status"], 503);
    }
}
