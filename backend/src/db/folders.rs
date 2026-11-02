use super::pool::{UserDb, now};
use serde::Serialize;
use sqlx::Row;

/// A cached IMAP folder, mirroring the `folders` table schema.
#[derive(Debug, Clone, Serialize)]
pub struct CachedFolder {
    pub name: String,
    pub delimiter: Option<String>,
    pub parent: Option<String>,
    pub flags: String,
    pub is_subscribed: bool,
    pub total_count: u32,
    pub unread_count: u32,
    pub uid_validity: u32,
    pub highest_modseq: u64,
}

fn folder_from_row(row: sqlx::any::AnyRow) -> Result<CachedFolder, String> {
    let integer = |index| -> Result<u32, String> {
        u32::try_from(row.try_get::<i64, _>(index).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())
    };
    Ok(CachedFolder {
        name: row.try_get(0).map_err(|e| e.to_string())?,
        delimiter: row.try_get(1).map_err(|e| e.to_string())?,
        parent: row.try_get(2).map_err(|e| e.to_string())?,
        flags: row.try_get(3).map_err(|e| e.to_string())?,
        is_subscribed: row.try_get::<i64, _>(4).map_err(|e| e.to_string())? != 0,
        total_count: integer(5)?,
        unread_count: integer(6)?,
        uid_validity: integer(7)?,
        highest_modseq: row
            .try_get::<String, _>(8)
            .map_err(|e| e.to_string())?
            .parse::<u64>()
            .map_err(|e| e.to_string())?,
    })
}

/// Update metadata without deleting cached messages or their tags.
#[allow(clippy::too_many_arguments)]
pub async fn upsert_folder(
    db: &UserDb,
    folder_name: &str,
    delimiter: Option<&str>,
    parent: Option<&str>,
    flags_csv: &str,
    is_subscribed: bool,
    total_count: u32,
    unread_count: u32,
    uid_validity: u32,
    highest_modseq: u64,
) -> Result<(), String> {
    let modseq = db.sql("$10", "CAST($10 AS NUMERIC)");
    let sql = format!(
        "INSERT INTO folders
        (user_hash, name, delimiter, parent, flags, is_subscribed, total_count,
         unread_count, uid_validity, highest_modseq, updated_at)
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,{modseq},$11)
        ON CONFLICT(user_hash, name) DO UPDATE SET
         delimiter=excluded.delimiter, parent=excluded.parent, flags=excluded.flags,
         is_subscribed=excluded.is_subscribed, total_count=excluded.total_count,
         unread_count=excluded.unread_count, uid_validity=excluded.uid_validity,
         highest_modseq=excluded.highest_modseq, updated_at=excluded.updated_at"
    );
    let mut connection = db.acquire().await?;
    sqlx::query(&sql)
        .bind(db.user_hash())
        .bind(folder_name)
        .bind(delimiter)
        .bind(parent)
        .bind(flags_csv)
        .bind(i64::from(is_subscribed))
        .bind(i64::from(total_count))
        .bind(i64::from(unread_count))
        .bind(i64::from(uid_validity))
        .bind(highest_modseq.to_string())
        .bind(now())
        .execute(&mut *connection)
        .await
        .map_err(|e| format!("Failed to upsert folder: {e}"))?;
    Ok(())
}

