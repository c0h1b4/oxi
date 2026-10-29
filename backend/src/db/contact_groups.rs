use serde::Serialize;
use sqlx::{Row, any::AnyRow};

use crate::db::contacts::{Contact, row_to_contact};
use crate::db::pool::{UserDb, now};

/// A contact group record, mirroring the `contact_groups` table.
#[derive(Debug, Clone, Serialize)]
pub struct ContactGroup {
    pub id: String,
    pub name: String,
    pub member_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

fn row_to_group(row: &AnyRow) -> Result<ContactGroup, sqlx::Error> {
    Ok(ContactGroup {
        id: row.try_get(0)?,
        name: row.try_get(1)?,
        created_at: row.try_get(2)?,
        updated_at: row.try_get(3)?,
        member_count: row.try_get(4)?,
    })
}

pub async fn create_group(db: &UserDb, id: &str, name: &str) -> Result<(), String> {
    let timestamp = now();
    let mut connection = db.acquire().await?;
    sqlx::query("INSERT INTO contact_groups (user_hash, id, name, created_at, updated_at) VALUES ($1, $2, $3, $4, $4)")
        .bind(db.user_hash()).bind(id).bind(name).bind(timestamp)
        .execute(&mut *connection).await.map_err(|e| format!("Failed to create contact group: {e}"))?;
    Ok(())
}

pub async fn list_groups(db: &UserDb) -> Result<Vec<ContactGroup>, String> {
    let mut connection = db.acquire().await?;
    let rows = sqlx::query(
        "SELECT g.id, g.name, g.created_at, g.updated_at,
                (SELECT COUNT(*) FROM contact_group_members m WHERE m.user_hash = g.user_hash AND m.group_id = g.id) AS member_count
         FROM contact_groups g WHERE g.user_hash = $1 ORDER BY g.name ASC",
    ).bind(db.user_hash()).fetch_all(&mut *connection).await
        .map_err(|e| format!("Failed to query groups: {e}"))?;
    rows.iter()
        .map(row_to_group)
        .collect::<Result<_, _>>()
        .map_err(|e| format!("Failed to read group row: {e}"))
}

pub async fn update_group(db: &UserDb, id: &str, name: &str) -> Result<bool, String> {
    let mut connection = db.acquire().await?;
    let result = sqlx::query(
        "UPDATE contact_groups SET name = $1, updated_at = $2 WHERE user_hash = $3 AND id = $4",
    )
    .bind(name)
    .bind(now())
    .bind(db.user_hash())
    .bind(id)
    .execute(&mut *connection)
    .await
    .map_err(|e| format!("Failed to update contact group: {e}"))?;
    Ok(result.rows_affected() > 0)
}

/// Delete a contact group. CASCADE handles member rows.
pub async fn delete_group(db: &UserDb, id: &str) -> Result<bool, String> {
    let mut connection = db.acquire().await?;
    let result = sqlx::query("DELETE FROM contact_groups WHERE user_hash = $1 AND id = $2")
        .bind(db.user_hash())
        .bind(id)
        .execute(&mut *connection)
        .await
        .map_err(|e| format!("Failed to delete contact group: {e}"))?;
    Ok(result.rows_affected() > 0)
}

pub async fn add_member(db: &UserDb, group_id: &str, contact_id: &str) -> Result<(), String> {
    let mut connection = db.acquire().await?;
    sqlx::query(
        "INSERT INTO contact_group_members (user_hash, group_id, contact_id) VALUES ($1, $2, $3)
         ON CONFLICT(user_hash, group_id, contact_id) DO NOTHING",
    )
    .bind(db.user_hash())
    .bind(group_id)
    .bind(contact_id)
    .execute(&mut *connection)
    .await
    .map_err(|e| format!("Failed to add group member: {e}"))?;
    Ok(())
}

pub async fn remove_member(db: &UserDb, group_id: &str, contact_id: &str) -> Result<bool, String> {
    let mut connection = db.acquire().await?;
    let result = sqlx::query("DELETE FROM contact_group_members WHERE user_hash = $1 AND group_id = $2 AND contact_id = $3")
        .bind(db.user_hash()).bind(group_id).bind(contact_id).execute(&mut *connection).await
        .map_err(|e| format!("Failed to remove group member: {e}"))?;
    Ok(result.rows_affected() > 0)
}

pub async fn list_group_members(db: &UserDb, group_id: &str) -> Result<Vec<Contact>, String> {
    let mut connection = db.acquire().await?;
    let rows = sqlx::query(
        "SELECT c.id, c.email, c.name, c.company, c.notes, CAST(c.is_favorite AS BIGINT),
                c.last_contacted, c.contact_count, c.source, c.created_at, c.updated_at
         FROM contacts c JOIN contact_group_members m ON m.user_hash = c.user_hash AND m.contact_id = c.id
         WHERE c.user_hash = $1 AND m.group_id = $2 ORDER BY c.name ASC",
    ).bind(db.user_hash()).bind(group_id).fetch_all(&mut *connection).await
        .map_err(|e| format!("Failed to query group members: {e}"))?;
    rows.iter()
        .map(row_to_contact)
        .collect::<Result<_, _>>()
        .map_err(|e| format!("Failed to read group member row: {e}"))
}

#[allow(dead_code)]
pub async fn list_contact_groups(
    db: &UserDb,
    contact_id: &str,
) -> Result<Vec<ContactGroup>, String> {
    let mut connection = db.acquire().await?;
    let rows = sqlx::query(
        "SELECT g.id, g.name, g.created_at, g.updated_at,
                (SELECT COUNT(*) FROM contact_group_members m2 WHERE m2.user_hash = g.user_hash AND m2.group_id = g.id) AS member_count
         FROM contact_groups g JOIN contact_group_members m ON m.user_hash = g.user_hash AND m.group_id = g.id
         WHERE g.user_hash = $1 AND m.contact_id = $2 ORDER BY g.name ASC",
    ).bind(db.user_hash()).bind(contact_id).fetch_all(&mut *connection).await
        .map_err(|e| format!("Failed to query contact groups: {e}"))?;
    rows.iter()
        .map(row_to_group)
        .collect::<Result<_, _>>()
        .map_err(|e| format!("Failed to read contact group row: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::contacts::{Contact as DbContact, upsert_contact};
    use crate::db::pool::open_test_db;

    fn sample_contact(id: &str, email: &str, name: &str) -> DbContact {
        DbContact {
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
    async fn test_create_and_list_groups() {
        let conn = open_test_db().await;
        create_group(&conn, "g1", "Friends").await.unwrap();
        create_group(&conn, "g2", "Work").await.unwrap();

        let groups = list_groups(&conn).await.unwrap();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].name, "Friends");
        assert_eq!(groups[1].name, "Work");
        assert_eq!(groups[0].member_count, 0);
    }

    #[tokio::test]
    async fn test_add_and_list_members() {
        let conn = open_test_db().await;
        create_group(&conn, "g1", "Friends").await.unwrap();
        upsert_contact(&conn, &sample_contact("c1", "alice@ex.com", "Alice"))
            .await
            .unwrap();
        upsert_contact(&conn, &sample_contact("c2", "bob@ex.com", "Bob"))
            .await
            .unwrap();

        add_member(&conn, "g1", "c1").await.unwrap();
        add_member(&conn, "g1", "c2").await.unwrap();
        // Duplicate add should be ignored
        add_member(&conn, "g1", "c1").await.unwrap();

        let members = list_group_members(&conn, "g1").await.unwrap();
        assert_eq!(members.len(), 2);

        let groups = list_groups(&conn).await.unwrap();
        assert_eq!(groups[0].member_count, 2);
    }

    #[tokio::test]
    async fn test_remove_member() {
        let conn = open_test_db().await;
        create_group(&conn, "g1", "Friends").await.unwrap();
        upsert_contact(&conn, &sample_contact("c1", "alice@ex.com", "Alice"))
            .await
            .unwrap();
        add_member(&conn, "g1", "c1").await.unwrap();

        let removed = remove_member(&conn, "g1", "c1").await.unwrap();
        assert!(removed);

        let members = list_group_members(&conn, "g1").await.unwrap();
        assert_eq!(members.len(), 0);
    }

    #[tokio::test]
    async fn test_delete_group_cascades() {
        let conn = open_test_db().await;
        create_group(&conn, "g1", "Friends").await.unwrap();
        upsert_contact(&conn, &sample_contact("c1", "alice@ex.com", "Alice"))
            .await
            .unwrap();
        add_member(&conn, "g1", "c1").await.unwrap();

        let deleted = delete_group(&conn, "g1").await.unwrap();
        assert!(deleted);

        let groups = list_groups(&conn).await.unwrap();
        assert_eq!(groups.len(), 0);
    }

    #[tokio::test]
    async fn test_list_contact_groups() {
        let conn = open_test_db().await;
        create_group(&conn, "g1", "Friends").await.unwrap();
        create_group(&conn, "g2", "Work").await.unwrap();
        upsert_contact(&conn, &sample_contact("c1", "alice@ex.com", "Alice"))
            .await
            .unwrap();
        add_member(&conn, "g1", "c1").await.unwrap();
        add_member(&conn, "g2", "c1").await.unwrap();

        let groups = list_contact_groups(&conn, "c1").await.unwrap();
        assert_eq!(groups.len(), 2);
    }

    #[tokio::test]
    async fn test_group_queries_ignore_other_users() {
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
        sqlx::query(
            "INSERT INTO contact_groups (user_hash, id, name) VALUES ($1, 'foreign', 'Foreign')",
        )
        .bind(&other)
        .execute(&mut *connection)
        .await
        .unwrap();
        sqlx::query("INSERT INTO contacts (user_hash, id, email, name) VALUES ($1, 'foreign-contact', 'foreign@test.com', 'Foreign')").bind(&other)
            .execute(&mut *connection).await.unwrap();
        sqlx::query("INSERT INTO contact_group_members (user_hash, group_id, contact_id) VALUES ($1, 'foreign', 'foreign-contact')").bind(&other)
            .execute(&mut *connection).await.unwrap();
        drop(connection);
        assert!(list_groups(&conn).await.unwrap().is_empty());
        assert!(
            list_group_members(&conn, "foreign")
                .await
                .unwrap()
                .is_empty()
        );
        assert!(
            list_contact_groups(&conn, "foreign-contact")
                .await
                .unwrap()
                .is_empty()
        );
        assert!(!update_group(&conn, "foreign", "Changed").await.unwrap());
        assert!(
            !remove_member(&conn, "foreign", "foreign-contact")
                .await
                .unwrap()
        );
        assert!(!delete_group(&conn, "foreign").await.unwrap());
    }

    #[tokio::test]
    async fn test_update_group() {
        let conn = open_test_db().await;
        create_group(&conn, "g1", "Friends").await.unwrap();

        let updated = update_group(&conn, "g1", "Close Friends").await.unwrap();
        assert!(updated);

        let groups = list_groups(&conn).await.unwrap();
        assert_eq!(groups[0].name, "Close Friends");
    }
}
