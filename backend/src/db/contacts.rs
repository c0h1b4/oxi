use serde::Serialize;
use sqlx::{Row, any::AnyRow};
use uuid::Uuid;

use crate::db::pool::{UserDb, now};

/// A known address extracted from message headers (not necessarily a contact).
#[derive(Debug, Clone, Serialize)]
pub struct KnownAddress {
    pub email: String,
    pub name: String,
}

/// A contact record, mirroring the `contacts` table.
#[derive(Debug, Clone, Serialize)]
pub struct Contact {
    pub id: String,
    pub email: String,
    pub name: String,
    pub company: String,
    pub notes: String,
    pub is_favorite: bool,
    pub last_contacted: Option<String>,
    pub contact_count: i64,
    pub source: String,
    pub created_at: String,
    pub updated_at: String,
}

pub(crate) fn row_to_contact(row: &AnyRow) -> Result<Contact, sqlx::Error> {
    Ok(Contact {
        id: row.try_get(0)?,
        email: row.try_get(1)?,
        name: row.try_get(2)?,
        company: row.try_get(3)?,
        notes: row.try_get(4)?,
        is_favorite: row.try_get::<i64, _>(5)? != 0,
        last_contacted: row.try_get(6)?,
        contact_count: row.try_get(7)?,
        source: row.try_get(8)?,
        created_at: row.try_get(9)?,
        updated_at: row.try_get(10)?,
    })
}

const CONTACT_SELECT_COLS: &str = "id, email, name, company, notes, CAST(is_favorite AS BIGINT), last_contacted, contact_count, source, created_at, updated_at";

// SQLite LIKE folds only ASCII by default. PostgreSQL LOWER also folds Unicode.
fn ascii_casefold(db: &UserDb, column: &str) -> String {
    if db.is_postgres() {
        format!("TRANSLATE({column}, 'ABCDEFGHIJKLMNOPQRSTUVWXYZ', 'abcdefghijklmnopqrstuvwxyz')")
    } else {
        format!("LOWER({column})")
    }
}

