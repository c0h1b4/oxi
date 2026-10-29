use sqlx::Row;

use crate::db::pool::{UserDb, now};
use serde::{Deserialize, Serialize};

/// A sender identity record, mirroring the `identities` table.
#[derive(Debug, Clone, Serialize)]
pub struct Identity {
    pub id: i64,
    pub display_name: String,
    pub email: String,
    pub signature_html: String,
    pub is_default: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// Fields for creating a new identity.
#[derive(Debug, Deserialize)]
pub struct CreateIdentity {
    #[serde(default)]
    pub display_name: String,
    pub email: String,
    #[serde(default)]
    pub signature_html: String,
    #[serde(default)]
    pub is_default: bool,
}

/// Fields for updating an existing identity. All are optional.
#[derive(Debug, Deserialize)]
pub struct UpdateIdentity {
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub signature_html: Option<String>,
    pub is_default: Option<bool>,
}

fn row_to_identity(row: &sqlx::any::AnyRow) -> Result<Identity, sqlx::Error> {
    Ok(Identity {
        id: row.try_get(0)?,
        display_name: row.try_get(1)?,
        email: row.try_get(2)?,
        signature_html: row.try_get(3)?,
        is_default: row.try_get::<i64, _>(4)? != 0,
        created_at: row.try_get(5)?,
        updated_at: row.try_get(6)?,
    })
}

const IDENTITY_SELECT_COLS: &str =
    "id, display_name, email, signature_html, is_default, created_at, updated_at";

/// List this user's identities, with the default identity first.
pub async fn list_identities(db: &UserDb) -> Result<Vec<Identity>, String> {
    let sql = format!(
        "SELECT {IDENTITY_SELECT_COLS} FROM identities WHERE user_hash = $1
         ORDER BY is_default DESC, email ASC"
    );
    let mut conn = db.acquire().await?;
    let rows = sqlx::query(&sql)
        .bind(db.user_hash())
        .fetch_all(&mut *conn)
        .await
        .map_err(|e| format!("Failed to query identities: {e}"))?;
    rows.iter()
        .map(row_to_identity)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Failed to read identity row: {e}"))
}

/// Get a single identity owned by this user.
pub async fn get_identity(db: &UserDb, id: i64) -> Result<Option<Identity>, String> {
    let sql =
        format!("SELECT {IDENTITY_SELECT_COLS} FROM identities WHERE user_hash = $1 AND id = $2");
    let mut conn = db.acquire().await?;
    let row = sqlx::query(&sql)
        .bind(db.user_hash())
        .bind(id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| format!("Failed to get identity: {e}"))?;
    row.as_ref()
        .map(row_to_identity)
        .transpose()
        .map_err(|e| format!("Failed to read identity row: {e}"))
}

/// Get this user's default identity.
#[allow(dead_code)]
pub async fn get_default_identity(db: &UserDb) -> Result<Option<Identity>, String> {
    let sql = format!(
        "SELECT {IDENTITY_SELECT_COLS} FROM identities
         WHERE user_hash = $1 AND is_default = 1 ORDER BY id LIMIT 1"
    );
    let mut conn = db.acquire().await?;
    let row = sqlx::query(&sql)
        .bind(db.user_hash())
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| format!("Failed to get default identity: {e}"))?;
    row.as_ref()
        .map(row_to_identity)
        .transpose()
        .map_err(|e| format!("Failed to read identity row: {e}"))
}

