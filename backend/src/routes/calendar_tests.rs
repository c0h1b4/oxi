use super::*;
struct AccountSmtp;

#[async_trait::async_trait]
impl SmtpClient for AccountSmtp {
    async fn send_message(
        &self,
        credentials: &SmtpCredentials,
        _message: &crate::smtp::client::SendableMessage,
    ) -> Result<String, SmtpError> {
        assert_eq!(credentials.host, "smtp.account.example.com");
        assert_eq!(credentials.port, 465);
        assert!(credentials.tls);
        assert_eq!(credentials.email, "owner@example.com");
        assert_eq!(credentials.password, "secret");
        Ok("<test@example.com>".into())
    }
}

#[tokio::test]
async fn calendar_invitations_use_active_account_smtp() {
    let (router, _directory, _config) = setup(Arc::new(AccountSmtp));
    let (status, event) = request(&router, "POST", "/events", event_body()).await;
    assert_eq!(status, StatusCode::CREATED);
    assert!(event.get("invitation_error").is_none());
}

#[tokio::test]
async fn calendar_invitations_no_attendees_does_not_send() {
    let smtp = Arc::new(MockSmtpClient::new());
    let (router, _directory, _) = setup(smtp.clone());
    let mut body = event_body();
    body["attendees"] = "[]".into();
    let (status, event) = request(&router, "POST", "/events", body).await;
    assert_eq!(status, StatusCode::CREATED);
    assert!(event.get("invitation_error").is_none());
    assert_eq!(smtp.sent_count(), 0);
    let id = event["id"].as_str().unwrap();
    let (status, _) = request(
        &router,
        "POST",
        &format!("/events/{id}/invitations"),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = request(
        &router,
        "POST",
        "/events/missing/invitations",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

use crate::smtp::client::SmtpError;
use crate::smtp::client::mock::MockSmtpClient;
use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::{post, put};
use http_body_util::BodyExt;
use tower::ServiceExt;

fn setup(smtp: Arc<dyn SmtpClient>) -> (Router, tempfile::TempDir, Arc<AppConfig>) {
    let directory = tempfile::tempdir().unwrap();
    let config: AppConfig = serde_json::from_value(serde_json::json!({
        "data_dir": directory.path().to_str().unwrap(),
    }))
    .unwrap();
    let config = Arc::new(config);
    let session = SessionState {
        account_id: "account".into(),
        email: "owner@example.com".into(),
        password: "secret".into(),
        user_hash: "calendar-test".into(),
        imap_host: "imap.example.com".into(),
        imap_port: 993,
        imap_tls: true,
        smtp_host: "smtp.account.example.com".into(),
        smtp_port: 465,
        smtp_tls: true,
        last_accessed: std::time::Instant::now(),
        timeout_override: None,
    };
    let router = Router::new()
        .route("/events", post(create_event))
        .route("/events/import-ics", post(import_ics))
        .route("/events/{id}", put(update_event))
        .route("/events/{id}/invitations", post(send_invitations))
        .layer(Extension(session))
        .layer(Extension(config.clone()))
        .layer(Extension(smtp as Arc<dyn SmtpClient>));
    (router, directory, config)
}

fn event_body() -> serde_json::Value {
    serde_json::json!({
        "title":"Team, meeting", "description":"Agenda\nNew line; notes", "location":"Room A",
        "start_time":"2026-11-15T09:00:00", "end_time":"2026-11-15T10:00:00",
        "attendees":"[\"alice@example.com\",\"ALICE@example.com\",\"owner@example.com\"]",
        "meeting_url":"https://example.com/meeting"
    })
}

async fn request(
    router: &Router,
    method: &str,
    url: &str,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(url)
                .header("Content-Type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap())
}

fn calendar(smtp: &MockSmtpClient) -> String {
    String::from_utf8(smtp.last_sent().unwrap().attachments[0].data.clone()).unwrap()
}

#[tokio::test]
async fn calendar_invitations_create_update_and_resend() {
    let smtp = Arc::new(MockSmtpClient::new());
    let (router, _directory, _config) = setup(smtp.clone());
    let (status, event) = request(&router, "POST", "/events", event_body()).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(event["organizer"], "owner@example.com");
    assert!(event.get("invitation_error").is_none());
    assert_eq!(smtp.sent_count(), 1);
    assert_eq!(smtp.last_sent().unwrap().to, ["alice@example.com"]);
    let initial = calendar(&smtp);
    let id = event["id"].as_str().unwrap();
    let uid = format!("UID:{id}@oxi.email");
    assert!(initial.contains(&uid));
    assert!(initial.contains("METHOD:REQUEST\r\n"));
    assert!(initial.contains("DTSTART:20261115T090000\r\n"));
    assert!(initial.contains("SEQUENCE:0\r\n"));
    assert!(initial.contains("SUMMARY:Team\\, meeting\r\n"));
    assert!(initial.contains("DESCRIPTION:Agenda\\nNew line\\; notes\r\n"));
    assert!(initial.contains("URL:https://example.com/meeting\r\n"));

    let url = format!("/events/{id}");
    let (status, _) = request(
        &router,
        "PUT",
        &url,
        serde_json::json!({"title":"Save only", "send_invitations":false}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(smtp.sent_count(), 1);
    let (status, _) = request(
        &router,
        "PUT",
        &url,
        serde_json::json!({"title":"Send update", "send_invitations":true}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(smtp.sent_count(), 2);
    assert!(calendar(&smtp).contains(&uid));
    assert!(calendar(&smtp).contains("SEQUENCE:2\r\n"));
    let (status, response) = request(
        &router,
        "POST",
        &format!("{url}/invitations"),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(response["sent"], 1);
    assert_eq!(smtp.sent_count(), 3);
    assert!(calendar(&smtp).contains(&uid));
    assert!(calendar(&smtp).contains("SEQUENCE:2\r\n"));
}

#[tokio::test]
async fn calendar_invitations_validate_before_persisting() {
    let smtp = Arc::new(MockSmtpClient::new());
    let (router, _directory, config) = setup(smtp.clone());
    for attendees in [
        "not json",
        "[\"not-an-email\"]",
        "[\"alice@example.com\\r\\nATTENDEE:mailto:bob@example.com\"]",
    ] {
        let mut body = event_body();
        body["attendees"] = attendees.into();
        let (status, _) = request(&router, "POST", "/events", body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
    let conn = db::pool::open_user_db(&config.data_dir, "calendar-test").unwrap();
    assert!(
        db::calendar::list_events(&conn, "2020", "2030")
            .unwrap()
            .is_empty()
    );
    let (_, event) = request(&router, "POST", "/events", event_body()).await;
    let url = format!("/events/{}", event["id"].as_str().unwrap());
    let (status, _) = request(
        &router,
        "PUT",
        &url,
        serde_json::json!({"title":"Must not save", "attendees":"bad", "send_invitations":true}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let stored = db::calendar::get_event(&conn, event["id"].as_str().unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(stored.title, "Team, meeting");
    assert_eq!(smtp.sent_count(), 1);
    conn.execute(
        "UPDATE calendar_events SET attendees = 'bad' WHERE id = ?1",
        [event["id"].as_str().unwrap()],
    )
    .unwrap();
    let (status, repaired) = request(
        &router,
        "PUT",
        &url,
        serde_json::json!({"attendees":"[\"bob@example.com\"]", "send_invitations":true}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(repaired.get("invitation_error").is_none());
    assert_eq!(smtp.last_sent().unwrap().to, ["bob@example.com"]);
}

#[tokio::test]
async fn calendar_invitations_failure_keeps_event_and_allows_retry() {
    let smtp = Arc::new(MockSmtpClient::new().with_error(SmtpError::AuthenticationFailed));
    let (router, _directory, config) = setup(smtp);
    let (status, event) = request(&router, "POST", "/events", event_body()).await;
    assert_eq!(status, StatusCode::CREATED);
    assert!(
        event["invitation_error"]
            .as_str()
            .unwrap()
            .contains("Authentication failed")
    );
    let id = event["id"].as_str().unwrap();
    let conn = db::pool::open_user_db(&config.data_dir, "calendar-test").unwrap();
    assert!(db::calendar::get_event(&conn, id).unwrap().is_some());
    let (status, _) = request(
        &router,
        "POST",
        &format!("/events/{id}/invitations"),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    let (status, updated) = request(
        &router,
        "PUT",
        &format!("/events/{id}"),
        serde_json::json!({"title":"Saved despite SMTP", "send_invitations":true}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(updated.get("invitation_error").is_some());
    assert_eq!(
        db::calendar::get_event(&conn, id).unwrap().unwrap().title,
        "Saved despite SMTP"
    );
}

#[tokio::test]
async fn calendar_invitations_import_is_silent_and_not_sendable() {
    let smtp = Arc::new(MockSmtpClient::new());
    let (router, _directory, _config) = setup(smtp.clone());
    let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:external@example.com\r\nSUMMARY:Imported\r\nDTSTART:20261115T090000Z\r\nDTEND:20261115T100000Z\r\nORGANIZER:mailto:owner@example.com\r\nATTENDEE:mailto:alice@example.com\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/events/import-ics")
                .body(Body::from(ics))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let imported: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(smtp.sent_count(), 0);
    let id = imported["events"][0]["id"].as_str().unwrap();
    let (status, _) = request(
        &router,
        "POST",
        &format!("/events/{id}/invitations"),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = request(
        &router,
        "PUT",
        &format!("/events/{id}"),
        serde_json::json!({"title":"Must not save", "send_invitations":true}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(smtp.sent_count(), 0);
}

#[tokio::test]
async fn calendar_invitations_legacy_organizer_all_day_and_offsets() {
    let smtp = Arc::new(MockSmtpClient::new());
    let (router, _directory, config) = setup(smtp.clone());
    let mut body = event_body();
    body["all_day"] = true.into();
    body["start_time"] = "2026-11-15T00:00:00".into();
    body["end_time"] = "2026-11-15T23:59:59".into();
    let (_, event) = request(&router, "POST", "/events", body).await;
    assert!(calendar(&smtp).contains("DTSTART;VALUE=DATE:20261115\r\n"));
    assert!(calendar(&smtp).contains("DTEND;VALUE=DATE:20261116\r\n"));
    let id = event["id"].as_str().unwrap();
    let conn = db::pool::open_user_db(&config.data_dir, "calendar-test").unwrap();
    conn.execute(
        "UPDATE calendar_events SET organizer = '' WHERE id = ?1",
        [id],
    )
    .unwrap();
    let (status, _) = request(&router, "PUT", &format!("/events/{id}"), serde_json::json!({"all_day":false, "start_time":"2026-11-15T09:00:00+02:00", "end_time":"2026-11-15T10:00:00+02:00", "send_invitations":true})).await;
    assert_eq!(status, StatusCode::OK);
    assert!(calendar(&smtp).contains("DTSTART:20261115T070000Z\r\n"));
    assert!(calendar(&smtp).contains("ORGANIZER:mailto:owner@example.com\r\n"));
    conn.execute(
        "UPDATE calendar_events SET organizer = 'someone@example.com' WHERE id = ?1",
        [id],
    )
    .unwrap();
    let (status, _) = request(
        &router,
        "POST",
        &format!("/events/{id}/invitations"),
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
