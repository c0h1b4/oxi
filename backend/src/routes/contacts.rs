use std::sync::Arc;

use axum::extract::{Multipart, Path, Query};
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

// ---------------------------------------------------------------------------
// vCard import/export
// ---------------------------------------------------------------------------

/// Escape special characters in a vCard field value.
fn vcard_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace(',', "\\,")
        .replace(';', "\\;")
        .replace('\n', "\\n")
}

/// Serialize a Contact to vCard 3.0 format.
fn contact_to_vcard(c: &Contact) -> String {
    let mut lines = Vec::new();
    lines.push("BEGIN:VCARD".to_string());
    lines.push("VERSION:3.0".to_string());

    let name = if c.name.is_empty() { &c.email } else { &c.name };
    lines.push(format!("FN:{}", vcard_escape(name)));
    lines.push(format!("N:{};;;;", vcard_escape(name)));
    lines.push(format!("EMAIL:{}", c.email));

    if !c.company.is_empty() {
        lines.push(format!("ORG:{}", vcard_escape(&c.company)));
    }
    if !c.notes.is_empty() {
        lines.push(format!("NOTE:{}", vcard_escape(&c.notes)));
    }

    lines.push("END:VCARD".to_string());
    lines.join("\r\n")
}

/// A parsed vCard entry with optional fields.
struct ParsedVCard {
    name: String,
    email: String,
    company: String,
    notes: String,
}

/// Parse vCard data (potentially multiple entries) from a string.
fn parse_vcards(data: &str) -> Vec<ParsedVCard> {
    let mut results = Vec::new();

    // Unfold continuation lines (lines starting with space or tab are continuations).
    let unfolded = data
        .replace("\r\n ", "")
        .replace("\r\n\t", "")
        .replace("\n ", "")
        .replace("\n\t", "");

    let mut in_vcard = false;
    let mut name = String::new();
    let mut email = String::new();
    let mut company = String::new();
    let mut notes = String::new();

    for line in unfolded.lines() {
        let line = line.trim();

        if line.eq_ignore_ascii_case("BEGIN:VCARD") {
            in_vcard = true;
            name.clear();
            email.clear();
            company.clear();
            notes.clear();
            continue;
        }

        if line.eq_ignore_ascii_case("END:VCARD") {
            if in_vcard && !email.is_empty() {
                results.push(ParsedVCard {
                    name: name.clone(),
                    email: email.clone(),
                    company: company.clone(),
                    notes: notes.clone(),
                });
            }
            in_vcard = false;
            continue;
        }

        if !in_vcard {
            continue;
        }

        // Extract field name and value (handle parameters like EMAIL;TYPE=WORK:...)
        let (field_name, value) = if let Some(colon_pos) = line.find(':') {
            let raw_key = &line[..colon_pos];
            let value = &line[colon_pos + 1..];
            // Strip parameters (e.g., EMAIL;TYPE=WORK -> EMAIL)
            let field = if let Some(semi_pos) = raw_key.find(';') {
                &raw_key[..semi_pos]
            } else {
                raw_key
            };
            (field.to_uppercase(), value.to_string())
        } else {
            continue;
        };

        // Unescape vCard values
        let value = value
            .replace("\\n", "\n")
            .replace("\\,", ",")
            .replace("\\;", ";")
            .replace("\\\\", "\\");

        match field_name.as_str() {
            "FN" => {
                if name.is_empty() {
                    name = value;
                }
            }
            "N" => {
                // N:Last;First;Middle;Prefix;Suffix — use FN if available, else reconstruct
                if name.is_empty() {
                    let parts: Vec<&str> = value.splitn(5, ';').collect();
                    let last = parts.first().unwrap_or(&"").trim();
                    let first = parts.get(1).unwrap_or(&"").trim();
                    let combined = format!("{first} {last}").trim().to_string();
                    if !combined.is_empty() {
                        name = combined;
                    }
                }
            }
            "EMAIL" => {
                if email.is_empty() {
                    email = value;
                }
            }
            "ORG" => {
                if company.is_empty() {
                    company = value;
                }
            }
            "NOTE" => {
                if notes.is_empty() {
                    notes = value;
                }
            }
            _ => {}
        }
    }

    results
}

