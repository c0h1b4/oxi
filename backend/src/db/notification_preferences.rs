use rusqlite::Connection;
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

/// Retrieve the singleton notification preferences row.
/// Returns sensible defaults if the row does not yet exist.
pub fn get_preferences(conn: &Connection) -> Result<NotificationPreferences, String> {
    let result = conn.query_row(
        "SELECT enabled, sound, folders, updated_at FROM notification_preferences WHERE id = 1",
        [],
        |row| {
            let enabled_int: i32 = row.get(0)?;
            let sound_int: i32 = row.get(1)?;
            let folders_json: String = row.get(2)?;
            let updated_at: String = row.get(3)?;
            Ok((enabled_int, sound_int, folders_json, updated_at))
        },
    );

    match result {
        Ok((enabled_int, sound_int, folders_json, updated_at)) => {
            let folders: Vec<String> = serde_json::from_str(&folders_json)
                .map_err(|e| format!("Failed to parse folders JSON: {e}"))?;
            Ok(NotificationPreferences {
                enabled: enabled_int != 0,
                sound: sound_int != 0,
                folders,
                updated_at,
            })
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(NotificationPreferences {
            enabled: true,
            sound: false,
            folders: vec!["INBOX".to_string()],
            updated_at: String::new(),
        }),
        Err(e) => Err(format!("Failed to get notification preferences: {e}")),
    }
}

/// Update the singleton notification preferences row.
/// Only provided fields are changed. Returns the updated preferences.
pub fn update_preferences(
    conn: &Connection,
    data: &UpdateNotificationPreferences,
) -> Result<NotificationPreferences, String> {
    // Ensure the row exists.
    conn.execute(
        "INSERT OR IGNORE INTO notification_preferences (id) VALUES (1)",
        [],
    )
    .map_err(|e| format!("Failed to ensure preferences row: {e}"))?;

    let mut sets = Vec::new();
    let mut values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;

    if let Some(enabled) = data.enabled {
        sets.push(format!("enabled = ?{idx}"));
        values.push(Box::new(enabled as i32));
        idx += 1;
    }
    if let Some(sound) = data.sound {
        sets.push(format!("sound = ?{idx}"));
        values.push(Box::new(sound as i32));
        idx += 1;
    }
    if let Some(ref folders) = data.folders {
        let folders_json =
            serde_json::to_string(folders).map_err(|e| format!("Failed to serialize folders: {e}"))?;
        sets.push(format!("folders = ?{idx}"));
        values.push(Box::new(folders_json));
        idx += 1;
    }

    if sets.is_empty() {
        return get_preferences(conn);
    }

    sets.push("updated_at = datetime('now')".to_string());
    let set_clause = sets.join(", ");
    let sql = format!("UPDATE notification_preferences SET {set_clause} WHERE id = ?{idx}");
    values.push(Box::new(1_i32));

    let params_refs: Vec<&dyn rusqlite::types::ToSql> =
        values.iter().map(|v| v.as_ref()).collect();

    conn.execute(&sql, params_refs.as_slice())
        .map_err(|e| format!("Failed to update notification preferences: {e}"))?;

    get_preferences(conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::pool::open_test_db;

    #[test]
    fn test_get_default_preferences() {
        let conn = open_test_db();
        let prefs = get_preferences(&conn).unwrap();

        assert!(prefs.enabled);
        assert!(!prefs.sound);
        assert_eq!(prefs.folders, vec!["INBOX".to_string()]);
    }

    #[test]
    fn test_update_enabled() {
        let conn = open_test_db();

        let prefs = update_preferences(
            &conn,
            &UpdateNotificationPreferences {
                enabled: Some(false),
                sound: None,
                folders: None,
            },
        )
        .unwrap();

        assert!(!prefs.enabled);
        assert!(!prefs.sound);
        assert_eq!(prefs.folders, vec!["INBOX".to_string()]);
    }

    #[test]
    fn test_update_folders() {
        let conn = open_test_db();

        let prefs = update_preferences(
            &conn,
            &UpdateNotificationPreferences {
                enabled: None,
                sound: None,
                folders: Some(vec!["INBOX".to_string(), "Sent".to_string()]),
            },
        )
        .unwrap();

        assert!(prefs.enabled);
        assert_eq!(
            prefs.folders,
            vec!["INBOX".to_string(), "Sent".to_string()]
        );
    }

    #[test]
    fn test_update_all_fields() {
        let conn = open_test_db();

        let prefs = update_preferences(
            &conn,
            &UpdateNotificationPreferences {
                enabled: Some(false),
                sound: Some(true),
                folders: Some(vec!["Drafts".to_string()]),
            },
        )
        .unwrap();

        assert!(!prefs.enabled);
        assert!(prefs.sound);
        assert_eq!(prefs.folders, vec!["Drafts".to_string()]);
    }
}
