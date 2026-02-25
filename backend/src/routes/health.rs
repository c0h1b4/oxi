use axum::Json;
use serde::Serialize;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
}

/// Handler for `GET /api/health`.
///
/// Returns `200 OK` with `{ "status": "ok" }`.
pub async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use crate::auth::session::SessionStore;
    use crate::config::AppConfig;
    use crate::imap::client::ImapClient;
    use crate::imap::client::mock::MockImapClient;
    use crate::smtp::client::SmtpClient;
    use crate::smtp::client::mock::MockSmtpClient;
    use crate::routes::create_router;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    #[tokio::test]
    async fn health_check_returns_ok() {
        let config = Arc::new(AppConfig {
            host: "127.0.0.1".to_string(),
            port: 3001,
            imap_host: None,
            imap_port: 993,
            smtp_host: None,
            smtp_port: 587,
            tls_enabled: true,
            data_dir: "/tmp/oxi-test".to_string(),
            session_timeout_hours: 24,
            static_dir: "nonexistent_static_dir".to_string(),
            environment: "development".to_string(),
        });
        let store = Arc::new(SessionStore::new(Duration::from_secs(3600)));
        let imap_client: Arc<dyn ImapClient> = Arc::new(MockImapClient::new());
        let smtp_client: Arc<dyn SmtpClient> = Arc::new(MockSmtpClient::new());
        let app = create_router(config, store, imap_client, smtp_client);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/health")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("response should succeed");

        assert_eq!(response.status(), StatusCode::OK);

        let body = response
            .into_body()
            .collect()
            .await
            .expect("body should collect")
            .to_bytes();

        let json: serde_json::Value =
            serde_json::from_slice(&body).expect("body should be valid JSON");
        assert_eq!(json["status"], "ok");
    }
}
