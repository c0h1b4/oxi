use std::sync::Arc;

use axum::extract::{Path, Query};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};

use crate::auth::session::SessionState;
use crate::calendar::{ics, invitations};
use crate::config::AppConfig;
use crate::db;
use crate::db::calendar::{CreateEvent, UpdateCalendarSettings, UpdateEvent};
use crate::error::AppError;
use crate::smtp::client::{SmtpClient, SmtpCredentials};

#[cfg(test)]
#[path = "calendar_tests.rs"]
mod tests;

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

#[derive(Deserialize)]
pub struct UpdateEventRequest {
    #[serde(flatten)]
    pub event: UpdateEvent,
    #[serde(default)]
    pub send_invitations: bool,
}

#[derive(Serialize)]
pub struct EventSaveResponse {
    #[serde(flatten)]
    pub event: db::calendar::CalendarEvent,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invitation_error: Option<String>,
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
/// Create a new calendar event and invite its attendees.
pub async fn create_event(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    Extension(smtp_client): Extension<Arc<dyn SmtpClient>>,
    Json(mut body): Json<CreateEvent>,
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
    invitations::recipients(&body.attendees, &session.email).map_err(AppError::BadRequest)?;
    if body.source == "manual" {
        if !body.organizer.trim().is_empty() && !body.organizer.eq_ignore_ascii_case(&session.email)
        {
            return Err(AppError::BadRequest(
                "Organizer must match the active account".into(),
            ));
        }
        body.organizer = session.email.clone();
    }
    let database = db::pool::UserDb::new(&config.data_dir, &session.user_hash);
    let event = database
        .run(move |conn| db::calendar::create_event(conn, &body))
        .await
        .map_err(AppError::InternalError)?;
    let invitation_error = if event.source == "manual" {
        deliver_invitations(&database, &event, &session, &config, smtp_client.as_ref())
            .await
            .err()
    } else {
        None
    };
    Ok((
        axum::http::StatusCode::CREATED,
        Json(EventSaveResponse {
            event,
            invitation_error,
        }),
    )
        .into_response())
}

/// `PUT /api/calendar/events/{id}`
///
/// Update an event. Invitations are opt-in for edits.
pub async fn update_event(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    Extension(smtp_client): Extension<Arc<dyn SmtpClient>>,
    Path(id): Path<String>,
    Json(body): Json<UpdateEventRequest>,
) -> Result<Response, AppError> {
    if let Some(attendees) = &body.event.attendees {
        invitations::recipients(attendees, &session.email).map_err(AppError::BadRequest)?;
    }
    let database = db::pool::UserDb::new(&config.data_dir, &session.user_hash);
    let lookup_id = id.clone();
    let existing = database
        .run(move |conn| db::calendar::get_event(conn, &lookup_id))
        .await
        .map_err(AppError::InternalError)?
        .ok_or_else(|| AppError::NotFound(format!("Event '{id}' not found")))?;
    if body.send_invitations {
        invitations::ensure_organizer(&existing, &session.email).map_err(AppError::BadRequest)?;
        if body.event.organizer.as_ref().is_some_and(|email| {
            !email.trim().is_empty() && !email.eq_ignore_ascii_case(&session.email)
        }) {
            return Err(AppError::BadRequest(
                "Organizer must match the active account".into(),
            ));
        }
        if body.event.attendees.is_none() {
            invitations::recipients(&existing.attendees, &session.email)
                .map_err(AppError::BadRequest)?;
        }
    }
    let event = database
        .run(move |conn| db::calendar::update_event(conn, &id, &body.event))
        .await
        .map_err(AppError::InternalError)?
        .ok_or_else(|| AppError::NotFound("Event not found".into()))?;
    let invitation_error = if body.send_invitations {
        deliver_invitations(&database, &event, &session, &config, smtp_client.as_ref())
            .await
            .err()
    } else {
        None
    };
    Ok(Json(EventSaveResponse {
        event,
        invitation_error,
    })
    .into_response())
}

/// `POST /api/calendar/events/{id}/invitations`
pub async fn send_invitations(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    Extension(smtp_client): Extension<Arc<dyn SmtpClient>>,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let database = db::pool::UserDb::new(&config.data_dir, &session.user_hash);
    let event = database
        .run(move |conn| db::calendar::get_event(conn, &id))
        .await
        .map_err(AppError::InternalError)?
        .ok_or_else(|| AppError::NotFound("Event not found".into()))?;
    invitations::ensure_organizer(&event, &session.email).map_err(AppError::BadRequest)?;
    let recipients =
        invitations::recipients(&event.attendees, &session.email).map_err(AppError::BadRequest)?;
    if recipients.is_empty() {
        return Err(AppError::BadRequest(
            "At least one attendee is required".into(),
        ));
    }
    let sent = deliver_invitations(&database, &event, &session, &config, smtp_client.as_ref())
        .await
        .map_err(AppError::ServiceUnavailable)?;
    Ok(Json(serde_json::json!({ "sent": sent })).into_response())
}

async fn deliver_invitations(
    database: &db::pool::UserDb,
    event: &db::calendar::CalendarEvent,
    session: &SessionState,
    config: &AppConfig,
    smtp_client: &dyn SmtpClient,
) -> Result<usize, String> {
    if invitations::recipients(&event.attendees, &session.email)?.is_empty() {
        return Ok(0);
    }
    let event = event.clone();
    let email = session.email.clone();
    let message = database
        .run(move |conn| invitations::build_invitation(conn, &event, &email))
        .await?;
    if message.to.is_empty() {
        return Ok(0);
    }
    let host = if !session.smtp_host.is_empty() {
        session.smtp_host.clone()
    } else {
        config
            .smtp_host
            .clone()
            .ok_or("SMTP server not configured")?
    };
    let credentials = SmtpCredentials {
        host,
        port: if session.smtp_host.is_empty() {
            config.smtp_port
        } else {
            session.smtp_port
        },
        tls: if session.smtp_host.is_empty() {
            config.tls_enabled
        } else {
            session.smtp_tls
        },
        email: session.email.clone(),
        password: session.password.clone(),
    };
    smtp_client
        .send_message(&credentials, &message)
        .await
        .map_err(|error| format!("Failed to send invitations: {error}"))?;
    Ok(message.to.len())
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
        return Err(AppError::BadRequest("URL template is required".to_string()));
    }

    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    let template =
        db::calendar::create_meeting_template(&conn, &body.name, &body.url_template, &body.icon)
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
