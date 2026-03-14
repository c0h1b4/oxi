use std::sync::Arc;

use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};

use crate::auth::session::SessionState;
use crate::config::AppConfig;
use crate::db;
use crate::db::display_preferences::UpdateDisplayPreferences;
use crate::error::AppError;

fn map_update_error(message: String) -> AppError {
    let is_invalid_preference = message.starts_with("Invalid density:")
        || message.starts_with("Invalid theme:")
        || message.starts_with("Invalid compose_format:")
        || message.starts_with("Invalid animation_mode:");

    if is_invalid_preference {
        AppError::BadRequest(message)
    } else {
        AppError::InternalError(message)
    }
}

/// `GET /api/settings/display`
pub async fn get_display_preferences(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
) -> Result<Response, AppError> {
    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Failed to open database: {e}")))?;

    let prefs = db::display_preferences::get_preferences(&conn)
        .map_err(AppError::InternalError)?;

    Ok(Json(prefs).into_response())
}

/// `PUT /api/settings/display`
pub async fn update_display_preferences(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    Json(data): Json<UpdateDisplayPreferences>,
) -> Result<Response, AppError> {
    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Failed to open database: {e}")))?;

    let prefs = db::display_preferences::update_preferences(&conn, &data)
        .map_err(map_update_error)?;

    Ok(Json(prefs).into_response())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    use axum::http::StatusCode;
    use axum::Extension;
    use axum::response::IntoResponse;
    use serde_json::json;
    use tempfile::TempDir;

    fn test_config(data_dir: &str) -> Arc<AppConfig> {
        Arc::new(AppConfig {
            host: "127.0.0.1".to_string(),
            port: 3001,
            imap_host: None,
            imap_port: 993,
            smtp_host: None,
            smtp_port: 587,
            tls_enabled: true,
            data_dir: data_dir.to_string(),
            session_timeout_hours: 24,
            static_dir: "nonexistent_static_dir".to_string(),
            environment: "development".to_string(),
            base_path: None,
        })
    }

    fn test_session() -> SessionState {
        SessionState {
            account_id: "acc-1".to_string(),
            email: "alice@example.com".to_string(),
            password: "password".to_string(),
            user_hash: crate::auth::user_data::hash_email("alice@example.com"),
            imap_host: "imap.example.com".to_string(),
            imap_port: 993,
            imap_tls: true,
            smtp_host: "smtp.example.com".to_string(),
            smtp_port: 587,
            smtp_tls: true,
            last_accessed: Instant::now(),
            timeout_override: None,
        }
    }

    #[tokio::test]
    async fn test_invalid_preferences_map_to_bad_request() {
        let tmp = TempDir::new().unwrap();
        let config = test_config(tmp.path().to_str().unwrap());
        let session = test_session();

        let result = update_display_preferences(
            Extension(session),
            Extension(config),
            Json(UpdateDisplayPreferences {
                density: None,
                theme: None,
                language: None,
                compose_format: None,
                deep_index: None,
                animation_mode: Some(Some("ultra".to_string())),
            }),
        )
        .await;

        match result {
            Err(AppError::BadRequest(message)) => {
                assert!(message.contains("Invalid animation_mode"));
            }
            other => panic!("expected BadRequest, got {other:?}"),
        }
    }

    #[test]
    fn test_map_update_error_invalid_animation_mode_returns_bad_request() {
        let err = map_update_error("Invalid animation_mode: ultra".to_string());

        match err {
            AppError::BadRequest(message) => {
                assert!(message.contains("Invalid animation_mode"));
            }
            other => panic!("expected BadRequest, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_invalid_preferences_map_to_http_400_response() {
        let tmp = TempDir::new().unwrap();
        let config = test_config(tmp.path().to_str().unwrap());
        let session = test_session();

        let result = update_display_preferences(
            Extension(session),
            Extension(config),
            Json(UpdateDisplayPreferences {
                density: None,
                theme: None,
                language: None,
                compose_format: None,
                deep_index: None,
                animation_mode: Some(Some("ultra".to_string())),
            }),
        )
        .await;

        let response = result
            .expect_err("invalid preferences should return an error")
            .into_response();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_animation_mode_deserialize_omitted_is_none() {
        let payload = json!({
            "theme": "dark"
        });

        let parsed: UpdateDisplayPreferences =
            serde_json::from_value(payload).expect("payload should deserialize");

        assert_eq!(parsed.animation_mode, None);
    }

    #[test]
    fn test_animation_mode_deserialize_null_is_some_none() {
        let payload = json!({
            "animation_mode": null
        });

        let parsed: UpdateDisplayPreferences =
            serde_json::from_value(payload).expect("payload should deserialize");

        assert_eq!(parsed.animation_mode, Some(None));
    }

    #[test]
    fn test_animation_mode_deserialize_value_is_some_some() {
        let payload = json!({
            "animation_mode": "medium"
        });

        let parsed: UpdateDisplayPreferences =
            serde_json::from_value(payload).expect("payload should deserialize");

        assert_eq!(
            parsed.animation_mode,
            Some(Some("medium".to_string()))
        );
    }
}
