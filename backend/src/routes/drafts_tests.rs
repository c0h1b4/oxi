use super::*;
use crate::imap::client::ImapMessageHeader;
use crate::imap::client::mock::MockImapClient;
use crate::smtp::client::SmtpClient;
use crate::smtp::client::mock::MockSmtpClient;

fn setup(dir: &tempfile::TempDir) -> (Arc<AppConfig>, SessionState) {
    let mut config: AppConfig = serde_json::from_value(serde_json::json!({})).unwrap();
    config.data_dir = dir.path().to_str().unwrap().to_string();
    config.imap_host = Some("imap.example.com".into());
    config.smtp_host = Some("smtp.example.com".into());
    let session = SessionState {
        account_id: "account".into(),
        email: "alice@example.com".into(),
        password: "password".into(),
        user_hash: "user".into(),
        imap_host: "imap.example.com".into(),
        imap_port: 993,
        imap_tls: true,
        smtp_host: "smtp.example.com".into(),
        smtp_port: 587,
        smtp_tls: true,
        last_accessed: std::time::Instant::now(),
        timeout_override: None,
    };
    (Arc::new(config), session)
}

fn request(subject: &str) -> UpsertDraftRequest {
    UpsertDraftRequest {
        id: "draft-1".into(),
        to: "bob@example.com".into(),
        cc: String::new(),
        bcc: String::new(),
        subject: subject.into(),
        text_body: "body".into(),
        html_body: None,
        in_reply_to: None,
        references: None,
    }
}

fn header(uid: u32, id: &str) -> ImapMessageHeader {
    ImapMessageHeader {
        uid,
        subject: None,
        from: vec![],
        to: vec![],
        date: None,
        flags: vec![],
        has_attachments: false,
        size: 0,
        message_id: Some(id.into()),
        in_reply_to: None,
        references: None,
        cc: vec![],
        reaction: None,
    }
}

#[tokio::test]
async fn edited_draft_replaces_only_its_previous_imap_copies() {
    let dir = tempfile::tempdir().unwrap();
    let (config, session) = setup(&dir);
    let client = Arc::new(MockImapClient::new().with_headers(vec![
        header(1, &draft_message_id("draft-1")),
        header(2, &draft_message_id("draft-1")),
        header(3, "<unrelated@example.com>"),
    ]));
    upsert_draft_handler(
        Extension(session.clone()),
        Extension(config.clone()),
        Extension(client.clone() as Arc<dyn ImapClient>),
        Json(request("edited")),
    )
    .await
    .unwrap();
    assert_eq!(
        *client.expunged.lock().unwrap(),
        vec![("Drafts".into(), 1), ("Drafts".into(), 2)]
    );
    {
        let appended = client.appended.lock().unwrap();
        assert_eq!(appended.len(), 1);
        let parsed = mail_parser::MessageParser::default()
            .parse(&appended[0].1)
            .unwrap();
        assert_eq!(
            format!("<{}>", parsed.message_id().unwrap()),
            draft_message_id("draft-1")
        );
    }
    let conn = config.user_db(&session.user_hash).await.unwrap();
    assert_eq!(db::drafts::list_drafts(&conn).await.unwrap().len(), 1);
}

#[tokio::test]
async fn sending_reopened_draft_removes_local_and_imap_copies() {
    let dir = tempfile::tempdir().unwrap();
    let (config, session) = setup(&dir);
    let client =
        Arc::new(MockImapClient::new().with_headers(vec![header(8, &draft_message_id("draft-1"))]));
    let conn = config.user_db(&session.user_hash).await.unwrap();
    db::drafts::upsert_draft(
        &conn,
        "draft-1",
        "bob@example.com",
        "",
        "",
        "saved",
        "body",
        None,
        None,
        None,
    )
    .await
    .unwrap();
    let req = super::super::send::SendRequest {
        to: vec!["bob@example.com".into()],
        cc: vec![],
        bcc: vec![],
        subject: "edited".into(),
        text_body: Some("edited body".into()),
        html_body: None,
        in_reply_to: None,
        references: None,
        draft_id: Some("draft-1".into()),
        from_identity_id: None,
    };
    super::super::send::send_message_handler(
        Extension(session.clone()),
        Extension(config),
        Extension(Arc::new(MockSmtpClient::new()) as Arc<dyn SmtpClient>),
        Extension(client.clone() as Arc<dyn ImapClient>),
        Json(req),
    )
    .await
    .unwrap();
    assert!(db::drafts::list_drafts(&conn).await.unwrap().is_empty());
    assert_eq!(*client.expunged.lock().unwrap(), vec![("Drafts".into(), 8)]);
}

#[tokio::test]
async fn failed_imap_sync_keeps_the_local_draft_and_does_not_delete_server_copies() {
    let dir = tempfile::tempdir().unwrap();
    let (config, session) = setup(&dir);
    let client = Arc::new(MockImapClient::new().with_error(
        crate::imap::client::ImapError::ConnectionFailed("offline".into()),
    ));
    upsert_draft_handler(
        Extension(session.clone()),
        Extension(config.clone()),
        Extension(client.clone() as Arc<dyn ImapClient>),
        Json(request("edited offline")),
    )
    .await
    .unwrap();
    let conn = config.user_db(&session.user_hash).await.unwrap();
    assert_eq!(
        db::drafts::get_draft(&conn, "draft-1")
            .await
            .unwrap()
            .unwrap()
            .subject,
        "edited offline"
    );
    assert!(client.expunged.lock().unwrap().is_empty());
}

#[tokio::test]
async fn discarding_a_draft_removes_its_server_copy() {
    let dir = tempfile::tempdir().unwrap();
    let (config, session) = setup(&dir);
    let client =
        Arc::new(MockImapClient::new().with_headers(vec![header(8, &draft_message_id("draft-1"))]));
    let conn = config.user_db(&session.user_hash).await.unwrap();
    db::drafts::upsert_draft(
        &conn,
        "draft-1",
        "bob@example.com",
        "",
        "",
        "saved",
        "body",
        None,
        None,
        None,
    )
    .await
    .unwrap();
    delete_draft_handler(
        Extension(session),
        Extension(config),
        Extension(client.clone() as Arc<dyn ImapClient>),
        AxumPath("draft-1".into()),
    )
    .await
    .unwrap();
    assert!(db::drafts::list_drafts(&conn).await.unwrap().is_empty());
    assert_eq!(*client.expunged.lock().unwrap(), vec![("Drafts".into(), 8)]);
}
