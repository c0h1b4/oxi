use rusqlite::{Connection, params};
use serde::Serialize;

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

/// Insert or update a draft. Uses INSERT OR REPLACE to upsert.
#[allow(clippy::too_many_arguments)]
pub fn upsert_draft(
    conn: &Connection,
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
    conn.execute(
        "INSERT INTO drafts (id, to_addresses, cc_addresses, bcc_addresses, subject, text_body, html_body, in_reply_to, references_header, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, datetime('now'))
         ON CONFLICT(id) DO UPDATE SET
             to_addresses = excluded.to_addresses,
             cc_addresses = excluded.cc_addresses,
             bcc_addresses = excluded.bcc_addresses,
             subject = excluded.subject,
             text_body = excluded.text_body,
             html_body = excluded.html_body,
             in_reply_to = excluded.in_reply_to,
             references_header = excluded.references_header,
             updated_at = datetime('now')",
        params![
            id,
            to_addresses,
            cc_addresses,
            bcc_addresses,
            subject,
            text_body,
            html_body,
            in_reply_to,
            references_header,
        ],
    )
    .map_err(|e| format!("Failed to upsert draft: {e}"))?;
    Ok(())
}

/// Retrieve a single draft by ID.
pub fn get_draft(conn: &Connection, id: &str) -> Result<Option<Draft>, String> {
    let result = conn.query_row(
        "SELECT id, to_addresses, cc_addresses, bcc_addresses, subject, text_body, html_body, in_reply_to, references_header, created_at, updated_at
         FROM drafts WHERE id = ?1",
        params![id],
        |row| {
            Ok(Draft {
                id: row.get(0)?,
                to_addresses: row.get(1)?,
                cc_addresses: row.get(2)?,
                bcc_addresses: row.get(3)?,
                subject: row.get(4)?,
                text_body: row.get(5)?,
                html_body: row.get(6)?,
                in_reply_to: row.get(7)?,
                references_header: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            })
        },
    );

    match result {
        Ok(draft) => Ok(Some(draft)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(format!("Failed to get draft: {e}")),
    }
}

/// List all drafts, ordered by updated_at descending.
pub fn list_drafts(conn: &Connection) -> Result<Vec<Draft>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, to_addresses, cc_addresses, bcc_addresses, subject, text_body, html_body, in_reply_to, references_header, created_at, updated_at
             FROM drafts ORDER BY updated_at DESC",
        )
        .map_err(|e| format!("Failed to prepare list_drafts: {e}"))?;

    let rows = stmt
        .query_map([], |row| {
            Ok(Draft {
                id: row.get(0)?,
                to_addresses: row.get(1)?,
                cc_addresses: row.get(2)?,
                bcc_addresses: row.get(3)?,
                subject: row.get(4)?,
                text_body: row.get(5)?,
                html_body: row.get(6)?,
                in_reply_to: row.get(7)?,
                references_header: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            })
        })
        .map_err(|e| format!("Failed to query drafts: {e}"))?;

    let mut drafts = Vec::new();
    for row in rows {
        drafts.push(row.map_err(|e| format!("Failed to read draft row: {e}"))?);
    }
    Ok(drafts)
}

/// Delete a draft by ID. Cascade will handle attachments in DB.
pub fn delete_draft(conn: &Connection, id: &str) -> Result<bool, String> {
    let deleted = conn
        .execute("DELETE FROM drafts WHERE id = ?1", params![id])
        .map_err(|e| format!("Failed to delete draft: {e}"))?;
    Ok(deleted > 0)
}

/// Add an attachment record for a draft.
pub fn add_draft_attachment(
    conn: &Connection,
    id: &str,
    draft_id: &str,
    filename: &str,
    content_type: &str,
    size: i64,
    file_path: &str,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO draft_attachments (id, draft_id, filename, content_type, size, file_path)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, draft_id, filename, content_type, size, file_path],
    )
    .map_err(|e| format!("Failed to add draft attachment: {e}"))?;
    Ok(())
}

/// Get all attachments for a draft.
pub fn get_draft_attachments(
    conn: &Connection,
    draft_id: &str,
) -> Result<Vec<DraftAttachment>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, draft_id, filename, content_type, size, file_path, created_at
             FROM draft_attachments WHERE draft_id = ?1 ORDER BY created_at ASC",
        )
        .map_err(|e| format!("Failed to prepare get_draft_attachments: {e}"))?;

    let rows = stmt
        .query_map(params![draft_id], |row| {
            Ok(DraftAttachment {
                id: row.get(0)?,
                draft_id: row.get(1)?,
                filename: row.get(2)?,
                content_type: row.get(3)?,
                size: row.get(4)?,
                file_path: row.get(5)?,
                created_at: row.get(6)?,
            })
        })
        .map_err(|e| format!("Failed to query draft attachments: {e}"))?;

    let mut attachments = Vec::new();
    for row in rows {
        attachments.push(row.map_err(|e| format!("Failed to read attachment row: {e}"))?);
    }
    Ok(attachments)
}

