use sqlx::Row;

use crate::db::pool::{UserDb, now};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct NotificationPreferences {
    pub enabled: bool,
    pub sound: bool,
    pub folders: Vec<String>,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateNotificationPreferences {
    pub enabled: Option<bool>,
    pub sound: Option<bool>,
    pub folders: Option<Vec<String>>,
}

/// Retrieve this user's notification preferences, falling back to defaults.
pub async fn get_preferences(db: &UserDb) -> Result<NotificationPreferences, String> {
    let mut conn = db.acquire().await?;
    let row = sqlx::query(
        "SELECT enabled, sound, folders, updated_at FROM notification_preferences WHERE user_hash = $1",
    )
    .bind(db.user_hash())
    .fetch_optional(&mut *conn)
    .await
    .map_err(|e| format!("Failed to get notification preferences: {e}"))?;

    match row {
        Some(row) => {
            let folders_json: String = row.try_get(2).map_err(|e| e.to_string())?;
            let folders = serde_json::from_str(&folders_json)
                .map_err(|e| format!("Failed to parse folders JSON: {e}"))?;
            Ok(NotificationPreferences {
                enabled: row.try_get::<i64, _>(0).map_err(|e| e.to_string())? != 0,
                sound: row.try_get::<i64, _>(1).map_err(|e| e.to_string())? != 0,
                folders,
                updated_at: row.try_get(3).map_err(|e| e.to_string())?,
            })
        }
        None => Ok(NotificationPreferences {
            enabled: true,
            sound: false,
            folders: vec!["INBOX".to_string()],
            updated_at: String::new(),
        }),
    }
}

/// Change only the supplied fields of this user's preferences.
pub async fn update_preferences(
    db: &UserDb,
    data: &UpdateNotificationPreferences,
) -> Result<NotificationPreferences, String> {
    let folders_json = data
        .folders
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|e| format!("Failed to serialize folders: {e}"))?;
    let mut conn = db.acquire().await?;
    sqlx::query(
        "INSERT INTO notification_preferences (user_hash, id, updated_at) VALUES ($1, 1, $2)
         ON CONFLICT (user_hash) DO NOTHING",
    )
    .bind(db.user_hash())
    .bind(now())
    .execute(&mut *conn)
    .await
    .map_err(|e| format!("Failed to ensure preferences row: {e}"))?;

    if data.enabled.is_some() || data.sound.is_some() || data.folders.is_some() {
        sqlx::query(
            "UPDATE notification_preferences SET
                 enabled = COALESCE($1, enabled),
                 sound = COALESCE($2, sound),
                 folders = COALESCE($3, folders),
                 updated_at = $4
             WHERE user_hash = $5",
        )
        .bind(data.enabled.map(i64::from))
        .bind(data.sound.map(i64::from))
        .bind(folders_json)
        .bind(now())
        .bind(db.user_hash())
        .execute(&mut *conn)
        .await
        .map_err(|e| format!("Failed to update notification preferences: {e}"))?;
    }
    drop(conn);
    get_preferences(db).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::pool::open_test_db;

    #[tokio::test]
    async fn test_get_default_preferences() {
        let conn = open_test_db().await;
        let prefs = get_preferences(&conn).await.unwrap();

        assert!(prefs.enabled);
        assert!(!prefs.sound);
        assert_eq!(prefs.folders, vec!["INBOX".to_string()]);
    }

    #[tokio::test]
    async fn test_update_enabled() {
        let conn = open_test_db().await;

        let prefs = update_preferences(
            &conn,
            &UpdateNotificationPreferences {
                enabled: Some(false),
                sound: None,
                folders: None,
            },
        )
        .await
        .unwrap();

        assert!(!prefs.enabled);
        assert!(!prefs.sound);
        assert_eq!(prefs.folders, vec!["INBOX".to_string()]);
    }

    #[tokio::test]
    async fn test_update_folders() {
        let conn = open_test_db().await;

        let prefs = update_preferences(
            &conn,
            &UpdateNotificationPreferences {
                enabled: None,
                sound: None,
                folders: Some(vec!["INBOX".to_string(), "Sent".to_string()]),
            },
        )
        .await
        .unwrap();

        assert!(prefs.enabled);
        assert_eq!(prefs.folders, vec!["INBOX".to_string(), "Sent".to_string()]);
    }

    #[tokio::test]
    async fn test_update_all_fields() {
        let conn = open_test_db().await;

        let prefs = update_preferences(
            &conn,
            &UpdateNotificationPreferences {
                enabled: Some(false),
                sound: Some(true),
                folders: Some(vec!["Drafts".to_string()]),
            },
        )
        .await
        .unwrap();

        assert!(!prefs.enabled);
        assert!(prefs.sound);
        assert_eq!(prefs.folders, vec!["Drafts".to_string()]);
    }
}
