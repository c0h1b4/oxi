use std::sync::Arc;

use axum::extract::Path;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};

use crate::auth::session::SessionState;
use crate::config::AppConfig;
use crate::db;
use crate::db::identities::{CreateIdentity, UpdateIdentity};
use crate::error::AppError;

/// `GET /api/identities` — list all sender identities.
pub async fn list_identities_handler(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
) -> Result<Response, AppError> {
    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Failed to open database: {e}")))?;

    let identities = db::identities::list_identities(&conn)
        .map_err(AppError::InternalError)?;

    Ok(Json(serde_json::json!({ "identities": identities })).into_response())
}

/// `GET /api/identities/:id` — get a single identity.
pub async fn get_identity_handler(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    Path(id): Path<i64>,
) -> Result<Response, AppError> {
    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Failed to open database: {e}")))?;

    let identity = db::identities::get_identity(&conn, id)
        .map_err(AppError::InternalError)?;

    match identity {
        Some(i) => Ok(Json(i).into_response()),
        None => Err(AppError::NotFound("Identity not found".to_string())),
    }
}

/// `POST /api/identities` — create a new identity.
pub async fn create_identity_handler(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    Json(data): Json<CreateIdentity>,
) -> Result<Response, AppError> {
    if data.email.trim().is_empty() {
        return Err(AppError::BadRequest("Email is required".to_string()));
    }

    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Failed to open database: {e}")))?;

    let identity = db::identities::create_identity(&conn, &data)
        .map_err(AppError::InternalError)?;

    Ok((axum::http::StatusCode::CREATED, Json(identity)).into_response())
}

/// `PUT /api/identities/:id` — update an identity.
pub async fn update_identity_handler(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    Path(id): Path<i64>,
    Json(data): Json<UpdateIdentity>,
) -> Result<Response, AppError> {
    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Failed to open database: {e}")))?;

    match db::identities::update_identity(&conn, id, &data)
        .map_err(AppError::InternalError)?
    {
        Some(identity) => Ok(Json(identity).into_response()),
        None => Err(AppError::NotFound("Identity not found".to_string())),
    }
}

/// `DELETE /api/identities/:id` — delete an identity.
pub async fn delete_identity_handler(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    Path(id): Path<i64>,
) -> Result<Response, AppError> {
    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Failed to open database: {e}")))?;

    if db::identities::delete_identity(&conn, id)
        .map_err(AppError::InternalError)?
    {
        Ok(Json(serde_json::json!({ "status": "deleted" })).into_response())
    } else {
        Err(AppError::NotFound("Identity not found".to_string()))
    }
}