/// `GET /api/contacts/export`
///
/// Export all contacts as a .vcf file download.
pub async fn export_contacts_handler(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
) -> Result<Response, AppError> {
    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    let contacts = db::contacts::list_contacts(&conn, None, 10000, 0)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    let vcf: String = contacts
        .iter()
        .map(contact_to_vcard)
        .collect::<Vec<_>>()
        .join("\r\n");

    Ok(Response::builder()
        .header("content-type", "text/vcard; charset=utf-8")
        .header(
            "content-disposition",
            "attachment; filename=\"contacts.vcf\"",
        )
        .body(axum::body::Body::from(vcf))
        .unwrap())
}

/// `GET /api/contacts/{id}/export`
///
/// Export a single contact as a .vcf file download.
pub async fn export_single_contact_handler(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    let contact = db::contacts::get_contact(&conn, &id)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    match contact {
        Some(c) => {
            let vcf = contact_to_vcard(&c);
            let filename = if c.name.is_empty() {
                "contact.vcf".to_string()
            } else {
                format!("{}.vcf", c.name.replace(' ', "_"))
            };

            Ok(Response::builder()
                .header("content-type", "text/vcard; charset=utf-8")
                .header(
                    "content-disposition",
                    format!("attachment; filename=\"{filename}\""),
                )
                .body(axum::body::Body::from(vcf))
                .unwrap())
        }
        None => Err(AppError::NotFound(format!("Contact '{id}' not found"))),
    }
}

/// Import response with counts.
#[derive(Serialize)]
pub struct ImportResponse {
    pub created: usize,
    pub updated: usize,
    pub skipped: usize,
}

/// `POST /api/contacts/import`
///
/// Import contacts from an uploaded .vcf file (multipart form data).
pub async fn import_contacts_handler(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    mut multipart: Multipart,
) -> Result<Response, AppError> {
    // Read the uploaded file content
    let mut vcf_content = String::new();
    if let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("Invalid multipart data: {e}")))?
    {
        let data = field
            .bytes()
            .await
            .map_err(|e| AppError::BadRequest(format!("Failed to read field: {e}")))?;
        vcf_content =
            String::from_utf8(data.to_vec()).map_err(|_| AppError::BadRequest("Invalid UTF-8 in vCard file".to_string()))?;
    }

    if vcf_content.is_empty() {
        return Err(AppError::BadRequest("No file uploaded".to_string()));
    }

    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    let now: String = conn
        .query_row("SELECT datetime('now')", [], |row| row.get(0))
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    let parsed = parse_vcards(&vcf_content);

    let mut created = 0usize;
    let mut updated = 0usize;
    let mut skipped = 0usize;

    for entry in &parsed {
        if entry.email.is_empty() {
            skipped += 1;
            continue;
        }

        let existing = db::contacts::get_contact_by_email(&conn, &entry.email)
            .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

        match existing {
            Some(mut c) => {
                // Merge only empty fields (don't overwrite manual edits).
                let mut changed = false;
                if c.name.is_empty() && !entry.name.is_empty() {
                    c.name = entry.name.clone();
                    changed = true;
                }
                if c.company.is_empty() && !entry.company.is_empty() {
                    c.company = entry.company.clone();
                    changed = true;
                }
                if c.notes.is_empty() && !entry.notes.is_empty() {
                    c.notes = entry.notes.clone();
                    changed = true;
                }
                if changed {
                    c.updated_at = now.clone();
                    db::contacts::upsert_contact(&conn, &c)
                        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;
                    updated += 1;
                } else {
                    skipped += 1;
                }
            }
            None => {
                let contact = Contact {
                    id: Uuid::new_v4().to_string(),
                    email: entry.email.clone(),
                    name: entry.name.clone(),
                    company: entry.company.clone(),
                    notes: entry.notes.clone(),
                    is_favorite: false,
                    last_contacted: None,
                    contact_count: 0,
                    source: "imported".to_string(),
                    created_at: now.clone(),
                    updated_at: now.clone(),
                };
                db::contacts::upsert_contact(&conn, &contact)
                    .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;
                created += 1;
            }
        }
    }

    Ok(Json(ImportResponse {
        created,
        updated,
        skipped,
    })
    .into_response())
}
