use sqlx::Row;

use crate::db::pool::{UserDb, now};
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

/// Retrieve the display preferences for this user, falling back to defaults.
pub async fn get_preferences(db: &UserDb) -> Result<DisplayPreferences, String> {
    let mut conn = db.acquire().await?;
    let row = sqlx::query(
        "SELECT density, theme, language, compose_format, CAST(deep_index AS BIGINT), animation_mode, updated_at
         FROM display_preferences WHERE user_hash = $1",
    )
    .bind(db.user_hash())
    .fetch_optional(&mut *conn)
    .await
    .map_err(|e| format!("Failed to get display preferences: {e}"))?;

    match row {
        Some(row) => Ok(DisplayPreferences {
            density: row.try_get(0).map_err(|e| e.to_string())?,
            theme: row.try_get(1).map_err(|e| e.to_string())?,
            language: row.try_get(2).map_err(|e| e.to_string())?,
            compose_format: row.try_get(3).map_err(|e| e.to_string())?,
            deep_index: row.try_get::<i64, _>(4).map_err(|e| e.to_string())? != 0,
            animation_mode: row.try_get(5).map_err(|e| e.to_string())?,
            updated_at: row.try_get(6).map_err(|e| e.to_string())?,
        }),
        None => Ok(DisplayPreferences {
            density: "comfortable".to_string(),
            theme: "system".to_string(),
            language: "en".to_string(),
            compose_format: "html".to_string(),
            deep_index: false,
            animation_mode: None,
            updated_at: String::new(),
        }),
    }
}

/// Change only the supplied fields of this user's preferences.
pub async fn update_preferences(
    db: &UserDb,
    data: &UpdateDisplayPreferences,
) -> Result<DisplayPreferences, String> {
    if let Some(ref density) = data.density
        && density != "compact"
        && density != "comfortable"
    {
        return Err(format!("Invalid density: {density}"));
    }
    if let Some(ref theme) = data.theme
        && theme != "light"
        && theme != "dark"
        && theme != "system"
    {
        return Err(format!("Invalid theme: {theme}"));
    }
    if let Some(ref compose_format) = data.compose_format
        && compose_format != "html"
        && compose_format != "text"
    {
        return Err(format!("Invalid compose_format: {compose_format}"));
    }
    if let Some(Some(ref value)) = data.animation_mode
        && value != "rich"
        && value != "medium"
        && value != "subtle"
        && value != "off"
    {
        return Err(format!("Invalid animation_mode: {value}"));
    }

    let mut conn = db.acquire().await?;
    sqlx::query(
        "INSERT INTO display_preferences (user_hash, id, updated_at) VALUES ($1, 1, $2)
         ON CONFLICT (user_hash) DO NOTHING",
    )
    .bind(db.user_hash())
    .bind(now())
    .execute(&mut *conn)
    .await
    .map_err(|e| format!("Failed to ensure preferences row: {e}"))?;

    if data.density.is_some()
        || data.theme.is_some()
        || data.language.is_some()
        || data.compose_format.is_some()
        || data.deep_index.is_some()
        || data.animation_mode.is_some()
    {
        sqlx::query(
            "UPDATE display_preferences SET
                 density = COALESCE($1, density),
                 theme = COALESCE($2, theme),
                 language = COALESCE($3, language),
                 compose_format = COALESCE($4, compose_format),
                 deep_index = COALESCE($5, deep_index),
                 animation_mode = CASE WHEN $6 = 1 THEN $7 ELSE animation_mode END,
                 updated_at = $8
             WHERE user_hash = $9",
        )
        .bind(data.density.as_deref())
        .bind(data.theme.as_deref())
        .bind(data.language.as_deref())
        .bind(data.compose_format.as_deref())
        .bind(data.deep_index.map(i64::from))
        .bind(i64::from(data.animation_mode.is_some()))
        .bind(
            data.animation_mode
                .as_ref()
                .and_then(|value| value.as_deref()),
        )
        .bind(now())
        .bind(db.user_hash())
        .execute(&mut *conn)
        .await
        .map_err(|e| format!("Failed to update display preferences: {e}"))?;
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

        assert_eq!(prefs.density, "comfortable");
        assert_eq!(prefs.theme, "system");
        assert_eq!(prefs.language, "en");
        assert_eq!(prefs.compose_format, "html");
    }

    #[tokio::test]
    async fn test_update_density() {
        let conn = open_test_db().await;

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
        .await
        .unwrap();

        assert_eq!(prefs.density, "compact");
        assert_eq!(prefs.theme, "system");
        assert_eq!(prefs.language, "en");
    }

    #[tokio::test]
    async fn test_update_theme() {
        let conn = open_test_db().await;

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
        .await
        .unwrap();

        assert_eq!(prefs.density, "comfortable");
        assert_eq!(prefs.theme, "dark");
    }

    #[tokio::test]
    async fn test_update_all_fields() {
        let conn = open_test_db().await;

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
        .await
        .unwrap();

        assert_eq!(prefs.density, "compact");
        assert_eq!(prefs.theme, "light");
        assert_eq!(prefs.language, "en");
    }

    #[tokio::test]
    async fn test_invalid_density_rejected() {
        let conn = open_test_db().await;

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
        )
        .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid density"));
    }

    #[tokio::test]
    async fn test_invalid_theme_rejected() {
        let conn = open_test_db().await;

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
        )
        .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid theme"));
    }

    #[tokio::test]
    async fn test_update_compose_format() {
        let conn = open_test_db().await;

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
        .await
        .unwrap();

        assert_eq!(prefs.compose_format, "text");
        assert_eq!(prefs.density, "comfortable");
    }

    #[tokio::test]
    async fn test_invalid_compose_format_rejected() {
        let conn = open_test_db().await;

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
        )
        .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid compose_format"));
    }

    #[tokio::test]
    async fn test_empty_update_returns_defaults() {
        let conn = open_test_db().await;

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
        .await
        .unwrap();

        assert_eq!(prefs.density, "comfortable");
        assert_eq!(prefs.theme, "system");
    }

    #[tokio::test]
    async fn test_read_defaults_animation_mode_none() {
        let conn = open_test_db().await;
        let prefs = get_preferences(&conn).await.unwrap();

        assert_eq!(prefs.animation_mode, None);
    }

    #[tokio::test]
    async fn test_update_animation_mode_valid() {
        let conn = open_test_db().await;

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
        .await
        .unwrap();

        assert_eq!(prefs.animation_mode.as_deref(), Some("medium"));
    }

    #[tokio::test]
    async fn test_update_animation_mode_invalid() {
        let conn = open_test_db().await;

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
        )
        .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid animation_mode"));
    }

    #[tokio::test]
    async fn test_update_animation_mode_null_clears_value() {
        let conn = open_test_db().await;

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
        .await
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
        .await
        .unwrap();

        assert_eq!(cleared_prefs.animation_mode, None);
    }

    #[tokio::test]
    async fn test_update_animation_mode_omitted_keeps_existing_value() {
        let conn = open_test_db().await;

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
        .await
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
        .await
        .unwrap();

        assert_eq!(updated_prefs.density, "compact");
        assert_eq!(updated_prefs.animation_mode.as_deref(), Some("subtle"));
    }
}