/// Create an identity, atomically clearing the previous default when requested.
pub async fn create_identity(db: &UserDb, data: &CreateIdentity) -> Result<Identity, String> {
    let tx = db.begin_write().await?;
    if data.is_default {
        clear_default(&tx).await?;
    }
    let mut conn = tx.acquire().await?;
    let row = sqlx::query(
        "INSERT INTO identities
             (user_hash, display_name, email, signature_html, is_default, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
    )
    .bind(tx.user_hash())
    .bind(&data.display_name)
    .bind(&data.email)
    .bind(&data.signature_html)
    .bind(i64::from(data.is_default))
    .bind(now())
    .bind(now())
    .fetch_one(&mut *conn)
    .await
    .map_err(|e| format!("Failed to create identity: {e}"))?;
    let id = row
        .try_get(0)
        .map_err(|e| format!("Failed to read created identity ID: {e}"))?;
    drop(conn);
    let identity = get_identity(&tx, id)
        .await?
        .ok_or_else(|| "Failed to read back created identity".to_string())?;
    tx.commit().await?;
    Ok(identity)
}

/// Change only the supplied fields. Default identity changes are atomic.
pub async fn update_identity(
    db: &UserDb,
    id: i64,
    data: &UpdateIdentity,
) -> Result<Option<Identity>, String> {
    let tx = db.begin_write().await?;
    let existing = get_identity(&tx, id).await?;
    if existing.is_none() {
        tx.commit().await?;
        return Ok(None);
    }
    if data.display_name.is_none()
        && data.email.is_none()
        && data.signature_html.is_none()
        && data.is_default.is_none()
    {
        tx.commit().await?;
        return Ok(existing);
    }
    if data.is_default == Some(true) {
        clear_default(&tx).await?;
    }

    let mut conn = tx.acquire().await?;
    sqlx::query(
        "UPDATE identities SET
             display_name = COALESCE($1, display_name),
             email = COALESCE($2, email),
             signature_html = COALESCE($3, signature_html),
             is_default = COALESCE($4, is_default),
             updated_at = $5
         WHERE user_hash = $6 AND id = $7",
    )
    .bind(data.display_name.as_deref())
    .bind(data.email.as_deref())
    .bind(data.signature_html.as_deref())
    .bind(data.is_default.map(i64::from))
    .bind(now())
    .bind(tx.user_hash())
    .bind(id)
    .execute(&mut *conn)
    .await
    .map_err(|e| format!("Failed to update identity: {e}"))?;
    drop(conn);
    let identity = get_identity(&tx, id).await?;
    tx.commit().await?;
    Ok(identity)
}

/// Delete an identity owned by this user.
pub async fn delete_identity(db: &UserDb, id: i64) -> Result<bool, String> {
    let mut conn = db.acquire().await?;
    let deleted = sqlx::query("DELETE FROM identities WHERE user_hash = $1 AND id = $2")
        .bind(db.user_hash())
        .bind(id)
        .execute(&mut *conn)
        .await
        .map_err(|e| format!("Failed to delete identity: {e}"))?;
    Ok(deleted.rows_affected() > 0)
}

/// Check whether this user has any identities.
pub async fn has_identities(db: &UserDb) -> Result<bool, String> {
    let mut conn = db.acquire().await?;
    let row = sqlx::query("SELECT COUNT(*) FROM identities WHERE user_hash = $1")
        .bind(db.user_hash())
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| format!("Failed to count identities: {e}"))?;
    let count: i64 = row.try_get(0).map_err(|e| e.to_string())?;
    Ok(count > 0)
}

