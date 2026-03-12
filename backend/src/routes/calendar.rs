use std::sync::Arc;

use axum::extract::{Path, Query};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};

use crate::auth::session::SessionState;
use crate::calendar::ics;
use crate::config::AppConfig;
use crate::db;
use crate::db::calendar::{CreateEvent, UpdateCalendarSettings, UpdateEvent};
use crate::error::AppError;

// ---------------------------------------------------------------------------
// Query / request types
// ---------------------------------------------------------------------------

/// Query parameters for `GET /api/calendar/events`.
#[derive(Deserialize)]
pub struct ListEventsParams {
    /// Start of the time range (ISO 8601).
    pub start: String,
    /// End of the time range (ISO 8601).
    pub end: String,
}

/// JSON body for `POST /api/calendar/meeting-templates`.
#[derive(Deserialize)]
pub struct CreateMeetingTemplateRequest {
    pub name: String,
    pub url_template: String,
    #[serde(default)]
    pub icon: String,
}

/// Response for ICS import.
#[derive(Serialize)]
pub struct ImportIcsResponse {
    pub created: usize,
    pub skipped: usize,
    pub events: Vec<db::calendar::CalendarEvent>,
}

// ---------------------------------------------------------------------------
// Event handlers
// ---------------------------------------------------------------------------

/// `GET /api/calendar/events?start=...&end=...`
///
/// List events within a time range.
pub async fn list_events(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    Query(params): Query<ListEventsParams>,
) -> Result<Response, AppError> {
    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    let events = db::calendar::list_events(&conn, &params.start, &params.end)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    Ok(Json(serde_json::json!({ "events": events })).into_response())
}

/// `GET /api/calendar/events/{id}`
///
/// Get a single event by ID.
pub async fn get_event(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    let event = db::calendar::get_event(&conn, &id)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    match event {
        Some(e) => Ok(Json(e).into_response()),
        None => Err(AppError::NotFound(format!("Event '{id}' not found"))),
    }
}

/// `POST /api/calendar/events`
///
/// Create a new calendar event.
pub async fn create_event(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    Json(body): Json<CreateEvent>,
) -> Result<Response, AppError> {
    if body.title.trim().is_empty() {
        return Err(AppError::BadRequest("Title is required".to_string()));
    }
    if body.start_time.trim().is_empty() {
        return Err(AppError::BadRequest("Start time is required".to_string()));
    }
    if body.end_time.trim().is_empty() {
        return Err(AppError::BadRequest("End time is required".to_string()));
    }

    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    let event = db::calendar::create_event(&conn, &body)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    Ok((axum::http::StatusCode::CREATED, Json(event)).into_response())
}

/// `PUT /api/calendar/events/{id}`
///
/// Update an existing calendar event.
pub async fn update_event(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    Path(id): Path<String>,
    Json(body): Json<UpdateEvent>,
) -> Result<Response, AppError> {
    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    match db::calendar::update_event(&conn, &id, &body)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?
    {
        Some(event) => Ok(Json(event).into_response()),
        None => Err(AppError::NotFound(format!("Event '{id}' not found"))),
    }
}

/// `DELETE /api/calendar/events/{id}`
///
/// Delete a calendar event.
pub async fn delete_event(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    let deleted = db::calendar::delete_event(&conn, &id)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    if deleted {
        Ok(Json(serde_json::json!({ "status": "deleted" })).into_response())
    } else {
        Err(AppError::NotFound(format!("Event '{id}' not found")))
    }
}

/// `POST /api/calendar/events/import-ics`
///
/// Import events from ICS text. Deduplicates by source_uid.
pub async fn import_ics(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    body: String,
) -> Result<Response, AppError> {
    if body.trim().is_empty() {
        return Err(AppError::BadRequest("Empty ICS body".to_string()));
    }

    let parsed = ics::parse_ics(&body)
        .map_err(|e| AppError::BadRequest(format!("Failed to parse ICS: {e}")))?;

    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    let mut created_events = Vec::new();
    let mut skipped = 0usize;

    for event_data in parsed {
        // Deduplicate by source_uid
        if let Some(ref uid) = event_data.source_uid
            && let Ok(Some(_)) = db::calendar::find_event_by_source_uid(&conn, uid)
        {
            skipped += 1;
            continue;
        }

        match db::calendar::create_event(&conn, &event_data) {
            Ok(event) => created_events.push(event),
            Err(e) => {
                tracing::warn!("Failed to create imported event: {e}");
                skipped += 1;
            }
        }
    }

    Ok(Json(ImportIcsResponse {
        created: created_events.len(),
        skipped,
        events: created_events,
    })
    .into_response())
}

// ---------------------------------------------------------------------------
// Settings handlers
// ---------------------------------------------------------------------------

/// `GET /api/calendar/settings`
///
/// Get calendar settings.
pub async fn get_calendar_settings(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
) -> Result<Response, AppError> {
    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    let settings = db::calendar::get_calendar_settings(&conn)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    Ok(Json(settings).into_response())
}

/// `PUT /api/calendar/settings`
///
/// Update calendar settings.
pub async fn update_calendar_settings(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    Json(body): Json<UpdateCalendarSettings>,
) -> Result<Response, AppError> {
    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    let settings = db::calendar::update_calendar_settings(&conn, &body)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    Ok(Json(settings).into_response())
}

// ---------------------------------------------------------------------------
// Meeting template handlers
// ---------------------------------------------------------------------------

/// `GET /api/calendar/meeting-templates`
///
/// List all meeting templates.
pub async fn list_meeting_templates(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
) -> Result<Response, AppError> {
    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    let templates = db::calendar::list_meeting_templates(&conn)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    Ok(Json(serde_json::json!({ "templates": templates })).into_response())
}

/// `POST /api/calendar/meeting-templates`
///
/// Create a new meeting template.
pub async fn create_meeting_template(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    Json(body): Json<CreateMeetingTemplateRequest>,
) -> Result<Response, AppError> {
    if body.name.trim().is_empty() {
        return Err(AppError::BadRequest("Name is required".to_string()));
    }
    if body.url_template.trim().is_empty() {
        return Err(AppError::BadRequest(
            "URL template is required".to_string(),
        ));
    }

    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    let template = db::calendar::create_meeting_template(&conn, &body.name, &body.url_template, &body.icon)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    Ok((axum::http::StatusCode::CREATED, Json(template)).into_response())
}

/// `DELETE /api/calendar/meeting-templates/{id}`
///
/// Delete a meeting template.
pub async fn delete_meeting_template(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    Path(id): Path<i64>,
) -> Result<Response, AppError> {
    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    let deleted = db::calendar::delete_meeting_template(&conn, id)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    if deleted {
        Ok(Json(serde_json::json!({ "status": "deleted" })).into_response())
    } else {
        Err(AppError::NotFound(format!(
            "Meeting template '{id}' not found"
        )))
    }
}
