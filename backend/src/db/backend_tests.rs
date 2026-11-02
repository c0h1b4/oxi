//! Backend contract tests. PostgreSQL tests run only with OXI_TEST_DATABASE_URL.
use super::{
    contacts, drafts, folders,
    header_sync::HeaderSync,
    messages,
    pool::{Database, open_test_db},
    tags,
};
use sqlx::Row;

#[tokio::test]
async fn upserts_preserve_cached_body_and_tags_and_unsigned_ranges() {
    let db = open_test_db().await;
    assert!(
        messages::get_messages(&db, "INBOX", u32::MAX, u32::MAX)
            .await
            .is_err()
    );
    folders::upsert_folder(
        &db,
        "INBOX",
        Some("/"),
        None,
        "",
        true,
        1,
        1,
        u32::MAX,
        u64::MAX,
    )
    .await
    .unwrap();
    messages::upsert_message(
        &db,
        "INBOX",
        u32::MAX,
        None,
        None,
        None,
        "Original",
        "a@test",
        "A",
        "[]",
        "[]",
        "2026-01-01",
        "",
        u32::MAX,
        false,
        "",
        None,
    )
    .await
    .unwrap();
    messages::cache_message_body(
        &db,
        "INBOX",
        u32::MAX,
        None,
        Some("cached body"),
        None,
        None,
        None,
    )
    .await
    .unwrap();
    tags::create_tag(&db, "tag", "Important", "#123456")
        .await
        .unwrap();
    tags::add_tag_to_message(&db, "tag", u32::MAX, "INBOX")
        .await
        .unwrap();
    folders::upsert_folder(
        &db,
        "INBOX",
        Some("/"),
        None,
        "",
        true,
        2,
        1,
        u32::MAX,
        u64::MAX,
    )
    .await
    .unwrap();
    messages::upsert_message(
        &db,
        "INBOX",
        u32::MAX,
        None,
        None,
        None,
        "Updated",
        "a@test",
        "A",
        "[]",
        "[]",
        "2026-01-01",
        "",
        u32::MAX,
        false,
        "",
        None,
    )
    .await
    .unwrap();
    assert_eq!(
        folders::get_folder(&db, "INBOX")
            .await
            .unwrap()
            .unwrap()
            .highest_modseq,
        u64::MAX
    );
    assert_eq!(
        messages::get_single_message(&db, "INBOX", u32::MAX)
            .await
            .unwrap()
            .unwrap()
            .size,
        u32::MAX
    );
    assert_eq!(
        messages::get_cached_body(&db, "INBOX", u32::MAX)
            .await
            .unwrap()
            .unwrap()
            .text
            .as_deref(),
        Some("cached body")
    );
    assert_eq!(
        tags::get_message_tags(&db, u32::MAX, "INBOX")
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn postgres_tenants_and_foreign_keys_are_isolated() {
    let Ok(url) = std::env::var("OXI_TEST_DATABASE_URL") else {
        return;
    };
    let directory = tempfile::tempdir().unwrap();
    let database = Database::connect(directory.path().to_str().unwrap(), Some(&url), 4)
        .await
        .unwrap();
    let alice = database
        .user(&format!("alice-{}", uuid::Uuid::new_v4()))
        .await
        .unwrap();
    let bob = database
        .user(&format!("bob-{}", uuid::Uuid::new_v4()))
        .await
        .unwrap();
    for db in [&alice, &bob] {
        folders::insert_folder_if_new(db, "INBOX", None, "")
            .await
            .unwrap();
        messages::upsert_message(
            db,
            "INBOX",
            1,
            None,
            None,
            None,
            db.user_hash(),
            "",
            "",
            "[]",
            "[]",
            "2026-01-01",
            "",
            1,
            false,
            "",
            None,
        )
        .await
        .unwrap();
        drafts::upsert_draft(
            db,
            "same-draft",
            "",
            "",
            "",
            db.user_hash(),
            "",
            None,
            None,
            None,
        )
        .await
        .unwrap();
        tags::create_tag(db, "same-tag", "Important", "#123456")
            .await
            .unwrap();
        tags::add_tag_to_message(db, "same-tag", 1, "INBOX")
            .await
            .unwrap();
        contacts::auto_add_contact(db, "same@test", db.user_hash())
            .await
            .unwrap();
    }
    assert_eq!(
        messages::get_single_message(&alice, "INBOX", 1)
            .await
            .unwrap()
            .unwrap()
            .subject,
        alice.user_hash()
    );
    assert_eq!(
        drafts::get_draft(&bob, "same-draft")
            .await
            .unwrap()
            .unwrap()
            .subject,
        bob.user_hash()
    );
    assert_eq!(
        contacts::get_contact_by_email(&alice, "same@test")
            .await
            .unwrap()
            .unwrap()
            .name,
        alice.user_hash()
    );
    let stale = HeaderSync::capture(&alice).await.unwrap();
    messages::remove_after_mutation(&alice, "INBOX", 1)
        .await
        .unwrap();
    assert!(stale.begin_write(&alice).await.is_err());
    assert!(
        messages::get_single_message(&bob, "INBOX", 1)
            .await
            .unwrap()
            .is_some()
    );
    assert_eq!(
        tags::get_message_tags(&bob, 1, "INBOX")
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(
        tags::get_message_tags(&alice, 1, "INBOX")
            .await
            .unwrap()
            .is_empty()
    );
    let mut connection = alice.acquire().await.unwrap();
    // Referencing a draft that exists only for Bob must fail for Alice.
    sqlx::query("DELETE FROM drafts WHERE user_hash=$1 AND id='same-draft'")
        .bind(alice.user_hash())
        .execute(&mut *connection)
        .await
        .unwrap();
    assert!(sqlx::query("INSERT INTO draft_attachments (user_hash,id,draft_id,filename,content_type,size,file_path) VALUES ($1,'bad','same-draft','x','text/plain',1,'x')")
        .bind(alice.user_hash()).execute(&mut *connection).await.is_err());
    let count: i64 = sqlx::query("SELECT COUNT(*) FROM users WHERE user_hash=$1")
        .bind(alice.user_hash())
        .fetch_one(&mut *connection)
        .await
        .unwrap()
        .get(0);
    assert_eq!(count, 1);
    assert!(
        !directory
            .path()
            .join(alice.user_hash())
            .join("db.sqlite")
            .exists()
    );
    drop(connection);
    database.close().await;
}

#[tokio::test]
async fn postgres_api_round_trip_uses_no_sqlite_and_survives_reconnect() {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use std::{sync::Arc, time::Duration};
    use tower::ServiceExt;
    let Ok(url) = std::env::var("OXI_TEST_DATABASE_URL") else {
        return;
    };
    let directory = tempfile::tempdir().unwrap();
    let mut config: crate::config::AppConfig = serde_json::from_str("{}").unwrap();
    config.data_dir = directory.path().to_str().unwrap().to_string();
    config.database_url = Some(url);
    config.serve_static = false;
    config.imap_host = Some("imap.test".into());
    config.smtp_host = Some("smtp.test".into());
    config.initialize_database().await.unwrap();
    let store = Arc::new(crate::auth::session::SessionStore::new(
        Duration::from_secs(3600),
    ));
    let user_hash = format!("api-{}", uuid::Uuid::new_v4());
    let browser = store.create_browser();
    let (token, account) = store.add_account_to_browser(
        &browser,
        "api@test".into(),
        "password".into(),
        user_hash.clone(),
        "imap.test".into(),
        993,
        true,
        "smtp.test".into(),
        587,
        true,
    );
    crate::auth::user_data::provision_user_data(&config.data_dir, &user_hash)
        .await
        .unwrap();
    let database = config.user_db(&user_hash).await.unwrap();
    folders::upsert_folder(&database, "INBOX", None, None, "", true, 0, 0, 1, 0)
        .await
        .unwrap();
    folders::update_folder_status(&database, "INBOX", 1, 0)
        .await
        .unwrap();
    let config = Arc::new(config);
    let imap = Arc::new(crate::imap::client::mock::MockImapClient::new());
    let smtp = Arc::new(crate::smtp::client::mock::MockSmtpClient::new());
    let app = crate::routes::create_router(
        config.clone(),
        store,
        imap,
        smtp,
        Arc::new(crate::search::engine::SearchEngine::new(
            directory.path().to_path_buf(),
        )),
        Arc::new(crate::realtime::events::EventBus::new()),
        Arc::new(crate::realtime::idle::IdleManager::new()),
    );
    let request = |method: &str, uri: &str, body: &str| {
        Request::builder()
            .method(method)
            .uri(uri)
            .header(
                "cookie",
                format!("oxi_browser={browser}; oxi_session_{account}={token}"),
            )
            .header("x-active-account", &account)
            .header("x-requested-with", "XMLHttpRequest")
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    };
    for (method, uri, body) in [
        ("PUT", "/api/settings/display", r#"{"theme":"dark"}"#),
        (
            "POST",
            "/api/contacts",
            r#"{"email":"person@test","name":"Person"}"#,
        ),
        (
            "POST",
            "/api/drafts",
            r#"{"id":"api-draft","subject":"Saved"}"#,
        ),
        (
            "POST",
            "/api/calendar/events",
            r#"{"title":"Meeting","start_time":"2026-01-01T10:00:00Z","end_time":"2026-01-01T11:00:00Z"}"#,
        ),
        ("GET", "/api/folders/INBOX/messages", ""),
        ("GET", "/api/search?q=Saved", ""),
        ("GET", "/api/ready", ""),
    ] {
        let response = app
            .clone()
            .oneshot(request(method, uri, body))
            .await
            .unwrap();
        assert!(
            response.status().is_success(),
            "{method} {uri}: {} {:?}",
            response.status(),
            response.into_body().collect().await.unwrap().to_bytes()
        );
    }
    messages::upsert_message(
        &database, "INBOX", 77, None, None, None, "Flags", "", "", "[]", "[]", "", "", 1, false,
        "", None,
    )
    .await
    .unwrap();
    let seen = app.clone().oneshot(request(
        "PATCH",
        "/api/messages/INBOX/77/flags",
        r#"{"flags":["\\Seen"],"add":true}"#,
    ));
    let flagged = app.clone().oneshot(request(
        "PATCH",
        "/api/messages/INBOX/77/flags",
        r#"{"flags":["\\Flagged"],"add":true}"#,
    ));
    let (seen, flagged) = tokio::join!(seen, flagged);
    assert_eq!(seen.unwrap().status(), StatusCode::OK);
    assert_eq!(flagged.unwrap().status(), StatusCode::OK);
    let flags = messages::get_single_message(&database, "INBOX", 77)
        .await
        .unwrap()
        .unwrap()
        .flags;
    assert!(flags.contains("\\Seen"));
    assert!(flags.contains("\\Flagged"));
    let response = app
        .clone()
        .oneshot(request("GET", "/api/contacts", ""))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(json["contacts"][0]["email"], "person@test");
    assert!(!directory.path().join(&user_hash).join("db.sqlite").exists());
    let reconnect = config
        .database
        .as_ref()
        .unwrap()
        .user(&user_hash)
        .await
        .unwrap();
    assert_eq!(
        drafts::get_draft(&reconnect, "api-draft")
            .await
            .unwrap()
            .unwrap()
            .subject,
        "Saved"
    );
    assert_eq!(
        super::display_preferences::get_preferences(&reconnect)
            .await
            .unwrap()
            .theme,
        "dark"
    );
    let multipart = "--oxi-boundary\r\nContent-Disposition: form-data; name=\"file\"; filename=\"hello.txt\"\r\nContent-Type: text/plain\r\n\r\nattachment bytes\r\n--oxi-boundary--\r\n";
    let mut upload = request("POST", "/api/drafts/api-draft/attachments", multipart);
    upload.headers_mut().insert(
        "content-type",
        "multipart/form-data; boundary=oxi-boundary"
            .parse()
            .unwrap(),
    );
    let response = app.clone().oneshot(upload).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let uploaded: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let attachment_id = uploaded["attachments"][0]["id"].as_str().unwrap();
    let uri = format!("/api/drafts/api-draft/attachments/{attachment_id}/content");
    let response = app.clone().oneshot(request("GET", &uri, "")).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        &response.into_body().collect().await.unwrap().to_bytes()[..],
        b"attachment bytes"
    );
    let response = app.clone().oneshot(request("POST", "/api/messages/send", r#"{"to":["person@test"],"subject":"Saved","text_body":"sent body","draft_id":"api-draft"}"#)).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        drafts::get_draft(&reconnect, "api-draft")
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        !directory
            .path()
            .join(&user_hash)
            .join("attachments/api-draft")
            .exists()
    );
    assert!(!directory.path().join(&user_hash).join("db.sqlite").exists());
    drop(app);
    config.database.as_ref().unwrap().close().await;
}