async fn clear_default(db: &UserDb) -> Result<(), String> {
    let mut conn = db.acquire().await?;
    sqlx::query(
        "UPDATE identities SET is_default = 0, updated_at = $1
         WHERE user_hash = $2 AND is_default = 1",
    )
    .bind(now())
    .bind(db.user_hash())
    .execute(&mut *conn)
    .await
    .map_err(|e| format!("Failed to clear default identity: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::pool::open_test_db;

    #[tokio::test]
    async fn test_create_and_list_identities() {
        let conn = open_test_db().await;

        let identity = create_identity(
            &conn,
            &CreateIdentity {
                display_name: "Alice".to_string(),
                email: "alice@example.com".to_string(),
                signature_html: "<p>Best,<br>Alice</p>".to_string(),
                is_default: true,
            },
        )
        .await
        .unwrap();

        assert_eq!(identity.display_name, "Alice");
        assert_eq!(identity.email, "alice@example.com");
        assert!(identity.is_default);

        let all = list_identities(&conn).await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].email, "alice@example.com");
    }

    #[tokio::test]
    async fn test_only_one_default() {
        let conn = open_test_db().await;

        let first = create_identity(
            &conn,
            &CreateIdentity {
                display_name: "Alice".to_string(),
                email: "alice@example.com".to_string(),
                signature_html: String::new(),
                is_default: true,
            },
        )
        .await
        .unwrap();

        let second = create_identity(
            &conn,
            &CreateIdentity {
                display_name: "Bob".to_string(),
                email: "bob@example.com".to_string(),
                signature_html: String::new(),
                is_default: true,
            },
        )
        .await
        .unwrap();

        // Second should be default, first should not.
        assert!(second.is_default);
        let refreshed_first = get_identity(&conn, first.id).await.unwrap().unwrap();
        assert!(!refreshed_first.is_default);
    }

    #[tokio::test]
    async fn test_update_identity() {
        let conn = open_test_db().await;

        let identity = create_identity(
            &conn,
            &CreateIdentity {
                display_name: "Alice".to_string(),
                email: "alice@example.com".to_string(),
                signature_html: String::new(),
                is_default: false,
            },
        )
        .await
        .unwrap();

        let updated = update_identity(
            &conn,
            identity.id,
            &UpdateIdentity {
                display_name: Some("Alice Smith".to_string()),
                email: None,
                signature_html: None,
                is_default: None,
            },
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(updated.display_name, "Alice Smith");
        assert_eq!(updated.email, "alice@example.com");
    }

    #[tokio::test]
    async fn test_delete_identity() {
        let conn = open_test_db().await;

        let identity = create_identity(
            &conn,
            &CreateIdentity {
                display_name: "Alice".to_string(),
                email: "alice@example.com".to_string(),
                signature_html: String::new(),
                is_default: false,
            },
        )
        .await
        .unwrap();

        assert!(delete_identity(&conn, identity.id).await.unwrap());
        assert!(get_identity(&conn, identity.id).await.unwrap().is_none());
        assert!(!delete_identity(&conn, identity.id).await.unwrap());
    }

    #[tokio::test]
    async fn test_has_identities() {
        let conn = open_test_db().await;
        assert!(!has_identities(&conn).await.unwrap());

        create_identity(
            &conn,
            &CreateIdentity {
                display_name: "Alice".to_string(),
                email: "alice@example.com".to_string(),
                signature_html: String::new(),
                is_default: true,
            },
        )
        .await
        .unwrap();

        assert!(has_identities(&conn).await.unwrap());
    }

    #[tokio::test]
    async fn test_failed_default_creation_preserves_previous_default() {
        let db = open_test_db().await;
        let original = create_identity(
            &db,
            &CreateIdentity {
                display_name: "Alice".to_string(),
                email: "alice@example.com".to_string(),
                signature_html: String::new(),
                is_default: true,
            },
        )
        .await
        .unwrap();
        let transaction = db.begin_write().await.unwrap();
        clear_default(&transaction).await.unwrap();
        let mut connection = transaction.acquire().await.unwrap();
        let result = sqlx::query("INSERT INTO identities (user_hash, email) VALUES ($1, NULL)")
            .bind(transaction.user_hash())
            .execute(&mut *connection)
            .await;
        assert!(result.is_err());
        drop(connection);
        drop(transaction);
        assert_eq!(
            get_default_identity(&db).await.unwrap().unwrap().id,
            original.id
        );
        assert_eq!(list_identities(&db).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_concurrent_default_creations_leave_one_default() {
        let db = open_test_db().await;
        let alice = CreateIdentity {
            display_name: "Alice".to_string(),
            email: "alice@example.com".to_string(),
            signature_html: String::new(),
            is_default: true,
        };
        let bob = CreateIdentity {
            display_name: "Bob".to_string(),
            email: "bob@example.com".to_string(),
            signature_html: String::new(),
            is_default: true,
        };
        let (first, second) =
            tokio::join!(create_identity(&db, &alice), create_identity(&db, &bob),);
        first.unwrap();
        second.unwrap();
        let identities = list_identities(&db).await.unwrap();
        assert_eq!(identities.len(), 2);
        assert_eq!(
            identities
                .iter()
                .filter(|identity| identity.is_default)
                .count(),
            1
        );
    }

    #[tokio::test]
    async fn test_identity_operations_ignore_other_users() {
        let db = open_test_db().await;
        let other = format!("another-user-{}", uuid::Uuid::new_v4());
        if db.is_postgres() {
            let mut connection = db.acquire().await.unwrap();
            sqlx::query("INSERT INTO users (user_hash) VALUES ($1)")
                .bind(&other)
                .execute(&mut *connection)
                .await
                .unwrap();
        }
        let mut conn = db.acquire().await.unwrap();
        let row = sqlx::query(
            "INSERT INTO identities (user_hash, email, is_default)
             VALUES ($1, 'other@example.com', 1) RETURNING id",
        )
        .bind(&other)
        .fetch_one(&mut *conn)
        .await
        .unwrap();
        let other_id: i64 = row.try_get(0).unwrap();
        drop(conn);

        assert!(!has_identities(&db).await.unwrap());
        assert!(list_identities(&db).await.unwrap().is_empty());
        assert!(get_identity(&db, other_id).await.unwrap().is_none());
        assert!(get_default_identity(&db).await.unwrap().is_none());
        assert!(
            update_identity(
                &db,
                other_id,
                &UpdateIdentity {
                    display_name: Some("Changed".to_string()),
                    email: None,
                    signature_html: None,
                    is_default: Some(false),
                },
            )
            .await
            .unwrap()
            .is_none()
        );
        assert!(!delete_identity(&db, other_id).await.unwrap());

        create_identity(
            &db,
            &CreateIdentity {
                display_name: String::new(),
                email: "local@example.com".to_string(),
                signature_html: String::new(),
                is_default: true,
            },
        )
        .await
        .unwrap();
        let mut conn = db.acquire().await.unwrap();
        let row = sqlx::query("SELECT is_default FROM identities WHERE id = $1")
            .bind(other_id)
            .fetch_one(&mut *conn)
            .await
            .unwrap();
        assert_eq!(row.try_get::<i64, _>(0).unwrap(), 1);
    }
}
