use std::sync::Arc;

use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};

use crate::auth::session::SessionState;
use crate::config::AppConfig;
use crate::db;
use crate::db::notification_preferences::UpdateNotificationPreferences;
use crate::error::AppError;

/// `GET /api/settings/notifications`
pub async fn get_notification_preferences(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
) -> Result<Response, AppError> {
    let conn = config
        .user_db(&session.user_hash)
        .await
        .map_err(|e| AppError::InternalError(format!("Failed to open database: {e}")))?;

    let prefs = db::notification_preferences::get_preferences(&conn)
        .await
        .map_err(AppError::InternalError)?;

    Ok(Json(prefs).into_response())
}

/// `PUT /api/settings/notifications`
pub async fn update_notification_preferences(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    Json(data): Json<UpdateNotificationPreferences>,
) -> Result<Response, AppError> {
    let conn = config
        .user_db(&session.user_hash)
        .await
        .map_err(|e| AppError::InternalError(format!("Failed to open database: {e}")))?;

    let prefs = db::notification_preferences::update_preferences(&conn, &data)
        .await
        .map_err(AppError::InternalError)?;

    Ok(Json(prefs).into_response())
}
