use rusqlite::Connection;
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct DisplayPreferences {
    pub density: String,
    pub theme: String,
    pub language: String,
    pub compose_format: String,
    pub deep_index: bool,
    pub animation_mode: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateDisplayPreferences {
    pub density: Option<String>,
    pub theme: Option<String>,
    pub language: Option<String>,
    pub compose_format: Option<String>,
    pub deep_index: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_animation_mode_field")]
    pub animation_mode: Option<Option<String>>,
}

fn deserialize_animation_mode_field<'de, D>(
    deserializer: D,
) -> Result<Option<Option<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<String>::deserialize(deserializer)?;
    Ok(Some(value))
}

/// Retrieve the singleton display preferences row.
/// Returns sensible defaults if the row does not yet exist.
pub fn get_preferences(conn: &Connection) -> Result<DisplayPreferences, String> {
    let result = conn.query_row(
        "SELECT density, theme, language, compose_format, deep_index, animation_mode, updated_at FROM display_preferences WHERE id = 1",
        [],
        |row| {
            let deep_index_int: i32 = row.get(4)?;
            Ok(DisplayPreferences {
                density: row.get(0)?,
                theme: row.get(1)?,
                language: row.get(2)?,
                compose_format: row.get(3)?,
                deep_index: deep_index_int != 0,
                animation_mode: row.get(5)?,
                updated_at: row.get(6)?,
            })
        },
    );

    match result {
        Ok(prefs) => Ok(prefs),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(DisplayPreferences {
            density: "comfortable".to_string(),
            theme: "system".to_string(),
            language: "en".to_string(),
            compose_format: "html".to_string(),
            deep_index: false,
            animation_mode: None,
            updated_at: String::new(),
        }),
        Err(e) => Err(format!("Failed to get display preferences: {e}")),
    }
}