/// Delete a single attachment by ID.
pub fn delete_draft_attachment(conn: &Connection, id: &str) -> Result<bool, String> {
    let deleted = conn
        .execute("DELETE FROM draft_attachments WHERE id = ?1", params![id])
        .map_err(|e| format!("Failed to delete draft attachment: {e}"))?;
    Ok(deleted > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::pool::open_test_db;

    #[test]
    fn test_upsert_and_get_draft() {
        let conn = open_test_db();

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
        .unwrap();

        let draft = get_draft(&conn, "draft-1").unwrap().unwrap();
        assert_eq!(draft.id, "draft-1");
        assert_eq!(draft.to_addresses, "bob@example.com");
        assert_eq!(draft.subject, "Hello");
        assert_eq!(draft.text_body, "Hi Bob");
        assert!(draft.html_body.is_none());
    }

    #[test]
    fn test_upsert_updates_existing() {
        let conn = open_test_db();

        upsert_draft(&conn, "draft-1", "bob@example.com", "", "", "v1", "body1", None, None, None).unwrap();
        upsert_draft(&conn, "draft-1", "carol@example.com", "", "", "v2", "body2", None, None, None).unwrap();

        let draft = get_draft(&conn, "draft-1").unwrap().unwrap();
        assert_eq!(draft.to_addresses, "carol@example.com");
        assert_eq!(draft.subject, "v2");
        assert_eq!(draft.text_body, "body2");
    }

    #[test]
    fn test_list_drafts_returns_all() {
        let conn = open_test_db();

        upsert_draft(&conn, "draft-a", "", "", "", "First", "", None, None, None).unwrap();
        upsert_draft(&conn, "draft-b", "", "", "", "Second", "", None, None, None).unwrap();

        let drafts = list_drafts(&conn).unwrap();
        assert_eq!(drafts.len(), 2);
        let ids: Vec<&str> = drafts.iter().map(|d| d.id.as_str()).collect();
        assert!(ids.contains(&"draft-a"));
        assert!(ids.contains(&"draft-b"));
    }

    #[test]
    fn test_delete_draft() {
        let conn = open_test_db();

        upsert_draft(&conn, "draft-1", "", "", "", "Test", "", None, None, None).unwrap();
        assert!(get_draft(&conn, "draft-1").unwrap().is_some());

        let deleted = delete_draft(&conn, "draft-1").unwrap();
        assert!(deleted);
        assert!(get_draft(&conn, "draft-1").unwrap().is_none());
    }

    #[test]
    fn test_delete_nonexistent_draft() {
        let conn = open_test_db();
        let deleted = delete_draft(&conn, "no-such-draft").unwrap();
        assert!(!deleted);
    }

    #[test]
    fn test_get_nonexistent_draft() {
        let conn = open_test_db();
        assert!(get_draft(&conn, "no-such-draft").unwrap().is_none());
    }

    #[test]
    fn test_add_and_get_attachments() {
        let conn = open_test_db();

        upsert_draft(&conn, "draft-1", "", "", "", "Test", "", None, None, None).unwrap();

        add_draft_attachment(
            &conn,
            "att-1",
            "draft-1",
            "file.pdf",
            "application/pdf",
            1024,
            "/data/abc/attachments/draft-1/att-1",
        )
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
        .unwrap();

        let atts = get_draft_attachments(&conn, "draft-1").unwrap();
        assert_eq!(atts.len(), 2);
        assert_eq!(atts[0].filename, "file.pdf");
        assert_eq!(atts[0].size, 1024);
        assert_eq!(atts[1].filename, "photo.jpg");
    }

    #[test]
    fn test_delete_attachment() {
        let conn = open_test_db();

        upsert_draft(&conn, "draft-1", "", "", "", "Test", "", None, None, None).unwrap();
        add_draft_attachment(&conn, "att-1", "draft-1", "file.pdf", "application/pdf", 100, "/path").unwrap();

        let deleted = delete_draft_attachment(&conn, "att-1").unwrap();
        assert!(deleted);

        let atts = get_draft_attachments(&conn, "draft-1").unwrap();
        assert!(atts.is_empty());
    }

    #[test]
    fn test_cascade_delete_attachments_on_draft_delete() {
        let conn = open_test_db();

        upsert_draft(&conn, "draft-1", "", "", "", "Test", "", None, None, None).unwrap();
        add_draft_attachment(&conn, "att-1", "draft-1", "file.pdf", "application/pdf", 100, "/path").unwrap();
        add_draft_attachment(&conn, "att-2", "draft-1", "pic.jpg", "image/jpeg", 200, "/path2").unwrap();

        delete_draft(&conn, "draft-1").unwrap();

        let atts = get_draft_attachments(&conn, "draft-1").unwrap();
        assert!(atts.is_empty());
    }
}