fn escape_like(term: &str) -> String {
    term.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// Update an existing id without deleting its group memberships. Replace an email collision as before.
pub async fn upsert_contact(db: &UserDb, contact: &Contact) -> Result<(), String> {
    let tx = db.begin_write().await?;
    let mut connection = tx.acquire().await?;
    let id = &contact.id;
    sqlx::query("DELETE FROM contacts WHERE user_hash = $1 AND email = $2 AND id <> $3")
        .bind(db.user_hash())
        .bind(&contact.email)
        .bind(id)
        .execute(&mut *connection)
        .await
        .map_err(|e| format!("Failed to replace contact email: {e}"))?;
    sqlx::query(
        "INSERT INTO contacts
            (user_hash, id, email, name, company, notes, is_favorite, last_contacted,
             contact_count, source, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
         ON CONFLICT(user_hash, id) DO UPDATE SET
             email = excluded.email, name = excluded.name, company = excluded.company,
             notes = excluded.notes, is_favorite = excluded.is_favorite,
             last_contacted = excluded.last_contacted, contact_count = excluded.contact_count,
             source = excluded.source, created_at = excluded.created_at, updated_at = excluded.updated_at",
    )
    .bind(db.user_hash()).bind(id).bind(&contact.email).bind(&contact.name)
    .bind(&contact.company).bind(&contact.notes).bind(i64::from(contact.is_favorite))
    .bind(&contact.last_contacted).bind(contact.contact_count).bind(&contact.source)
    .bind(&contact.created_at).bind(&contact.updated_at)
    .execute(&mut *connection).await.map_err(|e| format!("Failed to upsert contact: {e}"))?;
    drop(connection);
    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit contact upsert: {e}"))?;
    Ok(())
}

pub async fn get_contact(db: &UserDb, id: &str) -> Result<Option<Contact>, String> {
    let sql =
        format!("SELECT {CONTACT_SELECT_COLS} FROM contacts WHERE user_hash = $1 AND id = $2");
    let mut connection = db.acquire().await?;
    let row = sqlx::query(&sql)
        .bind(db.user_hash())
        .bind(id)
        .fetch_optional(&mut *connection)
        .await
        .map_err(|e| format!("Failed to get contact: {e}"))?;
    row.as_ref()
        .map(row_to_contact)
        .transpose()
        .map_err(|e| format!("Failed to read contact: {e}"))
}

pub async fn get_contact_by_email(db: &UserDb, email: &str) -> Result<Option<Contact>, String> {
    let sql =
        format!("SELECT {CONTACT_SELECT_COLS} FROM contacts WHERE user_hash = $1 AND email = $2");
    let mut connection = db.acquire().await?;
    let row = sqlx::query(&sql)
        .bind(db.user_hash())
        .bind(email)
        .fetch_optional(&mut *connection)
        .await
        .map_err(|e| format!("Failed to get contact by email: {e}"))?;
    row.as_ref()
        .map(row_to_contact)
        .transpose()
        .map_err(|e| format!("Failed to read contact: {e}"))
}

/// List contacts with an optional literal, case-insensitive name/email search.
pub async fn list_contacts(
    db: &UserDb,
    search: Option<&str>,
    limit: u32,
    offset: u32,
) -> Result<Vec<Contact>, String> {
    let mut connection = db.acquire().await?;
    let rows = match search.filter(|q| !q.is_empty()) {
        Some(q) => {
            let name = ascii_casefold(db, "name");
            let email = ascii_casefold(db, "email");
            let sql = format!(
                "SELECT {CONTACT_SELECT_COLS} FROM contacts WHERE user_hash = $1
                 AND ({name} LIKE $2 ESCAPE '\\' OR {email} LIKE $2 ESCAPE '\\')
                 ORDER BY name ASC LIMIT $3 OFFSET $4"
            );
            sqlx::query(&sql).bind(db.user_hash()).bind(format!("%{}%", escape_like(&q.to_ascii_lowercase())))
                .bind(i64::from(limit)).bind(i64::from(offset)).fetch_all(&mut *connection).await
        }
        None => {
            let sql = format!("SELECT {CONTACT_SELECT_COLS} FROM contacts WHERE user_hash = $1 ORDER BY name ASC LIMIT $2 OFFSET $3");
            sqlx::query(&sql).bind(db.user_hash()).bind(i64::from(limit)).bind(i64::from(offset))
                .fetch_all(&mut *connection).await
        }
    }.map_err(|e| format!("Failed to query contacts: {e}"))?;
    rows.iter()
        .map(row_to_contact)
        .collect::<Result<_, _>>()
        .map_err(|e| format!("Failed to read contact row: {e}"))
}

pub async fn delete_contact(db: &UserDb, id: &str) -> Result<bool, String> {
    let mut connection = db.acquire().await?;
    let result = sqlx::query("DELETE FROM contacts WHERE user_hash = $1 AND id = $2")
        .bind(db.user_hash())
        .bind(id)
        .execute(&mut *connection)
        .await
        .map_err(|e| format!("Failed to delete contact: {e}"))?;
    Ok(result.rows_affected() > 0)
}

pub async fn search_contacts(db: &UserDb, query: &str, limit: u32) -> Result<Vec<Contact>, String> {
    list_contacts(db, Some(query), limit, 0).await
}

/// Search known addresses, excluding this user's contacts.
pub async fn search_known_addresses(
    db: &UserDb,
    query: &str,
    limit: u32,
) -> Result<Vec<KnownAddress>, String> {
    let mut connection = db.acquire().await?;
    let name = ascii_casefold(db, "name");
    let email = ascii_casefold(db, "email");
    let sql = format!(
        r#"SELECT email, name FROM known_addresses
           WHERE user_hash = $1
             AND ({email} LIKE $2 ESCAPE '\' OR {name} LIKE $2 ESCAPE '\')
             AND email NOT IN (SELECT email FROM contacts WHERE user_hash = $1)
           ORDER BY email ASC LIMIT $3"#,
    );
    let rows = sqlx::query(&sql)
        .bind(db.user_hash())
        .bind(format!("%{}%", escape_like(&query.to_ascii_lowercase())))
        .bind(i64::from(limit))
        .fetch_all(&mut *connection)
        .await
        .map_err(|e| format!("Failed to query known addresses: {e}"))?;
    rows.iter()
        .map(|row| {
            Ok(KnownAddress {
                email: row.try_get(0)?,
                name: row.try_get(1)?,
            })
        })
        .collect::<Result<_, sqlx::Error>>()
        .map_err(|e| format!("Failed to read known address row: {e}"))
}

pub async fn populate_known_addresses(
    db: &UserDb,
    from_address: &str,
    from_name: &str,
    to_json: &str,
    cc_json: &str,
) -> Result<(), String> {
    if !from_address.is_empty() {
        insert_known_address(db, from_address, from_name).await?;
    }
    insert_addresses_from_json(db, to_json).await?;
    insert_addresses_from_json(db, cc_json).await?;
    Ok(())
}

async fn insert_known_address(db: &UserDb, email: &str, name: &str) -> Result<(), String> {
    let mut connection = db.acquire().await?;
    sqlx::query(
        "INSERT INTO known_addresses (user_hash, email, name) VALUES ($1, $2, $3)
         ON CONFLICT(user_hash, email) DO UPDATE SET name = excluded.name WHERE excluded.name != ''",
    )
    .bind(db.user_hash()).bind(email).bind(name).execute(&mut *connection).await
    .map_err(|e| format!("Failed to insert known address: {e}"))?;
    Ok(())
}

async fn insert_addresses_from_json(db: &UserDb, json: &str) -> Result<(), String> {
    let entries: Vec<serde_json::Value> = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return Ok(()),
    };
    for entry in &entries {
        let email = entry.get("address").and_then(|v| v.as_str()).unwrap_or("");
        if email.is_empty() {
            continue;
        }
        let name = entry.get("name").and_then(|v| v.as_str()).unwrap_or("");
        insert_known_address(db, email, name).await?;
    }
    Ok(())
}

#[allow(dead_code)]
pub async fn increment_contact_count(db: &UserDb, email: &str) -> Result<(), String> {
    let mut connection = db.acquire().await?;
    sqlx::query(
        "UPDATE contacts SET contact_count = contact_count + 1,
             last_contacted = $1, updated_at = $1 WHERE user_hash = $2 AND email = $3",
    )
    .bind(now())
    .bind(db.user_hash())
    .bind(email)
    .execute(&mut *connection)
    .await
    .map_err(|e| format!("Failed to increment contact count: {e}"))?;
    Ok(())
}

#[allow(dead_code)]
pub async fn auto_add_contact(db: &UserDb, email: &str, name: &str) -> Result<(), String> {
    let timestamp = now();
    let mut connection = db.acquire().await?;
    sqlx::query(
        "INSERT INTO contacts (user_hash, id, email, name, source, created_at, updated_at)
         VALUES ($1, $2, $3, $4, 'auto', $5, $5) ON CONFLICT(user_hash, email) DO NOTHING",
    )
    .bind(db.user_hash())
    .bind(Uuid::new_v4().to_string())
    .bind(email)
    .bind(name)
    .bind(timestamp)
    .execute(&mut *connection)
    .await
    .map_err(|e| format!("Failed to auto-add contact: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::pool::open_test_db;

    /// Helper: create a sample Contact with sensible defaults.
    fn sample_contact(id: &str, email: &str, name: &str) -> Contact {
        Contact {
            id: id.to_string(),
            email: email.to_string(),
            name: name.to_string(),
            company: String::new(),
            notes: String::new(),
            is_favorite: false,
            last_contacted: None,
            contact_count: 0,
            source: "manual".to_string(),
            created_at: "2024-01-01 00:00:00".to_string(),
            updated_at: "2024-01-01 00:00:00".to_string(),
        }
    }

    #[tokio::test]
    async fn test_upsert_and_get_contact() {
        let conn = open_test_db().await;
        let contact = sample_contact("c1", "alice@example.com", "Alice");

        upsert_contact(&conn, &contact).await.unwrap();

        let fetched = get_contact(&conn, "c1").await.unwrap();
        assert!(fetched.is_some());
        let fetched = fetched.unwrap();
        assert_eq!(fetched.id, "c1");
        assert_eq!(fetched.email, "alice@example.com");
        assert_eq!(fetched.name, "Alice");
        assert_eq!(fetched.source, "manual");
    }

    #[tokio::test]
    async fn test_get_contact_by_email() {
        let conn = open_test_db().await;
        let contact = sample_contact("c2", "bob@example.com", "Bob");
        upsert_contact(&conn, &contact).await.unwrap();

        let fetched = get_contact_by_email(&conn, "bob@example.com")
            .await
            .unwrap();
        assert!(fetched.is_some());
        let fetched = fetched.unwrap();
        assert_eq!(fetched.id, "c2");
        assert_eq!(fetched.name, "Bob");

        // Non-existent email returns None.
        let missing = get_contact_by_email(&conn, "nobody@example.com")
            .await
            .unwrap();
        assert!(missing.is_none());
    }

    #[tokio::test]
    async fn test_list_contacts() {
        let conn = open_test_db().await;
        upsert_contact(&conn, &sample_contact("c1", "alice@example.com", "Alice"))
            .await
            .unwrap();
        upsert_contact(&conn, &sample_contact("c2", "bob@example.com", "Bob"))
            .await
            .unwrap();

        let all = list_contacts(&conn, None, 100, 0).await.unwrap();
        assert_eq!(all.len(), 2);

        // Ordered by name ASC: Alice, Bob.
        assert_eq!(all[0].name, "Alice");
        assert_eq!(all[1].name, "Bob");
    }

    #[tokio::test]
    async fn test_search_contacts() {
        let conn = open_test_db().await;
        upsert_contact(
            &conn,
            &sample_contact("c1", "alice@example.com", "Alice Smith"),
        )
        .await
        .unwrap();
        upsert_contact(&conn, &sample_contact("c2", "bob@example.com", "Bob Jones"))
            .await
            .unwrap();

        // Search by name should find only Alice.
        let results = search_contacts(&conn, "Alice", 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Alice Smith");

        // Search by email should find only Bob.
        let results = search_contacts(&conn, "bob@", 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Bob Jones");
    }

    #[tokio::test]
    async fn test_search_is_case_insensitive_and_escapes_wildcards() {
        let conn = open_test_db().await;
        upsert_contact(
            &conn,
            &sample_contact("c1", "alice@example.com", r"Alice 100%_\"),
        )
        .await
        .unwrap();
        upsert_contact(&conn, &sample_contact("c2", "bob@example.com", "Bob 1000"))
            .await
            .unwrap();
        assert_eq!(search_contacts(&conn, "aLiCe", 10).await.unwrap().len(), 1);
        assert_eq!(search_contacts(&conn, r"%_\", 10).await.unwrap().len(), 1);
        populate_known_addresses(&conn, "Case@Test.com", r"100%_\", "[]", "[]")
            .await
            .unwrap();
        assert_eq!(
            search_known_addresses(&conn, "case@", 10)
                .await
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            search_known_addresses(&conn, r"%_\", 10)
                .await
                .unwrap()
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn test_upsert_replaces_email_collision_and_updates_existing_id() {
        let conn = open_test_db().await;
        upsert_contact(&conn, &sample_contact("c1", "same@example.com", "Old"))
            .await
            .unwrap();
        upsert_contact(&conn, &sample_contact("c2", "same@example.com", "New"))
            .await
            .unwrap();
        assert!(get_contact(&conn, "c1").await.unwrap().is_none());
        assert_eq!(get_contact(&conn, "c2").await.unwrap().unwrap().name, "New");
        upsert_contact(&conn, &sample_contact("c2", "updated@example.com", "Other"))
            .await
            .unwrap();
        upsert_contact(
            &conn,
            &sample_contact("c1", "updated@example.com", "Updated"),
        )
        .await
        .unwrap();
        assert!(get_contact(&conn, "c2").await.unwrap().is_none());
        assert!(
            get_contact_by_email(&conn, "same@example.com")
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(
            get_contact(&conn, "c1").await.unwrap().unwrap().email,
            "updated@example.com"
        );
    }

    #[tokio::test]
    async fn test_contact_queries_ignore_other_users() {
        let conn = open_test_db().await;
        let other = format!("other-{}", uuid::Uuid::new_v4());
        if conn.is_postgres() {
            let mut connection = conn.acquire().await.unwrap();
            sqlx::query("INSERT INTO users (user_hash) VALUES ($1)")
                .bind(&other)
                .execute(&mut *connection)
                .await
                .unwrap();
        }
        let mut connection = conn.acquire().await.unwrap();
        sqlx::query("INSERT INTO contacts (user_hash, id, email, name) VALUES ($1, 'foreign', 'foreign@test.com', 'Foreign')").bind(&other)
            .execute(&mut *connection).await.unwrap();
        sqlx::query("INSERT INTO known_addresses (user_hash, email, name) VALUES ($1, 'hidden@test.com', 'Hidden')").bind(&other)
            .execute(&mut *connection).await.unwrap();
        drop(connection);
        assert!(get_contact(&conn, "foreign").await.unwrap().is_none());
        assert!(
            get_contact_by_email(&conn, "foreign@test.com")
                .await
                .unwrap()
                .is_none()
        );
        assert!(list_contacts(&conn, None, 100, 0).await.unwrap().is_empty());
        assert!(
            search_contacts(&conn, "Foreign", 10)
                .await
                .unwrap()
                .is_empty()
        );
        assert!(!delete_contact(&conn, "foreign").await.unwrap());
        assert!(
            search_known_addresses(&conn, "test.com", 10)
                .await
                .unwrap()
                .is_empty()
        );
        populate_known_addresses(&conn, "foreign@test.com", "Own Known", "[]", "[]")
            .await
            .unwrap();
        let known = search_known_addresses(&conn, "test.com", 10).await.unwrap();
        assert_eq!(known.len(), 1);
        assert_eq!(known[0].name, "Own Known");
    }

    #[tokio::test]
    async fn test_delete_contact() {
        let conn = open_test_db().await;
        upsert_contact(&conn, &sample_contact("c1", "alice@example.com", "Alice"))
            .await
            .unwrap();

        let deleted = delete_contact(&conn, "c1").await.unwrap();
        assert!(deleted);

        // Should be gone now.
        let fetched = get_contact(&conn, "c1").await.unwrap();
        assert!(fetched.is_none());

        // Deleting again returns false.
        let deleted_again = delete_contact(&conn, "c1").await.unwrap();
        assert!(!deleted_again);
    }

    #[tokio::test]
    async fn test_auto_add_contact() {
        let conn = open_test_db().await;

        auto_add_contact(&conn, "auto@example.com", "Auto User")
            .await
            .unwrap();

        let fetched = get_contact_by_email(&conn, "auto@example.com")
            .await
            .unwrap();
        assert!(fetched.is_some());
        let fetched = fetched.unwrap();
        assert_eq!(fetched.source, "auto");
        assert_eq!(fetched.name, "Auto User");

        // Auto-adding the same email again should be a no-op (INSERT OR IGNORE).
        auto_add_contact(&conn, "auto@example.com", "Different Name")
            .await
            .unwrap();
        let fetched2 = get_contact_by_email(&conn, "auto@example.com")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(fetched2.name, "Auto User"); // Name should NOT change.
    }

    #[tokio::test]
    async fn test_increment_contact_count() {
        let conn = open_test_db().await;
        upsert_contact(&conn, &sample_contact("c1", "alice@example.com", "Alice"))
            .await
            .unwrap();

        // Increment twice.
        increment_contact_count(&conn, "alice@example.com")
            .await
            .unwrap();
        increment_contact_count(&conn, "alice@example.com")
            .await
            .unwrap();

        let fetched = get_contact_by_email(&conn, "alice@example.com")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(fetched.contact_count, 2);
        assert!(fetched.last_contacted.is_some());
    }

    #[tokio::test]
    async fn test_search_known_addresses_basic() {
        let conn = open_test_db().await;

        // Populate known_addresses directly (simulating what happens at sync time).
        populate_known_addresses(&conn, "alice@test.com", "Alice From", "[]", "[]")
            .await
            .unwrap();
        populate_known_addresses(
            &conn,
            "sender@other.com",
            "Sender",
            r#"[{"name":"Bob To","address":"bob@test.com"}]"#,
            "[]",
        )
        .await
        .unwrap();
        populate_known_addresses(
            &conn,
            "sender@other.com",
            "Sender",
            "[]",
            r#"[{"name":"Charlie Cc","address":"charlie@test.com"}]"#,
        )
        .await
        .unwrap();

        let results = search_known_addresses(&conn, "test.com", 10).await.unwrap();
        assert_eq!(results.len(), 3);

        assert_eq!(results[0].email, "alice@test.com");
        assert_eq!(results[0].name, "Alice From");
        assert_eq!(results[1].email, "bob@test.com");
        assert_eq!(results[1].name, "Bob To");
        assert_eq!(results[2].email, "charlie@test.com");
        assert_eq!(results[2].name, "Charlie Cc");
    }

    #[tokio::test]
    async fn test_search_known_addresses_excludes_contacts() {
        let conn = open_test_db().await;

        // Add alice@test.com as a contact.
        upsert_contact(
            &conn,
            &sample_contact("c1", "alice@test.com", "Alice Contact"),
        )
        .await
        .unwrap();

        // Populate known_addresses (simulating sync).
        populate_known_addresses(&conn, "alice@test.com", "Alice From", "[]", "[]")
            .await
            .unwrap();
        populate_known_addresses(
            &conn,
            "bob@test.com",
            "Bob From",
            r#"[{"name":"Alice To","address":"alice@test.com"}]"#,
            "[]",
        )
        .await
        .unwrap();
        populate_known_addresses(
            &conn,
            "charlie@test.com",
            "Charlie",
            "[]",
            r#"[{"name":"Alice Cc","address":"alice@test.com"}]"#,
        )
        .await
        .unwrap();

        // Search for "test.com" should exclude alice@test.com (she's a contact).
        let results = search_known_addresses(&conn, "test.com", 10).await.unwrap();

        // Should only have bob and charlie (alice excluded from all sources).
        assert_eq!(results.len(), 2);
        let emails: Vec<&str> = results.iter().map(|a| a.email.as_str()).collect();
        assert!(emails.contains(&"bob@test.com"));
        assert!(emails.contains(&"charlie@test.com"));
        assert!(!emails.contains(&"alice@test.com"));
    }

    #[tokio::test]
    async fn test_search_known_addresses_handles_invalid_json() {
        let conn = open_test_db().await;

        // Populate known_addresses (simulating sync).
        populate_known_addresses(&conn, "valid@test.com", "Valid User", "[]", "[]")
            .await
            .unwrap();
        populate_known_addresses(
            &conn,
            "sender@other.com",
            "Sender",
            r#"[{"name":"","address":"emptyname@test.com"}]"#,
            r#"[{"name":"Cc Person","address":"ccperson@test.com"}]"#,
        )
        .await
        .unwrap();
        populate_known_addresses(
            &conn,
            "sender@other.com",
            "Sender",
            r#"[{"name":"Json Person","address":"json@test.com"}]"#,
            "[]",
        )
        .await
        .unwrap();

        let results = search_known_addresses(&conn, "test.com", 10).await.unwrap();

        assert_eq!(results.len(), 4);
        let emails: Vec<&str> = results.iter().map(|a| a.email.as_str()).collect();
        assert!(emails.contains(&"valid@test.com"));
        assert!(emails.contains(&"emptyname@test.com"));
        assert!(emails.contains(&"ccperson@test.com"));
        assert!(emails.contains(&"json@test.com"));
    }

    #[tokio::test]
    async fn test_populate_known_addresses_deduplicates() {
        let conn = open_test_db().await;

        populate_known_addresses(&conn, "alice@test.com", "Alice", "[]", "[]")
            .await
            .unwrap();
        // Same email again with non-empty name -- should update the name.
        populate_known_addresses(&conn, "alice@test.com", "Alice Different", "[]", "[]")
            .await
            .unwrap();

        let results = search_known_addresses(&conn, "alice@test.com", 10)
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Alice Different");
    }

    #[tokio::test]
    async fn test_populate_known_addresses_skips_invalid_json() {
        let conn = open_test_db().await;

        // Invalid JSON in to_addresses should not cause an error.
        populate_known_addresses(&conn, "sender@test.com", "Sender", "not valid json", "[]")
            .await
            .unwrap();

        let results = search_known_addresses(&conn, "sender@test.com", 10)
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].email, "sender@test.com");
    }
}