/// Update the singleton display preferences row.
/// Only provided fields are changed. Returns the updated preferences.
pub fn update_preferences(
    conn: &Connection,
    data: &UpdateDisplayPreferences,
) -> Result<DisplayPreferences, String> {
    // Ensure the row exists.
    conn.execute(
        "INSERT OR IGNORE INTO display_preferences (id) VALUES (1)",
        [],
    )
    .map_err(|e| format!("Failed to ensure preferences row: {e}"))?;

    let mut sets = Vec::new();
    let mut values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;

    if let Some(ref density) = data.density {
        if density != "compact" && density != "comfortable" {
            return Err(format!("Invalid density: {density}"));
        }
        sets.push(format!("density = ?{idx}"));
        values.push(Box::new(density.clone()));
        idx += 1;
    }
    if let Some(ref theme) = data.theme {
        if theme != "light" && theme != "dark" && theme != "system" {
            return Err(format!("Invalid theme: {theme}"));
        }
        sets.push(format!("theme = ?{idx}"));
        values.push(Box::new(theme.clone()));
        idx += 1;
    }
    if let Some(ref language) = data.language {
        sets.push(format!("language = ?{idx}"));
        values.push(Box::new(language.clone()));
        idx += 1;
    }
    if let Some(ref compose_format) = data.compose_format {
        if compose_format != "html" && compose_format != "text" {
            return Err(format!("Invalid compose_format: {compose_format}"));
        }
        sets.push(format!("compose_format = ?{idx}"));
        values.push(Box::new(compose_format.clone()));
        idx += 1;
    }
    if let Some(deep_index) = data.deep_index {
        sets.push(format!("deep_index = ?{idx}"));
        values.push(Box::new(deep_index as i32));
        idx += 1;
    }
    if let Some(ref animation_mode) = data.animation_mode {
        if let Some(value) = animation_mode
            && value != "rich"
            && value != "medium"
            && value != "subtle"
            && value != "off"
        {
            return Err(format!("Invalid animation_mode: {value}"));
        }
        sets.push(format!("animation_mode = ?{idx}"));
        values.push(Box::new(animation_mode.clone()));
        idx += 1;
    }

    if sets.is_empty() {
        return get_preferences(conn);
    }

    sets.push("updated_at = datetime('now')".to_string());
    let set_clause = sets.join(", ");
    let sql = format!("UPDATE display_preferences SET {set_clause} WHERE id = ?{idx}");
    values.push(Box::new(1_i32));

    let params_refs: Vec<&dyn rusqlite::types::ToSql> = values.iter().map(|v| v.as_ref()).collect();

    conn.execute(&sql, params_refs.as_slice())
        .map_err(|e| format!("Failed to update display preferences: {e}"))?;

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

        assert_eq!(prefs.density, "comfortable");
        assert_eq!(prefs.theme, "system");
        assert_eq!(prefs.language, "en");
        assert_eq!(prefs.compose_format, "html");
    }

    #[test]
    fn test_update_density() {
        let conn = open_test_db();

        let prefs = update_preferences(
            &conn,
            &UpdateDisplayPreferences {
                density: Some("compact".to_string()),
                theme: None,
                language: None,
                compose_format: None,
                deep_index: None,
                animation_mode: None,
            },
        )
        .unwrap();

        assert_eq!(prefs.density, "compact");
        assert_eq!(prefs.theme, "system");
        assert_eq!(prefs.language, "en");
    }

    #[test]
    fn test_update_theme() {
        let conn = open_test_db();

        let prefs = update_preferences(
            &conn,
            &UpdateDisplayPreferences {
                density: None,
                theme: Some("dark".to_string()),
                language: None,
                compose_format: None,
                deep_index: None,
                animation_mode: None,
            },
        )
        .unwrap();

        assert_eq!(prefs.density, "comfortable");
        assert_eq!(prefs.theme, "dark");
    }

    #[test]
    fn test_update_all_fields() {
        let conn = open_test_db();

        let prefs = update_preferences(
            &conn,
            &UpdateDisplayPreferences {
                density: Some("compact".to_string()),
                theme: Some("light".to_string()),
                language: Some("en".to_string()),
                compose_format: None,
                deep_index: None,
                animation_mode: None,
            },
        )
        .unwrap();

        assert_eq!(prefs.density, "compact");
        assert_eq!(prefs.theme, "light");
        assert_eq!(prefs.language, "en");
    }

    #[test]
    fn test_invalid_density_rejected() {
        let conn = open_test_db();

        let result = update_preferences(
            &conn,
            &UpdateDisplayPreferences {
                density: Some("invalid".to_string()),
                theme: None,
                language: None,
                compose_format: None,
                deep_index: None,
                animation_mode: None,
            },
        );

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid density"));
    }

    #[test]
    fn test_invalid_theme_rejected() {
        let conn = open_test_db();

        let result = update_preferences(
            &conn,
            &UpdateDisplayPreferences {
                density: None,
                theme: Some("rainbow".to_string()),
                language: None,
                compose_format: None,
                deep_index: None,
                animation_mode: None,
            },
        );

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid theme"));
    }

    #[test]
    fn test_update_compose_format() {
        let conn = open_test_db();

        let prefs = update_preferences(
            &conn,
            &UpdateDisplayPreferences {
                density: None,
                theme: None,
                language: None,
                compose_format: Some("text".to_string()),
                deep_index: None,
                animation_mode: None,
            },
        )
        .unwrap();

        assert_eq!(prefs.compose_format, "text");
        assert_eq!(prefs.density, "comfortable");
    }

    #[test]
    fn test_invalid_compose_format_rejected() {
        let conn = open_test_db();

        let result = update_preferences(
            &conn,
            &UpdateDisplayPreferences {
                density: None,
                theme: None,
                language: None,
                compose_format: Some("markdown".to_string()),
                deep_index: None,
                animation_mode: None,
            },
        );

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid compose_format"));
    }

    #[test]
    fn test_empty_update_returns_defaults() {
        let conn = open_test_db();

        let prefs = update_preferences(
            &conn,
            &UpdateDisplayPreferences {
                density: None,
                theme: None,
                language: None,
                compose_format: None,
                deep_index: None,
                animation_mode: None,
            },
        )
        .unwrap();

        assert_eq!(prefs.density, "comfortable");
        assert_eq!(prefs.theme, "system");
    }

    #[test]
    fn test_read_defaults_animation_mode_none() {
        let conn = open_test_db();
        let prefs = get_preferences(&conn).unwrap();

        assert_eq!(prefs.animation_mode, None);
    }

    #[test]
    fn test_update_animation_mode_valid() {
        let conn = open_test_db();

        let prefs = update_preferences(
            &conn,
            &UpdateDisplayPreferences {
                density: None,
                theme: None,
                language: None,
                compose_format: None,
                deep_index: None,
                animation_mode: Some(Some("medium".to_string())),
            },
        )
        .unwrap();

        assert_eq!(prefs.animation_mode.as_deref(), Some("medium"));
    }

    #[test]
    fn test_update_animation_mode_invalid() {
        let conn = open_test_db();

        let result = update_preferences(
            &conn,
            &UpdateDisplayPreferences {
                density: None,
                theme: None,
                language: None,
                compose_format: None,
                deep_index: None,
                animation_mode: Some(Some("ultra".to_string())),
            },
        );

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid animation_mode"));
    }

    #[test]
    fn test_update_animation_mode_null_clears_value() {
        let conn = open_test_db();

        let set_prefs = update_preferences(
            &conn,
            &UpdateDisplayPreferences {
                density: None,
                theme: None,
                language: None,
                compose_format: None,
                deep_index: None,
                animation_mode: Some(Some("medium".to_string())),
            },
        )
        .unwrap();
        assert_eq!(set_prefs.animation_mode.as_deref(), Some("medium"));

        let cleared_prefs = update_preferences(
            &conn,
            &UpdateDisplayPreferences {
                density: None,
                theme: None,
                language: None,
                compose_format: None,
                deep_index: None,
                animation_mode: Some(None),
            },
        )
        .unwrap();

        assert_eq!(cleared_prefs.animation_mode, None);
    }

    #[test]
    fn test_update_animation_mode_omitted_keeps_existing_value() {
        let conn = open_test_db();

        let set_prefs = update_preferences(
            &conn,
            &UpdateDisplayPreferences {
                density: None,
                theme: None,
                language: None,
                compose_format: None,
                deep_index: None,
                animation_mode: Some(Some("subtle".to_string())),
            },
        )
        .unwrap();
        assert_eq!(set_prefs.animation_mode.as_deref(), Some("subtle"));

        let updated_prefs = update_preferences(
            &conn,
            &UpdateDisplayPreferences {
                density: Some("compact".to_string()),
                theme: None,
                language: None,
                compose_format: None,
                deep_index: None,
                animation_mode: None,
            },
        )
        .unwrap();

        assert_eq!(updated_prefs.density, "compact");
        assert_eq!(updated_prefs.animation_mode.as_deref(), Some("subtle"));
    }
}
