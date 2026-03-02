use std::sync::Arc;

use axum::extract::Path;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::session::SessionState;
use crate::config::AppConfig;
use crate::db;
use crate::db::contact_groups::ContactGroup;
use crate::db::contacts::Contact;
use crate::error::AppError;

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct CreateGroupBody {
    pub name: String,
}

#[derive(Deserialize)]
pub struct AddMemberBody {
    pub contact_id: String,
}

#[derive(Serialize)]
pub struct ListGroupsResponse {
    pub groups: Vec<ContactGroup>,
}

#[derive(Serialize)]
pub struct GroupMembersResponse {
    pub members: Vec<Contact>,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// `GET /api/contact-groups`
pub async fn list_groups_handler(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
) -> Result<Response, AppError> {
    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    let groups = db::contact_groups::list_groups(&conn)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    Ok(Json(ListGroupsResponse { groups }).into_response())
}

/// `POST /api/contact-groups`
pub async fn create_group_handler(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    Json(body): Json<CreateGroupBody>,
) -> Result<Response, AppError> {
    let name = body.name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("Group name is required".to_string()));
    }

    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    let id = Uuid::new_v4().to_string();
    db::contact_groups::create_group(&conn, &id, name)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    Ok(Json(serde_json::json!({ "id": id, "name": name })).into_response())
}

/// `PUT /api/contact-groups/{id}`
pub async fn update_group_handler(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    Path(id): Path<String>,
    Json(body): Json<CreateGroupBody>,
) -> Result<Response, AppError> {
    let name = body.name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("Group name is required".to_string()));
    }

    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    let updated = db::contact_groups::update_group(&conn, &id, name)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    if updated {
        Ok(Json(serde_json::json!({ "id": id, "name": name })).into_response())
    } else {
        Err(AppError::NotFound(format!("Group '{id}' not found")))
    }
}

/// `DELETE /api/contact-groups/{id}`
pub async fn delete_group_handler(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    let deleted = db::contact_groups::delete_group(&conn, &id)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    if deleted {
        Ok(Json(serde_json::json!({ "status": "deleted" })).into_response())
    } else {
        Err(AppError::NotFound(format!("Group '{id}' not found")))
    }
}

/// `GET /api/contact-groups/{id}/members`
pub async fn list_members_handler(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    let members = db::contact_groups::list_group_members(&conn, &id)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    Ok(Json(GroupMembersResponse { members }).into_response())
}

/// `POST /api/contact-groups/{id}/members`
pub async fn add_member_handler(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    Path(id): Path<String>,
    Json(body): Json<AddMemberBody>,
) -> Result<Response, AppError> {
    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    db::contact_groups::add_member(&conn, &id, &body.contact_id)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    Ok(Json(serde_json::json!({ "status": "ok" })).into_response())
}

/// `DELETE /api/contact-groups/{id}/members/{contact_id}`
pub async fn remove_member_handler(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    Path((id, contact_id)): Path<(String, String)>,
) -> Result<Response, AppError> {
    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    let removed = db::contact_groups::remove_member(&conn, &id, &contact_id)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    if removed {
        Ok(Json(serde_json::json!({ "status": "ok" })).into_response())
    } else {
        Err(AppError::NotFound("Member not found in group".to_string()))
    }
}
