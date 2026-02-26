use std::sync::Arc;

use axum::extract::{Path, Query};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::session::SessionState;
use crate::config::AppConfig;
use crate::db;
use crate::db::contacts::Contact;
use crate::error::AppError;

// ---------------------------------------------------------------------------
// Query / request types
// ---------------------------------------------------------------------------

/// Query parameters for `GET /api/contacts`.
#[derive(Deserialize)]
pub struct ListContactsParams {
    /// Optional search query to filter by name or email.
    pub q: Option<String>,
    /// Maximum number of results (default 50).
    #[serde(default = "default_limit")]
    pub limit: u32,
    /// Offset for pagination (default 0).
    #[serde(default)]
    pub offset: u32,
}

/// Query parameters for `GET /api/contacts/autocomplete`.
#[derive(Deserialize)]
pub struct AutocompleteParams {
    /// Search query (required for autocomplete).
    pub q: Option<String>,
    /// Maximum number of suggestions (default 10).
    #[serde(default = "default_autocomplete_limit")]
    pub limit: u32,
}

/// JSON body for `POST /api/contacts`.
#[derive(Deserialize)]
pub struct CreateContactBody {
    pub id: Option<String>,
    pub email: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub company: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub is_favorite: bool,
    pub last_contacted: Option<String>,
    #[serde(default)]
    pub contact_count: i64,
    #[serde(default = "default_source")]
    pub source: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

fn default_limit() -> u32 {
    50
}

fn default_autocomplete_limit() -> u32 {
    10
}

fn default_source() -> String {
    "manual".to_string()
}

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

/// Response envelope for `GET /api/contacts`.
#[derive(Serialize)]
pub struct ListContactsResponse {
    pub contacts: Vec<Contact>,
    pub total_count: usize,
}

/// A single autocomplete suggestion.
#[derive(Serialize)]
pub struct AutocompleteSuggestion {
    pub email: String,
    pub name: String,
}

/// Response envelope for `GET /api/contacts/autocomplete`.
#[derive(Serialize)]
pub struct AutocompleteResponse {
    pub suggestions: Vec<AutocompleteSuggestion>,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// `GET /api/contacts?q=&limit=50&offset=0`
///
/// Lists contacts with optional search. If `q` is provided, uses
/// `search_contacts`; otherwise uses `list_contacts`.
pub async fn list_contacts_handler(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    Query(params): Query<ListContactsParams>,
) -> Result<Response, AppError> {
    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    let query = params.q.as_deref().map(|s| s.trim()).filter(|s| !s.is_empty());

    let contacts = match query {
        Some(q) => db::contacts::search_contacts(&conn, q, params.limit)
            .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?,
        None => db::contacts::list_contacts(&conn, None, params.limit, params.offset)
            .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?,
    };

    let total_count = contacts.len();

    Ok(Json(ListContactsResponse {
        contacts,
        total_count,
    })
    .into_response())
}

/// `POST /api/contacts`
///
/// Creates or updates a contact. Generates a UUID for `id` if not provided.
pub async fn create_contact_handler(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    Json(body): Json<CreateContactBody>,
) -> Result<Response, AppError> {
    if body.email.trim().is_empty() {
        return Err(AppError::BadRequest("Email is required".to_string()));
    }

    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    let now: String = conn
        .query_row("SELECT datetime('now')", [], |row| row.get(0))
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;
    let id = body.id.unwrap_or_else(|| Uuid::new_v4().to_string());

    let contact = Contact {
        id,
        email: body.email,
        name: body.name,
        company: body.company,
        notes: body.notes,
        is_favorite: body.is_favorite,
        last_contacted: body.last_contacted,
        contact_count: body.contact_count,
        source: body.source,
        created_at: if body.created_at.is_empty() {
            now.clone()
        } else {
            body.created_at
        },
        updated_at: if body.updated_at.is_empty() {
            now
        } else {
            body.updated_at
        },
    };

    db::contacts::upsert_contact(&conn, &contact)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    Ok(Json(contact).into_response())
}

/// `GET /api/contacts/autocomplete?q=al&limit=10`
///
/// Fast autocomplete endpoint. Returns matching contacts as lightweight
/// suggestions with only email and name.
pub async fn autocomplete_handler(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    Query(params): Query<AutocompleteParams>,
) -> Result<Response, AppError> {
    let query = params
        .q
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());

    let query = match query {
        Some(q) => q,
        None => {
            return Ok(Json(AutocompleteResponse {
                suggestions: vec![],
            })
            .into_response());
        }
    };

    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    let contacts = db::contacts::search_contacts(&conn, query, params.limit)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    let suggestions: Vec<AutocompleteSuggestion> = contacts
        .into_iter()
        .map(|c| AutocompleteSuggestion {
            email: c.email,
            name: c.name,
        })
        .collect();

    Ok(Json(AutocompleteResponse { suggestions }).into_response())
}

/// `GET /api/contacts/:id`
///
/// Returns a single contact by id. Returns 404 if not found.
pub async fn get_contact_handler(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    let contact = db::contacts::get_contact(&conn, &id)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    match contact {
        Some(c) => Ok(Json(c).into_response()),
        None => Err(AppError::NotFound(format!("Contact '{id}' not found"))),
    }
}

/// `DELETE /api/contacts/:id`
///
/// Deletes a contact by id. Returns 404 if the contact does not exist.
pub async fn delete_contact_handler(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    let deleted = db::contacts::delete_contact(&conn, &id)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    if deleted {
        Ok(Json(serde_json::json!({ "status": "deleted" })).into_response())
    } else {
        Err(AppError::NotFound(format!("Contact '{id}' not found")))
    }
}