pub async fn insert_folder_if_new(
    db: &UserDb,
    folder_name: &str,
    delimiter: Option<&str>,
    flags_csv: &str,
) -> Result<(), String> {
    let mut connection = db.acquire().await?;
    sqlx::query(
        "INSERT INTO folders (user_hash, name, delimiter, flags, is_subscribed, updated_at)
        VALUES ($1,$2,$3,$4,1,$5) ON CONFLICT(user_hash,name) DO NOTHING",
    )
    .bind(db.user_hash())
    .bind(folder_name)
    .bind(delimiter)
    .bind(flags_csv)
    .bind(now())
    .execute(&mut *connection)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn get_all_folders(db: &UserDb) -> Result<Vec<CachedFolder>, String> {
    let mut connection = db.acquire().await?;
    sqlx::query(
        "SELECT name, delimiter, parent, flags, is_subscribed,
        total_count, unread_count, uid_validity, CAST(highest_modseq AS TEXT)
        FROM folders WHERE user_hash=$1 ORDER BY name",
    )
    .bind(db.user_hash())
    .fetch_all(&mut *connection)
    .await
    .map_err(|e| e.to_string())?
    .into_iter()
    .map(folder_from_row)
    .collect()
}

pub async fn remove_stale_folders(db: &UserDb, current_names: &[String]) -> Result<usize, String> {
    let transaction = db.begin_write().await?;
    let mut sql = "DELETE FROM folders WHERE user_hash=$1".to_string();
    if !current_names.is_empty() {
        let placeholders = (2..current_names.len() + 2)
            .map(|i| format!("${i}"))
            .collect::<Vec<_>>();
        sql.push_str(&format!(" AND name NOT IN ({})", placeholders.join(",")));
    }
    let mut connection = transaction.acquire().await?;
    let mut query = sqlx::query(&sql).bind(transaction.user_hash());
    for name in current_names {
        query = query.bind(name);
    }
    let deleted = query
        .execute(&mut *connection)
        .await
        .map_err(|e| e.to_string())?
        .rows_affected();
    drop(connection);
    if deleted > 0 {
        super::header_sync::invalidate(&transaction).await?;
    }
    transaction.commit().await?;
    usize::try_from(deleted).map_err(|e| e.to_string())
}

pub async fn touch_all_folders(db: &UserDb) -> Result<(), String> {
    let mut connection = db.acquire().await?;
    sqlx::query("UPDATE folders SET updated_at=$1 WHERE user_hash=$2")
        .bind(now())
        .bind(db.user_hash())
        .execute(&mut *connection)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn update_folder_status(
    db: &UserDb,
    name: &str,
    uid_validity: u32,
    total_count: u32,
) -> Result<(), String> {
    let mut connection = db.acquire().await?;
    sqlx::query("UPDATE folders SET uid_validity=$1,total_count=$2,messages_updated_at=$3 WHERE user_hash=$4 AND name=$5")
        .bind(i64::from(uid_validity)).bind(i64::from(total_count)).bind(now()).bind(db.user_hash()).bind(name)
        .execute(&mut *connection).await.map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn update_folder_sync_status(
    db: &UserDb,
    name: &str,
    uid_validity: u32,
    total_count: u32,
    highest_modseq: u64,
) -> Result<(), String> {
    let modseq = db.sql("$3", "CAST($3 AS NUMERIC)");
    let sql = format!(
        "UPDATE folders SET uid_validity=$1,total_count=$2,highest_modseq={modseq} WHERE user_hash=$4 AND name=$5"
    );
    let mut connection = db.acquire().await?;
    sqlx::query(&sql)
        .bind(i64::from(uid_validity))
        .bind(i64::from(total_count))
        .bind(highest_modseq.to_string())
        .bind(db.user_hash())
        .bind(name)
        .execute(&mut *connection)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn refresh_unread_count(db: &UserDb, folder_name: &str) -> Result<(), String> {
    let mut connection = db.acquire().await?;
    sqlx::query("UPDATE folders SET unread_count=(SELECT COUNT(*) FROM messages
        WHERE messages.user_hash=folders.user_hash AND messages.folder=folders.name AND flags NOT LIKE '%\\Seen%' ESCAPE '!')
        WHERE user_hash=$1 AND name=$2")
        .bind(db.user_hash()).bind(folder_name).execute(&mut *connection).await.map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn adjust_unread_count(db: &UserDb, folder_name: &str, delta: i32) -> Result<(), String> {
    let mut connection = db.acquire().await?;
    sqlx::query("UPDATE folders SET unread_count=CASE WHEN unread_count+$1 < 0 THEN 0 ELSE unread_count+$1 END WHERE user_hash=$2 AND name=$3")
        .bind(i64::from(delta)).bind(db.user_hash()).bind(folder_name)
        .execute(&mut *connection).await.map_err(|e| e.to_string())?;
    Ok(())
}

fn freshness_cutoff(max_age_secs: u32) -> String {
    (chrono::Utc::now() - chrono::Duration::seconds(i64::from(max_age_secs)))
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}

pub async fn is_folder_cache_fresh(db: &UserDb, max_age_secs: u32) -> Result<bool, String> {
    let mut connection = db.acquire().await?;
    let row = sqlx::query("SELECT 1 FROM folders WHERE user_hash=$1 AND updated_at>$2 LIMIT 1")
        .bind(db.user_hash())
        .bind(freshness_cutoff(max_age_secs))
        .fetch_optional(&mut *connection)
        .await
        .map_err(|e| e.to_string())?;
    Ok(row.is_some())
}

pub async fn is_folder_fresh(
    db: &UserDb,
    folder_name: &str,
    max_age_secs: u32,
) -> Result<bool, String> {
    let mut connection = db.acquire().await?;
    let row = sqlx::query(
        "SELECT 1 FROM folders WHERE user_hash=$1 AND name=$2 AND messages_updated_at>$3 LIMIT 1",
    )
    .bind(db.user_hash())
    .bind(folder_name)
    .bind(freshness_cutoff(max_age_secs))
    .fetch_optional(&mut *connection)
    .await
    .map_err(|e| e.to_string())?;
    Ok(row.is_some())
}

pub async fn is_folder_messages_invalidated(
    db: &UserDb,
    folder_name: &str,
) -> Result<bool, String> {
    let mut connection = db.acquire().await?;
    let row = sqlx::query(
        "SELECT 1 FROM folders WHERE user_hash=$1 AND name=$2 AND messages_updated_at IS NULL",
    )
    .bind(db.user_hash())
    .bind(folder_name)
    .fetch_optional(&mut *connection)
    .await
    .map_err(|e| e.to_string())?;
    Ok(row.is_some())
}

pub async fn invalidate_folder_freshness(db: &UserDb, folder_name: &str) -> Result<(), String> {
    let mut connection = db.acquire().await?;
    sqlx::query("UPDATE folders SET messages_updated_at=NULL WHERE user_hash=$1 AND name=$2")
        .bind(db.user_hash())
        .bind(folder_name)
        .execute(&mut *connection)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn get_folder(db: &UserDb, name: &str) -> Result<Option<CachedFolder>, String> {
    let mut connection = db.acquire().await?;
    sqlx::query("SELECT name,delimiter,parent,flags,is_subscribed,total_count,unread_count,uid_validity,CAST(highest_modseq AS TEXT)
        FROM folders WHERE user_hash=$1 AND name=$2")
        .bind(db.user_hash()).bind(name).fetch_optional(&mut *connection).await.map_err(|e| e.to_string())?
        .map(folder_from_row).transpose()
}

pub async fn delete_folder_and_messages(db: &UserDb, folder_name: &str) -> Result<(), String> {
    let transaction = db.begin_write().await?;
    let mut connection = transaction.acquire().await?;
    sqlx::query("DELETE FROM folders WHERE user_hash=$1 AND name=$2")
        .bind(transaction.user_hash())
        .bind(folder_name)
        .execute(&mut *connection)
        .await
        .map_err(|e| e.to_string())?;
    drop(connection);
    super::header_sync::invalidate(&transaction).await?;
    transaction.commit().await
}

/// Copy the destination rows before removing sources, so foreign keys remain valid.
/// Message bodies, tags, and descendants all move in one write transaction.
pub async fn rename_folder_in_cache(
    db: &UserDb,
    old_name: &str,
    new_name: &str,
) -> Result<(), String> {
    if old_name == new_name {
        return Ok(());
    }
    let transaction = db.begin_write().await?;
    let folders = get_all_folders(&transaction).await?;
    let delimiter = folders
        .iter()
        .find(|f| f.name == old_name)
        .and_then(|f| f.delimiter.as_deref());
    let prefix = delimiter.map(|d| format!("{old_name}{d}"));
    let renamed: Vec<_> = folders
        .iter()
        .filter(|f| f.name == old_name || prefix.as_ref().is_some_and(|p| f.name.starts_with(p)))
        .map(|f| {
            (
                f.name.clone(),
                format!("{new_name}{}", &f.name[old_name.len()..]),
            )
        })
        .collect();
    let mut connection = transaction.acquire().await?;
    for (old, new) in &renamed {
        sqlx::query("INSERT INTO folders (user_hash,name,delimiter,parent,flags,is_subscribed,total_count,unread_count,uid_validity,highest_modseq,updated_at,messages_updated_at)
            SELECT user_hash,$1,delimiter,parent,flags,is_subscribed,total_count,unread_count,uid_validity,highest_modseq,updated_at,messages_updated_at
            FROM folders WHERE user_hash=$2 AND name=$3")
            .bind(new).bind(transaction.user_hash()).bind(old).execute(&mut *connection).await.map_err(|e| e.to_string())?;
        sqlx::query("INSERT INTO messages (user_hash,uid,folder,message_id,in_reply_to,references_header,subject,from_address,from_name,to_addresses,cc_addresses,date,flags,size,has_attachments,snippet,body_cached,body_html,body_text,date_epoch,attachments_json,raw_headers,reaction,email_theme)
            SELECT user_hash,uid,$1,message_id,in_reply_to,references_header,subject,from_address,from_name,to_addresses,cc_addresses,date,flags,size,has_attachments,snippet,body_cached,body_html,body_text,date_epoch,attachments_json,raw_headers,reaction,email_theme
            FROM messages WHERE user_hash=$2 AND folder=$3")
            .bind(new).bind(transaction.user_hash()).bind(old).execute(&mut *connection).await.map_err(|e| e.to_string())?;
        sqlx::query("INSERT INTO message_tags (user_hash,tag_id,message_uid,message_folder,created_at)
            SELECT user_hash,tag_id,message_uid,$1,created_at FROM message_tags WHERE user_hash=$2 AND message_folder=$3")
            .bind(new).bind(transaction.user_hash()).bind(old).execute(&mut *connection).await.map_err(|e| e.to_string())?;
    }
    for (old, new) in &renamed {
        sqlx::query("UPDATE folders SET parent=$1 WHERE user_hash=$2 AND parent=$3")
            .bind(new)
            .bind(transaction.user_hash())
            .bind(old)
            .execute(&mut *connection)
            .await
            .map_err(|e| e.to_string())?;
        sqlx::query("DELETE FROM folders WHERE user_hash=$1 AND name=$2")
            .bind(transaction.user_hash())
            .bind(old)
            .execute(&mut *connection)
            .await
            .map_err(|e| e.to_string())?;
    }
    drop(connection);
    super::header_sync::invalidate(&transaction).await?;
    transaction.commit().await
}

pub async fn find_folder_by_attribute(
    db: &UserDb,
    attribute: &str,
) -> Result<Option<String>, String> {
    let mut connection = db.acquire().await?;
    sqlx::query(
        "SELECT name FROM folders WHERE user_hash=$1 AND flags LIKE '%' || $2 || '%' LIMIT 1",
    )
    .bind(db.user_hash())
    .bind(attribute)
    .fetch_optional(&mut *connection)
    .await
    .map_err(|e| e.to_string())?
    .map(|row| row.try_get(0).map_err(|e| e.to_string()))
    .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::pool::open_test_db;

    #[tokio::test]
    async fn test_upsert_and_get_folders() {
        let conn = open_test_db().await;

        upsert_folder(
            &conn,
            "INBOX",
            Some("/"),
            None,
            "\\HasNoChildren",
            true,
            42,
            5,
            100,
            200,
        )
        .await
        .unwrap();
        upsert_folder(
            &conn,
            "Sent",
            Some("/"),
            None,
            "\\Sent",
            true,
            10,
            0,
            101,
            300,
        )
        .await
        .unwrap();

        let folders = get_all_folders(&conn).await.unwrap();
        assert_eq!(folders.len(), 2);

        // Sorted alphabetically: INBOX < Sent
        assert_eq!(folders[0].name, "INBOX");
        assert_eq!(folders[0].total_count, 42);
        assert_eq!(folders[0].unread_count, 5);
        assert!(folders[0].is_subscribed);

        assert_eq!(folders[1].name, "Sent");
        assert_eq!(folders[1].highest_modseq, 300);
    }

    #[tokio::test]
    async fn test_upsert_updates_existing_folder() {
        let conn = open_test_db().await;

        upsert_folder(
            &conn,
            "INBOX",
            Some("/"),
            None,
            "\\HasNoChildren",
            true,
            10,
            2,
            100,
            50,
        )
        .await
        .unwrap();

        // Upsert again with different counts.
        upsert_folder(
            &conn,
            "INBOX",
            Some("/"),
            None,
            "\\HasNoChildren",
            true,
            99,
            33,
            100,
            75,
        )
        .await
        .unwrap();

        let folders = get_all_folders(&conn).await.unwrap();
        assert_eq!(folders.len(), 1);
        assert_eq!(folders[0].total_count, 99);
        assert_eq!(folders[0].unread_count, 33);
        assert_eq!(folders[0].highest_modseq, 75);
    }

    #[tokio::test]
    async fn sync_metadata_does_not_revalidate_invalidated_messages() {
        let conn = open_test_db().await;
        insert_folder_if_new(&conn, "Sent", None, "\\Sent")
            .await
            .unwrap();
        update_folder_status(&conn, "Sent", 1, 0).await.unwrap();
        assert!(is_folder_fresh(&conn, "Sent", 30).await.unwrap());

        invalidate_folder_freshness(&conn, "Sent").await.unwrap();
        update_folder_sync_status(&conn, "Sent", 2, 1, 42)
            .await
            .unwrap();

        assert!(is_folder_messages_invalidated(&conn, "Sent").await.unwrap());
        assert!(!is_folder_fresh(&conn, "Sent", 30).await.unwrap());
        assert_eq!(
            crate::db::messages::count_messages(&conn, "Sent")
                .await
                .unwrap(),
            0
        );
        let folder = get_folder(&conn, "Sent").await.unwrap().unwrap();
        assert_eq!(folder.uid_validity, 2);
        assert_eq!(folder.total_count, 1);
        assert_eq!(folder.highest_modseq, 42);
    }

    #[tokio::test]
    async fn sync_metadata_preserves_message_sync_timestamp() {
        let conn = open_test_db().await;
        insert_folder_if_new(&conn, "Sent", None, "\\Sent")
            .await
            .unwrap();
        {
            let mut connection = conn.acquire().await.unwrap();
            sqlx::query("UPDATE folders SET messages_updated_at = '2000-01-01 00:00:00' WHERE user_hash=$1 AND name='Sent'")
                .bind(conn.user_hash()).execute(&mut *connection).await.unwrap();
        }

        update_folder_sync_status(&conn, "Sent", 1, 5, 42)
            .await
            .unwrap();

        let timestamp: String = {
            let mut connection = conn.acquire().await.unwrap();
            sqlx::query(
                "SELECT messages_updated_at FROM folders WHERE user_hash=$1 AND name='Sent'",
            )
            .bind(conn.user_hash())
            .fetch_one(&mut *connection)
            .await
            .unwrap()
            .try_get(0)
            .unwrap()
        };
        assert_eq!(timestamp, "2000-01-01 00:00:00");
        assert!(!is_folder_fresh(&conn, "Sent", 30).await.unwrap());
    }

    #[tokio::test]
    async fn test_remove_stale_folders() {
        let conn = open_test_db().await;

        upsert_folder(&conn, "INBOX", None, None, "", true, 0, 0, 0, 0)
            .await
            .unwrap();
        upsert_folder(&conn, "Sent", None, None, "", true, 0, 0, 0, 0)
            .await
            .unwrap();
        upsert_folder(&conn, "Trash", None, None, "", true, 0, 0, 0, 0)
            .await
            .unwrap();

        // Keep only INBOX and Sent; Trash should be removed.
        let current = vec!["INBOX".to_string(), "Sent".to_string()];
        let deleted = remove_stale_folders(&conn, &current).await.unwrap();
        assert_eq!(deleted, 1);

        let folders = get_all_folders(&conn).await.unwrap();
        let names: Vec<&str> = folders.iter().map(|f| f.name.as_str()).collect();
        assert!(names.contains(&"INBOX"));
        assert!(names.contains(&"Sent"));
        assert!(!names.contains(&"Trash"));
    }

    #[tokio::test]
    async fn full_modseq_range_roundtrips() {
        let db = open_test_db().await;
        for value in [0, i64::MAX as u64, i64::MAX as u64 + 1, u64::MAX] {
            upsert_folder(
                &db,
                "INBOX",
                Some("/"),
                None,
                "",
                true,
                u32::MAX,
                u32::MAX,
                u32::MAX,
                value,
            )
            .await
            .unwrap();
            let folder = get_folder(&db, "INBOX").await.unwrap().unwrap();
            assert_eq!(folder.highest_modseq, value);
            assert_eq!(folder.total_count, u32::MAX);
            assert_eq!(folder.uid_validity, u32::MAX);
            update_folder_sync_status(&db, "INBOX", 1, 0, value)
                .await
                .unwrap();
            assert_eq!(get_all_folders(&db).await.unwrap()[0].highest_modseq, value);
        }
    }

    #[tokio::test]
    async fn rename_moves_descendants_bodies_tags_and_rolls_back_conflicts() {
        use crate::db::messages::{
            cache_message_body, get_cached_body, get_single_message, upsert_message,
        };
        let db = open_test_db().await;
        upsert_folder(&db, "Old", Some("/"), None, "", true, 1, 0, 1, u64::MAX)
            .await
            .unwrap();
        upsert_folder(
            &db,
            "Old/Child",
            Some("/"),
            Some("Old"),
            "",
            true,
            0,
            0,
            1,
            0,
        )
        .await
        .unwrap();
        upsert_message(
            &db,
            "Old",
            u32::MAX,
            None,
            None,
            None,
            "subject",
            "a@b",
            "a",
            "[]",
            "[]",
            "2024-01-01",
            "",
            u32::MAX,
            false,
            "",
            None,
        )
        .await
        .unwrap();
        cache_message_body(&db, "Old", u32::MAX, None, Some("cached"), None, None, None)
            .await
            .unwrap();
        {
            let mut connection = db.acquire().await.unwrap();
            sqlx::query("INSERT INTO tags (user_hash,id,name) VALUES ($1,'tag','Tag')")
                .bind(db.user_hash())
                .execute(&mut *connection)
                .await
                .unwrap();
            sqlx::query("INSERT INTO message_tags (user_hash,tag_id,message_uid,message_folder) VALUES ($1,'tag',$2,'Old')")
                .bind(db.user_hash()).bind(i64::from(u32::MAX)).execute(&mut *connection).await.unwrap();
        }
        rename_folder_in_cache(&db, "Old", "New").await.unwrap();
        assert!(get_folder(&db, "Old").await.unwrap().is_none());
        assert_eq!(
            get_folder(&db, "New/Child")
                .await
                .unwrap()
                .unwrap()
                .parent
                .as_deref(),
            Some("New")
        );
        assert_eq!(
            get_cached_body(&db, "New", u32::MAX)
                .await
                .unwrap()
                .unwrap()
                .text
                .as_deref(),
            Some("cached")
        );
        assert_eq!(
            get_single_message(&db, "New", u32::MAX)
                .await
                .unwrap()
                .unwrap()
                .size,
            u32::MAX
        );
        {
            let mut connection = db.acquire().await.unwrap();
            let row = sqlx::query(
                "SELECT message_folder FROM message_tags WHERE user_hash=$1 AND tag_id='tag'",
            )
            .bind(db.user_hash())
            .fetch_one(&mut *connection)
            .await
            .unwrap();
            assert_eq!(row.try_get::<String, _>(0).unwrap(), "New");
        }
        insert_folder_if_new(&db, "Taken", None, "").await.unwrap();
        assert!(rename_folder_in_cache(&db, "New", "Taken").await.is_err());
        assert!(get_folder(&db, "New").await.unwrap().is_some());
        assert!(
            get_single_message(&db, "New", u32::MAX)
                .await
                .unwrap()
                .is_some()
        );
    }
}
