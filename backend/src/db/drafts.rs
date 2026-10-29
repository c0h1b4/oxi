use serde::Serialize;
use sqlx::{Row, any::AnyRow};

use super::pool::{UserDb, now};

/// A locally-saved draft message.
#[derive(Debug, Clone, Serialize)]
pub struct Draft {
    pub id: String,
    pub to_addresses: String,
    pub cc_addresses: String,
    pub bcc_addresses: String,
    pub subject: String,
    pub text_body: String,
    pub html_body: Option<String>,
    pub in_reply_to: Option<String>,
    pub references_header: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// An attachment associated with a draft.
#[derive(Debug, Clone, Serialize)]
pub struct DraftAttachment {
    pub id: String,
    pub draft_id: String,
    pub filename: String,
    pub content_type: String,
    pub size: i64,
    pub file_path: String,
    pub created_at: String,
}

fn row_to_draft(row: &AnyRow) -> Result<Draft, sqlx::Error> {
    Ok(Draft {
        id: row.try_get(0)?,
        to_addresses: row.try_get(1)?,
        cc_addresses: row.try_get(2)?,
        bcc_addresses: row.try_get(3)?,
        subject: row.try_get(4)?,
        text_body: row.try_get(5)?,
        html_body: row.try_get(6)?,
        in_reply_to: row.try_get(7)?,
        references_header: row.try_get(8)?,
        created_at: row.try_get(9)?,
        updated_at: row.try_get(10)?,
    })
}

const DRAFT_COLUMNS: &str = "id, to_addresses, cc_addresses, bcc_addresses, subject, text_body, html_body, in_reply_to, references_header, created_at, updated_at";

/// Insert or update a draft without replacing its attachment records.
#[allow(clippy::too_many_arguments)]
pub async fn upsert_draft(
    db: &UserDb,
    id: &str,
    to_addresses: &str,
    cc_addresses: &str,
    bcc_addresses: &str,
    subject: &str,
    text_body: &str,
    html_body: Option<&str>,
    in_reply_to: Option<&str>,
    references_header: Option<&str>,
) -> Result<(), String> {
    let mut connection = db.acquire().await?;
    let timestamp = now();
    sqlx::query(
        "INSERT INTO drafts
        (user_hash, id, to_addresses, cc_addresses, bcc_addresses, subject, text_body, html_body,
         in_reply_to, references_header, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $11)
        ON CONFLICT(user_hash, id) DO UPDATE SET
        to_addresses = excluded.to_addresses, cc_addresses = excluded.cc_addresses,
        bcc_addresses = excluded.bcc_addresses, subject = excluded.subject,
        text_body = excluded.text_body, html_body = excluded.html_body,
        in_reply_to = excluded.in_reply_to, references_header = excluded.references_header,
        updated_at = excluded.updated_at",
    )
    .bind(db.user_hash())
    .bind(id)
    .bind(to_addresses)
    .bind(cc_addresses)
    .bind(bcc_addresses)
    .bind(subject)
    .bind(text_body)
    .bind(html_body)
    .bind(in_reply_to)
    .bind(references_header)
    .bind(timestamp)
    .execute(&mut *connection)
    .await
    .map_err(|e| format!("Failed to upsert draft: {e}"))?;
    Ok(())
}

pub async fn get_draft(db: &UserDb, id: &str) -> Result<Option<Draft>, String> {
    let mut connection = db.acquire().await?;
    let sql = format!("SELECT {DRAFT_COLUMNS} FROM drafts WHERE user_hash = $1 AND id = $2");
    sqlx::query(&sql)
        .bind(db.user_hash())
        .bind(id)
        .fetch_optional(&mut *connection)
        .await
        .and_then(|row| row.as_ref().map(row_to_draft).transpose())
        .map_err(|e| format!("Failed to get draft: {e}"))
}

pub async fn list_drafts(db: &UserDb) -> Result<Vec<Draft>, String> {
    let mut connection = db.acquire().await?;
    let sql =
        format!("SELECT {DRAFT_COLUMNS} FROM drafts WHERE user_hash = $1 ORDER BY updated_at DESC");
    let rows = sqlx::query(&sql)
        .bind(db.user_hash())
        .fetch_all(&mut *connection)
        .await
        .map_err(|e| format!("Failed to query drafts: {e}"))?;
    rows.iter()
        .map(row_to_draft)
        .collect::<Result<_, _>>()
        .map_err(|e| format!("Failed to read draft row: {e}"))
}

/// Delete only this user's draft. Foreign keys cascade to its attachments.
pub async fn delete_draft(db: &UserDb, id: &str) -> Result<bool, String> {
    let mut connection = db.acquire().await?;
    sqlx::query("DELETE FROM drafts WHERE user_hash = $1 AND id = $2")
        .bind(db.user_hash())
        .bind(id)
        .execute(&mut *connection)
        .await
        .map(|result| result.rows_affected() > 0)
        .map_err(|e| format!("Failed to delete draft: {e}"))
}

#[allow(clippy::too_many_arguments)]
pub async fn add_draft_attachment(
    db: &UserDb,
    id: &str,
    draft_id: &str,
    filename: &str,
    content_type: &str,
    size: i64,
    file_path: &str,
) -> Result<(), String> {
    let mut connection = db.acquire().await?;
    // The tenant-scoped parent check also protects SQLite's legacy ID-only foreign key.
    let result = sqlx::query(
        "INSERT INTO draft_attachments
        (user_hash, id, draft_id, filename, content_type, size, file_path, created_at)
        SELECT $1, $2, $3, $4, $5, $6, $7, $8
        WHERE EXISTS (SELECT 1 FROM drafts WHERE user_hash = $1 AND id = $3)",
    )
    .bind(db.user_hash())
    .bind(id)
    .bind(draft_id)
    .bind(filename)
    .bind(content_type)
    .bind(size)
    .bind(file_path)
    .bind(now())
    .execute(&mut *connection)
    .await
    .map_err(|e| format!("Failed to add draft attachment: {e}"))?;
    if result.rows_affected() == 0 {
        return Err("Failed to add draft attachment: draft not found".to_string());
    }
    Ok(())
}

pub async fn get_draft_attachments(
    db: &UserDb,
    draft_id: &str,
) -> Result<Vec<DraftAttachment>, String> {
    let mut connection = db.acquire().await?;
    let rows = sqlx::query(
        "SELECT id, draft_id, filename, content_type, size, file_path, created_at
        FROM draft_attachments WHERE user_hash = $1 AND draft_id = $2 ORDER BY created_at ASC",
    )
    .bind(db.user_hash())
    .bind(draft_id)
    .fetch_all(&mut *connection)
    .await
    .map_err(|e| format!("Failed to query draft attachments: {e}"))?;
    rows.iter()
        .map(|row| {
            Ok(DraftAttachment {
                id: row.try_get(0)?,
                draft_id: row.try_get(1)?,
                filename: row.try_get(2)?,
                content_type: row.try_get(3)?,
                size: row.try_get(4)?,
                file_path: row.try_get(5)?,
                created_at: row.try_get(6)?,
            })
        })
        .collect::<Result<_, sqlx::Error>>()
        .map_err(|e| format!("Failed to read attachment row: {e}"))
}

pub async fn delete_draft_attachment(db: &UserDb, id: &str) -> Result<bool, String> {
    let mut connection = db.acquire().await?;
    sqlx::query("DELETE FROM draft_attachments WHERE user_hash = $1 AND id = $2")
        .bind(db.user_hash())
        .bind(id)
        .execute(&mut *connection)
        .await
        .map(|result| result.rows_affected() > 0)
        .map_err(|e| format!("Failed to delete draft attachment: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::pool::open_test_db;

    #[tokio::test]
    async fn test_upsert_and_get_draft() {
        let conn = open_test_db().await;

        upsert_draft(
            &conn,
            "draft-1",
            "bob@example.com",
            "",
            "",
            "Hello",
            "Hi Bob",
            None,
            None,
            None,
        )
        .await
        .unwrap();

        let draft = get_draft(&conn, "draft-1").await.unwrap().unwrap();
        assert_eq!(draft.id, "draft-1");
        assert_eq!(draft.to_addresses, "bob@example.com");
        assert_eq!(draft.subject, "Hello");
        assert_eq!(draft.text_body, "Hi Bob");
        assert!(draft.html_body.is_none());
    }

    #[tokio::test]
    async fn test_upsert_updates_existing() {
        let conn = open_test_db().await;

        upsert_draft(
            &conn,
            "draft-1",
            "bob@example.com",
            "",
            "",
            "v1",
            "body1",
            None,
            None,
            None,
        )
        .await
        .unwrap();
        upsert_draft(
            &conn,
            "draft-1",
            "carol@example.com",
            "",
            "",
            "v2",
            "body2",
            None,
            None,
            None,
        )
        .await
        .unwrap();

        let draft = get_draft(&conn, "draft-1").await.unwrap().unwrap();
        assert_eq!(draft.to_addresses, "carol@example.com");
        assert_eq!(draft.subject, "v2");
        assert_eq!(draft.text_body, "body2");
    }

    #[tokio::test]
    async fn test_list_drafts_returns_all() {
        let conn = open_test_db().await;

        upsert_draft(&conn, "draft-a", "", "", "", "First", "", None, None, None)
            .await
            .unwrap();
        upsert_draft(&conn, "draft-b", "", "", "", "Second", "", None, None, None)
            .await
            .unwrap();

        let drafts = list_drafts(&conn).await.unwrap();
        assert_eq!(drafts.len(), 2);
        let ids: Vec<&str> = drafts.iter().map(|d| d.id.as_str()).collect();
        assert!(ids.contains(&"draft-a"));
        assert!(ids.contains(&"draft-b"));
    }

    #[tokio::test]
    async fn test_delete_draft() {
        let conn = open_test_db().await;

        upsert_draft(&conn, "draft-1", "", "", "", "Test", "", None, None, None)
            .await
            .unwrap();
        assert!(get_draft(&conn, "draft-1").await.unwrap().is_some());

        let deleted = delete_draft(&conn, "draft-1").await.unwrap();
        assert!(deleted);
        assert!(get_draft(&conn, "draft-1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_delete_nonexistent_draft() {
        let conn = open_test_db().await;
        let deleted = delete_draft(&conn, "no-such-draft").await.unwrap();
        assert!(!deleted);
    }

    #[tokio::test]
    async fn test_get_nonexistent_draft() {
        let conn = open_test_db().await;
        assert!(get_draft(&conn, "no-such-draft").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_add_and_get_attachments() {
        let conn = open_test_db().await;

        upsert_draft(&conn, "draft-1", "", "", "", "Test", "", None, None, None)
            .await
            .unwrap();

        add_draft_attachment(
            &conn,
            "att-1",
            "draft-1",
            "file.pdf",
            "application/pdf",
            1024,
            "/data/abc/attachments/draft-1/att-1",
        )
        .await
        .unwrap();

        add_draft_attachment(
            &conn,
            "att-2",
            "draft-1",
            "photo.jpg",
            "image/jpeg",
            2048,
            "/data/abc/attachments/draft-1/att-2",
        )
        .await
        .unwrap();

        let atts = get_draft_attachments(&conn, "draft-1").await.unwrap();
        assert_eq!(atts.len(), 2);
        assert_eq!(atts[0].filename, "file.pdf");
        assert_eq!(atts[0].size, 1024);
        assert_eq!(atts[1].filename, "photo.jpg");
    }

    #[tokio::test]
    async fn test_delete_attachment() {
        let conn = open_test_db().await;

        upsert_draft(&conn, "draft-1", "", "", "", "Test", "", None, None, None)
            .await
            .unwrap();
        add_draft_attachment(
            &conn,
            "att-1",
            "draft-1",
            "file.pdf",
            "application/pdf",
            100,
            "/path",
        )
        .await
        .unwrap();

        let deleted = delete_draft_attachment(&conn, "att-1").await.unwrap();
        assert!(deleted);

        let atts = get_draft_attachments(&conn, "draft-1").await.unwrap();
        assert!(atts.is_empty());
    }

    #[tokio::test]
    async fn test_drafts_compose_inside_transaction() {
        let conn = open_test_db().await;
        let transaction = conn.begin_write().await.unwrap();
        upsert_draft(
            &transaction,
            "draft-tx",
            "",
            "",
            "",
            "Test",
            "",
            None,
            None,
            None,
        )
        .await
        .unwrap();
        add_draft_attachment(
            &transaction,
            "att-tx",
            "draft-tx",
            "file.txt",
            "text/plain",
            4,
            "/path",
        )
        .await
        .unwrap();
        assert!(get_draft(&transaction, "draft-tx").await.unwrap().is_some());
        transaction.commit().await.unwrap();
        assert_eq!(
            get_draft_attachments(&conn, "draft-tx")
                .await
                .unwrap()
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn test_cascade_delete_attachments_on_draft_delete() {
        let conn = open_test_db().await;

        upsert_draft(&conn, "draft-1", "", "", "", "Test", "", None, None, None)
            .await
            .unwrap();
        add_draft_attachment(
            &conn,
            "att-1",
            "draft-1",
            "file.pdf",
            "application/pdf",
            100,
            "/path",
        )
        .await
        .unwrap();
        add_draft_attachment(
            &conn,
            "att-2",
            "draft-1",
            "pic.jpg",
            "image/jpeg",
            200,
            "/path2",
        )
        .await
        .unwrap();

        delete_draft(&conn, "draft-1").await.unwrap();

        let atts = get_draft_attachments(&conn, "draft-1").await.unwrap();
        assert!(atts.is_empty());
    }
}
