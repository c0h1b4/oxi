use serde::{Deserialize, Serialize};

/// Default page size for paginated list queries.
pub fn default_per_page() -> u32 {
    50
}

/// Query parameters for `GET /api/folders/:folder/messages`.
#[derive(Deserialize)]
pub(crate) struct ListMessagesQuery {
    #[serde(default)]
    pub(crate) page: u32,
    #[serde(default = "default_per_page")]
    pub(crate) per_page: u32,
}

/// Request body for `PATCH /api/messages/:folder/:uid/flags`.
#[derive(Deserialize)]
pub(crate) struct UpdateFlagsRequest {
    pub(crate) flags: Vec<String>,
    pub(crate) add: bool,
}

/// Request body for `POST /api/messages/move`.
#[derive(Deserialize)]
pub(crate) struct MoveMessageRequest {
    pub(crate) from_folder: String,
    pub(crate) to_folder: String,
    pub(crate) uid: u32,
}

/// Response envelope for `GET /api/folders/:folder/messages`.
#[derive(Serialize)]
pub(crate) struct ListMessagesResponse {
    pub(crate) messages: Vec<MessageSummary>,
    pub(crate) total_count: u32,
    pub(crate) page: u32,
    pub(crate) per_page: u32,
    pub(crate) syncing: bool,
}

/// A message summary in the list response.
#[derive(Serialize, Clone)]
pub struct MessageSummary {
    pub uid: u32,
    pub folder: String,
    pub subject: String,
    pub from_address: String,
    pub from_name: String,
    pub to_addresses: String,
    pub date: String,
    pub flags: String,
    pub size: u32,
    pub has_attachments: bool,
    pub snippet: String,
    pub reaction: Option<String>,
    pub tags: Vec<crate::db::tags::MessageTag>,
}

/// An email address entry for the detail response.
#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct AddressEntry {
    pub(crate) name: Option<String>,
    pub(crate) address: String,
}

/// Parse a JSON-encoded address list string (e.g. from the SQLite cache) into
/// a `Vec<AddressEntry>`. Returns an empty vec on parse failure.
pub(crate) fn parse_address_list(json_str: &str) -> Vec<AddressEntry> {
    serde_json::from_str(json_str).unwrap_or_default()
}

/// Split a comma-separated flags string into a `Vec<String>`.
pub(crate) fn parse_flags(flags_csv: &str) -> Vec<String> {
    if flags_csv.is_empty() {
        return vec![];
    }
    flags_csv
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Response for `GET /api/messages/:folder/:uid`.
#[derive(Serialize)]
pub(crate) struct MessageDetailResponse {
    pub(crate) uid: u32,
    pub(crate) folder: String,
    pub(crate) subject: String,
    pub(crate) from_address: String,
    pub(crate) from_name: String,
    pub(crate) to_addresses: Vec<AddressEntry>,
    pub(crate) cc_addresses: Vec<AddressEntry>,
    pub(crate) date: String,
    pub(crate) flags: Vec<String>,
    pub(crate) has_attachments: bool,
    pub(crate) html: Option<String>,
    pub(crate) text: Option<String>,
    pub(crate) raw_headers: String,
    pub(crate) attachments: Vec<AttachmentMeta>,
    pub(crate) thread: Vec<ThreadMessage>,
}

/// A message summary within a thread.
#[derive(Serialize)]
pub(crate) struct ThreadMessage {
    pub(crate) uid: u32,
    pub(crate) folder: String,
    pub(crate) message_id: Option<String>,
    pub(crate) in_reply_to: Option<String>,
    pub(crate) subject: String,
    pub(crate) from_address: String,
    pub(crate) from_name: String,
    pub(crate) to_addresses: String,
    pub(crate) cc_addresses: String,
    pub(crate) date: String,
    pub(crate) flags: String,
    pub(crate) size: u32,
    pub(crate) has_attachments: bool,
    pub(crate) snippet: String,
}

/// Attachment metadata (without the binary data).
#[derive(Serialize, Deserialize)]
pub(crate) struct AttachmentMeta {
    pub(crate) id: String,
    pub(crate) filename: Option<String>,
    pub(crate) content_type: String,
    pub(crate) size: usize,
    pub(crate) content_id: Option<String>,
}
