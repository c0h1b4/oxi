use async_trait::async_trait;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// Represents an IMAP folder (mailbox).
#[derive(Debug, Clone, Serialize)]
pub struct ImapFolder {
    /// Folder name as returned by the IMAP server (e.g. "INBOX", "Sent").
    pub name: String,
    /// Delimiter used by the server (e.g. "/" or ".").
    pub delimiter: Option<String>,
    /// IMAP attributes for this folder (e.g. `\Noselect`, `\HasChildren`).
    pub attributes: Vec<String>,
}

/// A parsed email address with optional display name.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EmailAddress {
    /// Display name, if present (e.g. "Alice Smith").
    pub name: Option<String>,
    /// The actual email address (e.g. "alice@example.com").
    pub address: String,
}

/// A lightweight summary of an email message (envelope data).
#[derive(Debug, Clone, Serialize)]
pub struct ImapMessageHeader {
    /// IMAP UID of the message within its folder.
    pub uid: u32,
    /// Subject line.
    pub subject: Option<String>,
    /// Sender(s) of the message.
    pub from: Vec<EmailAddress>,
    /// Recipient(s) of the message.
    pub to: Vec<EmailAddress>,
    /// Date header value (raw string from the server).
    pub date: Option<String>,
    /// IMAP flags currently set on this message (e.g. `\Seen`, `\Flagged`).
    pub flags: Vec<String>,
    /// Whether this message has attachments (derived from BODYSTRUCTURE).
    pub has_attachments: bool,
    /// RFC 2822 size of the message in bytes.
    pub size: u32,
    /// Message-ID header value for threading.
    pub message_id: Option<String>,
    /// In-Reply-To header value for threading.
    pub in_reply_to: Option<String>,
    /// References header value for threading.
    pub references: Option<String>,
    /// CC addresses.
    pub cc: Vec<EmailAddress>,
}

/// The full body of an email message, including attachments.
#[derive(Debug, Clone, Serialize)]
pub struct ImapMessageBody {
    /// IMAP UID of the message within its folder.
    pub uid: u32,
    /// Plain-text body part, if available.
    pub text_plain: Option<String>,
    /// HTML body part, if available.
    pub text_html: Option<String>,
    /// List of attachments found in the message.
    pub attachments: Vec<ImapAttachment>,
    /// Raw RFC 822 headers as a single string.
    pub raw_headers: String,
}

/// Metadata about a single attachment in an email message.
#[derive(Debug, Clone, Serialize)]
pub struct ImapAttachment {
    /// Filename of the attachment, if provided by the sender.
    pub filename: Option<String>,
    /// MIME content type (e.g. "application/pdf").
    pub content_type: String,
    /// Size in bytes.
    pub size: usize,
    /// Raw attachment content.
    pub data: Vec<u8>,
    /// Content-ID for inline images (e.g. "image001@01D1234"), without angle brackets.
    pub content_id: Option<String>,
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur during IMAP operations.
#[derive(Debug)]
pub enum ImapError {
    /// Could not connect to the IMAP server.
    ConnectionFailed(String),
    /// The server rejected our credentials.
    AuthenticationFailed,
    /// The requested folder does not exist.
    FolderNotFound(String),
    /// The requested message UID was not found in the given folder.
    MessageNotFound { uid: u32, folder: String },
    /// A low-level IMAP protocol error.
    ProtocolError(String),
}

impl fmt::Display for ImapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImapError::ConnectionFailed(msg) => write!(f, "Connection failed: {msg}"),
            ImapError::AuthenticationFailed => write!(f, "Authentication failed"),
            ImapError::FolderNotFound(name) => write!(f, "Folder not found: {name}"),
            ImapError::MessageNotFound { uid, folder } => {
                write!(f, "Message UID {uid} not found in folder {folder}")
            }
            ImapError::ProtocolError(msg) => write!(f, "Protocol error: {msg}"),
        }
    }
}

impl std::error::Error for ImapError {}

// ---------------------------------------------------------------------------
// Connection parameters (passed explicitly to every method)
// ---------------------------------------------------------------------------

/// Parameters needed to establish an IMAP connection.
/// Passed explicitly to every trait method so the trait stays stateless.
#[derive(Debug, Clone)]
pub struct ImapCredentials {
    pub host: String,
    pub port: u16,
    pub tls: bool,
    pub email: String,
    pub password: String,
}

// ---------------------------------------------------------------------------
// Trait definition
// ---------------------------------------------------------------------------

/// Lightweight result of an IMAP `SELECT` command.
#[derive(Debug, Clone, Serialize)]
pub struct FolderStatus {
    /// UIDVALIDITY — changes when the mailbox is rebuilt or UIDs are reassigned.
    pub uid_validity: u32,
    /// The total number of messages currently in the folder.
    pub exists: u32,
    /// The highest UID that will be assigned to the next appended message.
    pub uid_next: u32,
}

/// Abstraction over IMAP operations.
///
/// Every method receives explicit connection parameters so that the trait
/// remains stateless — no persistent connections are held.
///
/// The `Send + Sync` bounds allow implementations to be shared across
/// Tokio tasks and stored in `Arc`.
#[allow(dead_code)]
#[async_trait]
pub trait ImapClient: Send + Sync {
    /// List all folders (mailboxes) on the server.
    async fn list_folders(&self, creds: &ImapCredentials) -> Result<Vec<ImapFolder>, ImapError>;

    /// Perform a lightweight `SELECT` on a folder to get its status
    /// (UIDVALIDITY, EXISTS count, UIDNEXT) without fetching any messages.
    async fn folder_status(
        &self,
        creds: &ImapCredentials,
        folder: &str,
    ) -> Result<FolderStatus, ImapError>;

    /// Fetch message headers (envelopes) for a range of UIDs in a folder.
    async fn fetch_headers(
        &self,
        creds: &ImapCredentials,
        folder: &str,
        uid_range: &str,
    ) -> Result<Vec<ImapMessageHeader>, ImapError>;

    /// Fetch the full body of a single message by UID.
    async fn fetch_body(
        &self,
        creds: &ImapCredentials,
        folder: &str,
        uid: u32,
    ) -> Result<ImapMessageBody, ImapError>;

    /// Add flags to a message (IMAP +FLAGS).
    async fn add_flags(
        &self,
        creds: &ImapCredentials,
        folder: &str,
        uid: u32,
        flags: &[&str],
    ) -> Result<(), ImapError>;

    /// Remove flags from a message (IMAP -FLAGS).
    async fn remove_flags(
        &self,
        creds: &ImapCredentials,
        folder: &str,
        uid: u32,
        flags: &[&str],
    ) -> Result<(), ImapError>;

    /// Set (replace) the flags on a message.
    async fn set_flags(
        &self,
        creds: &ImapCredentials,
        folder: &str,
        uid: u32,
        flags: &[&str],
    ) -> Result<(), ImapError>;

    /// Move a message from one folder to another.
    async fn move_message(
        &self,
        creds: &ImapCredentials,
        from_folder: &str,
        uid: u32,
        to_folder: &str,
    ) -> Result<(), ImapError>;

    /// Permanently remove a message that has the `\Deleted` flag.
    async fn expunge_message(
        &self,
        creds: &ImapCredentials,
        folder: &str,
        uid: u32,
    ) -> Result<(), ImapError>;

    /// Append a raw RFC822 message to a folder (e.g. saving sent mail to "Sent").
    async fn append_message(
        &self,
        creds: &ImapCredentials,
        folder: &str,
        message_bytes: &[u8],
        flags: &[&str],
    ) -> Result<(), ImapError>;

    /// Create a new folder (mailbox) and subscribe to it.
    async fn create_folder(
        &self,
        creds: &ImapCredentials,
        folder_name: &str,
    ) -> Result<(), ImapError>;

    /// Rename an existing folder.
    async fn rename_folder(
        &self,
        creds: &ImapCredentials,
        from: &str,
        to: &str,
    ) -> Result<(), ImapError>;

    /// Permanently delete a folder (mailbox).
    async fn delete_folder(
        &self,
        creds: &ImapCredentials,
        folder_name: &str,
    ) -> Result<(), ImapError>;

    /// Subscribe to or unsubscribe from a folder.
    async fn subscribe_folder(
        &self,
        creds: &ImapCredentials,
        folder_name: &str,
        subscribe: bool,
    ) -> Result<(), ImapError>;
}

// ---------------------------------------------------------------------------
// Real implementation backed by async-imap
// ---------------------------------------------------------------------------

/// Production IMAP client that uses `async-imap` and `mail-parser`.
///
/// This is a stateless unit struct — every method creates a fresh connection,
/// performs the operation, and disconnects.
pub struct RealImapClient;

// ---- Connection helper ----------------------------------------------------

/// Establish an authenticated IMAP session.
///
/// Returns a `Session` over a TLS stream (when `creds.tls` is true) or a
/// plain TCP stream.  Because the two stream types are different concrete
/// types we use an enum wrapper that implements the traits `async-imap`
/// requires (`tokio::io::AsyncRead + AsyncWrite + Unpin + Debug`).
async fn connect(
    creds: &ImapCredentials,
) -> Result<async_imap::Session<ImapStream>, ImapError> {
    let connect_future = tokio::net::TcpStream::connect((creds.host.as_str(), creds.port));
    // 10 second timeout for the initial TCP connection
    let tcp = tokio::time::timeout(std::time::Duration::from_secs(10), connect_future)
        .await
        .map_err(|_| ImapError::ConnectionFailed("connection timed out".to_string()))?
        .map_err(|e| ImapError::ConnectionFailed(e.to_string()))?;

    if creds.tls {
        let tls = async_native_tls::TlsConnector::new();
        let tls_stream = tls
            .connect(&creds.host, tcp)
            .await
            .map_err(|e| ImapError::ConnectionFailed(e.to_string()))?;
        let client = async_imap::Client::new(ImapStream::Tls(tls_stream));
        let session = client
            .login(&creds.email, &creds.password)
            .await
            .map_err(|(e, _)| classify_login_error(e))?;
        Ok(session)
    } else {
        let client = async_imap::Client::new(ImapStream::Plain(tcp));
        let session = client
            .login(&creds.email, &creds.password)
            .await
            .map_err(|(e, _)| classify_login_error(e))?;
        Ok(session)
    }
}

/// Classify an `async_imap::error::Error` that occurred during LOGIN.
fn classify_login_error(err: async_imap::error::Error) -> ImapError {
    match err {
        async_imap::error::Error::No(_) => ImapError::AuthenticationFailed,
        async_imap::error::Error::Io(e) => ImapError::ConnectionFailed(e.to_string()),
        async_imap::error::Error::ConnectionLost => {
            ImapError::ConnectionFailed("connection lost".to_string())
        }
        other => ImapError::ProtocolError(other.to_string()),
    }
}

/// Map a generic `async_imap` error to our `ImapError`.
fn map_imap_error(err: async_imap::error::Error) -> ImapError {
    match err {
        async_imap::error::Error::No(msg) => ImapError::ProtocolError(format!("NO: {msg}")),
        async_imap::error::Error::Io(e) => ImapError::ConnectionFailed(e.to_string()),
        async_imap::error::Error::ConnectionLost => {
            ImapError::ConnectionFailed("connection lost".to_string())
        }
        other => ImapError::ProtocolError(other.to_string()),
    }
}

// ---- Stream enum ----------------------------------------------------------

/// A wrapper enum so that `Session` can be generic over a single type
/// regardless of whether TLS is used.
#[derive(Debug)]
enum ImapStream {
    Tls(async_native_tls::TlsStream<tokio::net::TcpStream>),
    Plain(tokio::net::TcpStream),
}

impl tokio::io::AsyncRead for ImapStream {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.get_mut() {
            ImapStream::Tls(s) => std::pin::Pin::new(s).poll_read(cx, buf),
            ImapStream::Plain(s) => std::pin::Pin::new(s).poll_read(cx, buf),
        }
    }
}

impl tokio::io::AsyncWrite for ImapStream {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        match self.get_mut() {
            ImapStream::Tls(s) => std::pin::Pin::new(s).poll_write(cx, buf),
            ImapStream::Plain(s) => std::pin::Pin::new(s).poll_write(cx, buf),
        }
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.get_mut() {
            ImapStream::Tls(s) => std::pin::Pin::new(s).poll_flush(cx),
            ImapStream::Plain(s) => std::pin::Pin::new(s).poll_flush(cx),
        }
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.get_mut() {
            ImapStream::Tls(s) => std::pin::Pin::new(s).poll_shutdown(cx),
            ImapStream::Plain(s) => std::pin::Pin::new(s).poll_shutdown(cx),
        }
    }
}

// ---- Helper: convert NameAttribute to string ------------------------------

fn name_attribute_to_string(attr: &async_imap::types::NameAttribute<'_>) -> String {
    use async_imap::types::NameAttribute;
    match attr {
        NameAttribute::NoInferiors => "\\Noinferiors".to_string(),
        NameAttribute::NoSelect => "\\Noselect".to_string(),
        NameAttribute::Marked => "\\Marked".to_string(),
        NameAttribute::Unmarked => "\\Unmarked".to_string(),
        NameAttribute::All => "\\All".to_string(),
        NameAttribute::Archive => "\\Archive".to_string(),
        NameAttribute::Drafts => "\\Drafts".to_string(),
        NameAttribute::Flagged => "\\Flagged".to_string(),
        NameAttribute::Junk => "\\Junk".to_string(),
        NameAttribute::Sent => "\\Sent".to_string(),
        NameAttribute::Trash => "\\Trash".to_string(),
        NameAttribute::Extension(s) => s.to_string(),
        _ => format!("{attr:?}"),
    }
}

// ---- Helper: convert Flag to string ---------------------------------------

fn flag_to_string(flag: &async_imap::types::Flag<'_>) -> String {
    use async_imap::types::Flag;
    match flag {
        Flag::Seen => "\\Seen".to_string(),
        Flag::Answered => "\\Answered".to_string(),
        Flag::Flagged => "\\Flagged".to_string(),
        Flag::Deleted => "\\Deleted".to_string(),
        Flag::Draft => "\\Draft".to_string(),
        Flag::Recent => "\\Recent".to_string(),
        Flag::MayCreate => "\\*".to_string(),
        Flag::Custom(s) => s.to_string(),
    }
}

// ---- Helper: decode RFC 2047 encoded words --------------------------------

/// Decode RFC 2047 encoded-word sequences like `=?utf-8?b?SGVsbG8=?=`.
fn decode_rfc2047(input: &str) -> String {
    // Quick check: if there's no encoded-word marker, return as-is.
    if !input.contains("=?") {
        return input.to_string();
    }

    let mut result = String::with_capacity(input.len());
    let mut remaining = input;

    while let Some(start) = remaining.find("=?") {
        // Add any text before the encoded word.
        result.push_str(&remaining[..start]);

        let after_start = &remaining[start + 2..];
        // Find charset?encoding?text?= pattern.
        if let Some(q1) = after_start.find('?') {
            let charset = &after_start[..q1];
            let after_charset = &after_start[q1 + 1..];
            if let Some(q2) = after_charset.find('?') {
                let encoding = &after_charset[..q2];
                let after_encoding = &after_charset[q2 + 1..];
                if let Some(end) = after_encoding.find("?=") {
                    let encoded_text = &after_encoding[..end];
                    let decoded_bytes = match encoding.to_ascii_uppercase().as_str() {
                        "B" => base64::Engine::decode(
                            &base64::engine::general_purpose::STANDARD,
                            encoded_text,
                        )
                        .ok(),
                        "Q" => decode_quoted_printable_header(encoded_text),
                        _ => None,
                    };

                    if let Some(bytes) = decoded_bytes {
                        let text = decode_charset(charset, &bytes);
                        result.push_str(&text);
                    } else {
                        // Failed to decode — keep the original encoded word.
                        let full_len = 2 + q1 + 1 + q2 + 1 + end + 2;
                        result.push_str(&remaining[start..start + full_len]);
                    }

                    // Skip past the encoded word.
                    let skip = q1 + 1 + q2 + 1 + end + 2;
                    remaining = &after_start[skip..];
                    // Strip whitespace between consecutive encoded words (RFC 2047 §6.2).
                    if remaining.starts_with("=?") || remaining.trim_start().starts_with("=?") {
                        remaining = remaining.trim_start();
                    }
                    continue;
                }
            }
        }

        // Malformed encoded word — keep the `=?` and move past it.
        result.push_str("=?");
        remaining = after_start;
    }

    result.push_str(remaining);
    result
}

/// Decode quoted-printable as used in RFC 2047 headers (underscores = spaces).
fn decode_quoted_printable_header(input: &str) -> Option<Vec<u8>> {
    let mut bytes = Vec::with_capacity(input.len());
    let mut chars = input.bytes();
    while let Some(b) = chars.next() {
        match b {
            b'_' => bytes.push(b' '),
            b'=' => {
                let hi = chars.next()?;
                let lo = chars.next()?;
                let hex = [hi, lo];
                let hex_str = std::str::from_utf8(&hex).ok()?;
                let byte = u8::from_str_radix(hex_str, 16).ok()?;
                bytes.push(byte);
            }
            _ => bytes.push(b),
        }
    }
    Some(bytes)
}

/// Best-effort charset decoding. UTF-8 and Latin-1 are the most common.
fn decode_charset(charset: &str, bytes: &[u8]) -> String {
    match charset.to_ascii_lowercase().as_str() {
        "utf-8" | "utf8" => String::from_utf8_lossy(bytes).into_owned(),
        "iso-8859-1" | "latin1" | "latin-1" => bytes.iter().map(|&b| b as char).collect(),
        // For other charsets, try UTF-8 first, then fall back to Latin-1.
        _ => String::from_utf8(bytes.to_vec())
            .unwrap_or_else(|_| bytes.iter().map(|&b| b as char).collect()),
    }
}

// ---- Helper: convert imap_proto::Address to EmailAddress ------------------

fn imap_address_to_email(addr: &async_imap::imap_proto::types::Address<'_>) -> EmailAddress {
    let name = addr
        .name
        .as_ref()
        .and_then(|b| std::str::from_utf8(b).ok())
        .map(decode_rfc2047);

    let mailbox = addr
        .mailbox
        .as_ref()
        .and_then(|b| std::str::from_utf8(b).ok())
        .unwrap_or("");
    let host = addr
        .host
        .as_ref()
        .and_then(|b| std::str::from_utf8(b).ok())
        .unwrap_or("");

    let address = if host.is_empty() {
        mailbox.to_string()
    } else {
        format!("{mailbox}@{host}")
    };

    EmailAddress { name, address }
}

// ---- Helper: detect attachments from BODYSTRUCTURE -----------------------

fn has_attachments(bs: &async_imap::imap_proto::types::BodyStructure<'_>) -> bool {
    use async_imap::imap_proto::types::BodyStructure;
    match bs {
        BodyStructure::Basic { common, .. } => {
            // Check for explicit "attachment" disposition.
            if let Some(ref disp) = common.disposition
                && disp.ty.eq_ignore_ascii_case("attachment")
            {
                return true;
            }
            // Non-text basic parts (image, application, audio, video) are likely attachments.
            let ty = common.ty.ty.to_ascii_lowercase();
            matches!(ty.as_str(), "application" | "image" | "audio" | "video")
        }
        BodyStructure::Text { common, .. } => {
            // Text parts with "attachment" disposition count.
            if let Some(ref disp) = common.disposition {
                return disp.ty.eq_ignore_ascii_case("attachment");
            }
            false
        }
        BodyStructure::Message { body, .. } => has_attachments(body),
        BodyStructure::Multipart { bodies, .. } => bodies.iter().any(has_attachments),
    }
}

// ---- Trait implementation -------------------------------------------------

#[async_trait]
impl ImapClient for RealImapClient {
    async fn folder_status(
        &self,
        creds: &ImapCredentials,
        folder: &str,
    ) -> Result<FolderStatus, ImapError> {
        let mut session = connect(creds).await?;

        let mailbox = session
            .select(folder)
            .await
            .map_err(|e| match &e {
                async_imap::error::Error::No(msg)
                    if msg.to_lowercase().contains("not found")
                        || msg.to_lowercase().contains("doesn't exist")
                        || msg.to_lowercase().contains("does not exist")
                        || msg.to_lowercase().contains("no such") =>
                {
                    ImapError::FolderNotFound(folder.to_string())
                }
                _ => map_imap_error(e),
            })?;

        let uid_validity = mailbox.uid_validity.unwrap_or(0);
        let exists = mailbox.exists;
        let uid_next = mailbox.uid_next.unwrap_or(0);

        let _ = session.logout().await;
        Ok(FolderStatus {
            uid_validity,
            exists,
            uid_next,
        })
    }

    async fn list_folders(&self, creds: &ImapCredentials) -> Result<Vec<ImapFolder>, ImapError> {
        let mut session = connect(creds).await?;

        let folders = {
            let names_stream = session
                .list(Some(""), Some("*"))
                .await
                .map_err(map_imap_error)?;

            let mut names_stream = std::pin::pin!(names_stream);
            let mut names = Vec::new();
            while let Some(result) = names_stream.next().await {
                names.push(result.map_err(map_imap_error)?);
            }

            names
                .iter()
                .map(|n| ImapFolder {
                    name: n.name().to_string(),
                    delimiter: n.delimiter().map(|d| d.to_string()),
                    attributes: n
                        .attributes()
                        .iter()
                        .map(name_attribute_to_string)
                        .collect(),
                })
                .collect()
        };

        let _ = session.logout().await;
        Ok(folders)
    }

    async fn fetch_headers(
        &self,
        creds: &ImapCredentials,
        folder: &str,
        uid_range: &str,
    ) -> Result<Vec<ImapMessageHeader>, ImapError> {
        let mut session = connect(creds).await?;

        session
            .select(folder)
            .await
            .map_err(|e| match &e {
                async_imap::error::Error::No(msg)
                    if msg.to_lowercase().contains("not found")
                        || msg.to_lowercase().contains("doesn't exist")
                        || msg.to_lowercase().contains("does not exist")
                        || msg.to_lowercase().contains("no such") =>
                {
                    ImapError::FolderNotFound(folder.to_string())
                }
                _ => map_imap_error(e),
            })?;

        let headers = {
            // Fetch ENVELOPE, FLAGS, BODYSTRUCTURE, RFC822.SIZE, and threading headers.
            // We only fetch Message-ID, In-Reply-To, and References (a few bytes per message)
            // rather than full raw headers, to keep bulk syncs lightweight.
            let mut fetch_stream = session
                .uid_fetch(
                    uid_range,
                    "(UID ENVELOPE FLAGS BODYSTRUCTURE RFC822.SIZE BODY.PEEK[HEADER.FIELDS (Message-ID In-Reply-To References)])",
                )
                .await
                .map_err(map_imap_error)?;

            let mut fetches = Vec::new();
            while let Some(result) = fetch_stream.next().await {
                fetches.push(result.map_err(map_imap_error)?);
            }

            let mut headers = Vec::with_capacity(fetches.len());
            for fetch in &fetches {
                let uid = match fetch.uid {
                    Some(u) => u,
                    None => continue,
                };

                // Parse threading headers from the small HEADER.FIELDS response.
                let raw_header_bytes = fetch.header();
                let parsed_threading = raw_header_bytes.and_then(|raw| {
                    mail_parser::MessageParser::default().parse(raw)
                });

                let (subject, from, to, cc, date) = if let Some(env) = fetch.envelope() {
                    let subject = env
                        .subject
                        .as_ref()
                        .and_then(|b| std::str::from_utf8(b).ok())
                        .map(decode_rfc2047);

                    let from: Vec<EmailAddress> = env
                        .from
                        .as_ref()
                        .map(|addrs| addrs.iter().map(imap_address_to_email).collect())
                        .unwrap_or_default();

                    let to: Vec<EmailAddress> = env
                        .to
                        .as_ref()
                        .map(|addrs| addrs.iter().map(imap_address_to_email).collect())
                        .unwrap_or_default();

                    let cc: Vec<EmailAddress> = env
                        .cc
                        .as_ref()
                        .map(|addrs| addrs.iter().map(imap_address_to_email).collect())
                        .unwrap_or_default();

                    let date = env
                        .date
                        .as_ref()
                        .and_then(|b| std::str::from_utf8(b).ok())
                        .map(|s| s.to_string());

                    (subject, from, to, cc, date)
                } else {
                    // No envelope — we can't fill subject/from/to/date from the
                    // small threading-only header fetch, so leave them empty.
                    // They'll be populated when the user opens the message body.
                    tracing::warn!(
                        uid = uid,
                        folder = %folder,
                        "ENVELOPE missing for message, headers will be empty until body is fetched"
                    );
                    (None, vec![], vec![], vec![], None)
                };

                // Extract threading headers from the small HEADER.FIELDS response.
                let message_id = parsed_threading.as_ref().and_then(|p| {
                    p.message_id().map(|s| format!("<{s}>"))
                });
                let in_reply_to = parsed_threading.as_ref().and_then(|p| {
                    let val = p.in_reply_to();
                    val.as_text().map(|s| format!("<{s}>"))
                });
                let references = parsed_threading.as_ref().and_then(|p| {
                    let val = p.references();
                    val.as_text_list()
                        .map(|list| list.iter().map(|s| format!("<{s}>")).collect::<Vec<_>>().join(" "))
                        .or_else(|| val.as_text().map(|s| format!("<{s}>")))
                });

                let flags: Vec<String> = fetch.flags().map(|f| flag_to_string(&f)).collect();

                let has_attach = fetch
                    .bodystructure()
                    .map(|bs| has_attachments(bs))
                    .unwrap_or(false);

                let size = fetch.size.unwrap_or(0);

                headers.push(ImapMessageHeader {
                    uid,
                    subject,
                    from,
                    to,
                    date,
                    flags,
                    has_attachments: has_attach,
                    size,
                    message_id,
                    in_reply_to,
                    references,
                    cc,
                });
            }
            headers
        };

        let _ = session.logout().await;
        Ok(headers)
    }

    async fn fetch_body(
        &self,
        creds: &ImapCredentials,
        folder: &str,
        uid: u32,
    ) -> Result<ImapMessageBody, ImapError> {
        let mut session = connect(creds).await?;

        session
            .select(folder)
            .await
            .map_err(|e| match &e {
                async_imap::error::Error::No(msg)
                    if msg.to_lowercase().contains("not found")
                        || msg.to_lowercase().contains("doesn't exist")
                        || msg.to_lowercase().contains("does not exist")
                        || msg.to_lowercase().contains("no such") =>
                {
                    ImapError::FolderNotFound(folder.to_string())
                }
                _ => map_imap_error(e),
            })?;

        let uid_str = uid.to_string();
        let body = {
            let mut fetch_stream = session
                .uid_fetch(&uid_str, "(UID BODY[])")
                .await
                .map_err(map_imap_error)?;

            let mut fetches = Vec::new();
            while let Some(result) = fetch_stream.next().await {
                fetches.push(result.map_err(map_imap_error)?);
            }

            let fetch = fetches.first().ok_or(ImapError::MessageNotFound {
                uid,
                folder: folder.to_string(),
            })?;

            let raw = fetch.body().ok_or_else(|| {
                ImapError::ProtocolError("BODY[] not returned by server".to_string())
            })?;

            use mail_parser::MimeHeaders;

            let parsed = mail_parser::MessageParser::default()
                .parse(raw)
                .ok_or_else(|| {
                    ImapError::ProtocolError("failed to parse RFC822 message".to_string())
                })?;

            let text_plain: Option<String> = parsed.body_text(0).map(|s| s.to_string());

            let text_html: Option<String> = parsed.body_html(0).map(|s| s.to_string());

            tracing::debug!(
                uid = uid,
                folder = %folder,
                total_parts = parsed.parts.len(),
                attachment_count = parsed.attachments().count(),
                has_text = text_plain.is_some(),
                has_html = text_html.is_some(),
                "fetch_body: parsed message structure"
            );

            let mut attachments = Vec::new();

            // Collect explicit attachments.
            for attachment in parsed.attachments() {
                let filename: Option<String> =
                    attachment.attachment_name().map(|s| s.to_string());
                let content_type: String = attachment.content_type().map_or_else(
                    || "application/octet-stream".to_string(),
                    |ct: &mail_parser::ContentType<'_>| {
                        if let Some(subtype) = ct.subtype() {
                            format!("{}/{}", ct.ctype(), subtype)
                        } else {
                            ct.ctype().to_string()
                        }
                    },
                );
                let content_id = attachment
                    .content_id()
                    .map(|s| s.trim_matches(|c| c == '<' || c == '>').to_string());
                let data = attachment.contents().to_vec();
                let size = data.len();
                attachments.push(ImapAttachment {
                    filename,
                    content_type,
                    size,
                    data,
                    content_id,
                });
            }

            // Also collect inline parts with Content-ID (e.g. embedded images
            // referenced via cid: URLs in the HTML body).
            for part in parsed.parts.iter() {
                if part.content_id().is_none() {
                    continue;
                }
                // Skip if this is a text/html or text/plain body part.
                let is_text = part
                    .content_type()
                    .is_some_and(|ct| ct.ctype() == "text");
                if is_text {
                    continue;
                }
                let cid = part
                    .content_id()
                    .unwrap()
                    .trim_matches(|c| c == '<' || c == '>')
                    .to_string();
                // Skip if we already captured this part via attachments().
                if attachments.iter().any(|a| a.content_id.as_deref() == Some(&cid)) {
                    continue;
                }
                let content_type: String = part.content_type().map_or_else(
                    || "application/octet-stream".to_string(),
                    |ct: &mail_parser::ContentType<'_>| {
                        if let Some(subtype) = ct.subtype() {
                            format!("{}/{}", ct.ctype(), subtype)
                        } else {
                            ct.ctype().to_string()
                        }
                    },
                );
                let data = part.contents().to_vec();
                let size = data.len();
                attachments.push(ImapAttachment {
                    filename: part.attachment_name().map(|s| s.to_string()),
                    content_type,
                    size,
                    data,
                    content_id: Some(cid),
                });
            }

            // Extract raw headers from the RFC 822 message.
            let raw_str = String::from_utf8_lossy(raw);
            let raw_headers = raw_str
                .split_once("\r\n\r\n")
                .or_else(|| raw_str.split_once("\n\n"))
                .map_or_else(|| raw_str.to_string(), |(h, _)| h.to_string());

            ImapMessageBody {
                uid,
                text_plain,
                text_html,
                attachments,
                raw_headers,
            }
        };

        let _ = session.logout().await;
        Ok(body)
    }

    async fn add_flags(
        &self,
        creds: &ImapCredentials,
        folder: &str,
        uid: u32,
        flags: &[&str],
    ) -> Result<(), ImapError> {
        let mut session = connect(creds).await?;
        session.select(folder).await.map_err(map_imap_error)?;

        let uid_str = uid.to_string();
        let flags_str = format!("+FLAGS ({})", flags.join(" "));
        {
            let mut store_stream = session
                .uid_store(&uid_str, &flags_str)
                .await
                .map_err(map_imap_error)?;
            while let Some(result) = store_stream.next().await {
                result.map_err(map_imap_error)?;
            }
        }

        let _ = session.logout().await;
        Ok(())
    }

    async fn remove_flags(
        &self,
        creds: &ImapCredentials,
        folder: &str,
        uid: u32,
        flags: &[&str],
    ) -> Result<(), ImapError> {
        let mut session = connect(creds).await?;
        session.select(folder).await.map_err(map_imap_error)?;

        let uid_str = uid.to_string();
        let flags_str = format!("-FLAGS ({})", flags.join(" "));
        {
            let mut store_stream = session
                .uid_store(&uid_str, &flags_str)
                .await
                .map_err(map_imap_error)?;
            while let Some(result) = store_stream.next().await {
                result.map_err(map_imap_error)?;
            }
        }

        let _ = session.logout().await;
        Ok(())
    }

    async fn set_flags(
        &self,
        creds: &ImapCredentials,
        folder: &str,
        uid: u32,
        flags: &[&str],
    ) -> Result<(), ImapError> {
        let mut session = connect(creds).await?;

        session.select(folder).await.map_err(map_imap_error)?;

        let uid_str = uid.to_string();
        let flags_str = format!("FLAGS ({})", flags.join(" "));
        {
            let mut store_stream = session
                .uid_store(&uid_str, &flags_str)
                .await
                .map_err(map_imap_error)?;

            // Consume the stream to completion so the command finishes.
            while let Some(result) = store_stream.next().await {
                result.map_err(map_imap_error)?;
            }
        }

        let _ = session.logout().await;
        Ok(())
    }

    async fn move_message(
        &self,
        creds: &ImapCredentials,
        from_folder: &str,
        uid: u32,
        to_folder: &str,
    ) -> Result<(), ImapError> {
        let mut session = connect(creds).await?;

        session
            .select(from_folder)
            .await
            .map_err(map_imap_error)?;

        let uid_str = uid.to_string();

        // Try UID MOVE first; fall back to COPY + DELETE + EXPUNGE if the
        // server does not support the MOVE extension.
        match session.uid_mv(&uid_str, to_folder).await {
            Ok(()) => {}
            Err(async_imap::error::Error::No(_) | async_imap::error::Error::Bad(_)) => {
                // Fallback: COPY, then flag \Deleted, then EXPUNGE.
                session
                    .uid_copy(&uid_str, to_folder)
                    .await
                    .map_err(map_imap_error)?;

                {
                    let mut store_stream = session
                        .uid_store(&uid_str, "+FLAGS (\\Deleted)")
                        .await
                        .map_err(map_imap_error)?;
                    while let Some(r) = store_stream.next().await {
                        r.map_err(map_imap_error)?;
                    }
                }

                {
                    let expunge_stream =
                        session.expunge().await.map_err(map_imap_error)?;
                    let mut expunge_stream = std::pin::pin!(expunge_stream);
                    while let Some(r) = expunge_stream.next().await {
                        r.map_err(map_imap_error)?;
                    }
                }
            }
            Err(e) => return Err(map_imap_error(e)),
        }

        let _ = session.logout().await;
        Ok(())
    }

    async fn expunge_message(
        &self,
        creds: &ImapCredentials,
        folder: &str,
        uid: u32,
    ) -> Result<(), ImapError> {
        let mut session = connect(creds).await?;

        session.select(folder).await.map_err(map_imap_error)?;

        let uid_str = uid.to_string();

        // Mark the message as \Deleted.
        {
            let mut store_stream = session
                .uid_store(&uid_str, "+FLAGS (\\Deleted)")
                .await
                .map_err(map_imap_error)?;
            while let Some(r) = store_stream.next().await {
                r.map_err(map_imap_error)?;
            }
        }

        // Try UID EXPUNGE for precision; fall back to EXPUNGE.
        let uid_expunge_ok = {
            match session.uid_expunge(&uid_str).await {
                Ok(stream) => {
                    let mut stream = std::pin::pin!(stream);
                    while let Some(r) = stream.next().await {
                        r.map_err(map_imap_error)?;
                    }
                    true
                }
                Err(_) => false,
            }
        };
        if !uid_expunge_ok {
            let stream = session.expunge().await.map_err(map_imap_error)?;
            let mut stream = std::pin::pin!(stream);
            while let Some(r) = stream.next().await {
                r.map_err(map_imap_error)?;
            }
        }

        let _ = session.logout().await;
        Ok(())
    }

    async fn append_message(
        &self,
        creds: &ImapCredentials,
        folder: &str,
        message_bytes: &[u8],
        flags: &[&str],
    ) -> Result<(), ImapError> {
        let mut session = connect(creds).await?;

        let flags_str: Vec<String> = flags.iter().map(|f| f.to_string()).collect();
        let flags_joined = if flags_str.is_empty() {
            None
        } else {
            Some(format!("({})", flags_str.join(" ")))
        };
        session
            .append(
                folder,
                flags_joined.as_deref(),
                None,
                message_bytes,
            )
            .await
            .map_err(map_imap_error)?;

        let _ = session.logout().await;
        Ok(())
    }

    async fn create_folder(
        &self,
        creds: &ImapCredentials,
        folder_name: &str,
    ) -> Result<(), ImapError> {
        let mut session = connect(creds).await?;
        session.create(folder_name).await.map_err(map_imap_error)?;
        session
            .subscribe(folder_name)
            .await
            .map_err(map_imap_error)?;
        session.logout().await.ok();
        Ok(())
    }

    async fn rename_folder(
        &self,
        creds: &ImapCredentials,
        from: &str,
        to: &str,
    ) -> Result<(), ImapError> {
        let mut session = connect(creds).await?;
        session.rename(from, to).await.map_err(map_imap_error)?;
        session.logout().await.ok();
        Ok(())
    }

    async fn delete_folder(
        &self,
        creds: &ImapCredentials,
        folder_name: &str,
    ) -> Result<(), ImapError> {
        let mut session = connect(creds).await?;
        session.delete(folder_name).await.map_err(map_imap_error)?;
        session.logout().await.ok();
        Ok(())
    }

    async fn subscribe_folder(
        &self,
        creds: &ImapCredentials,
        folder_name: &str,
        subscribe: bool,
    ) -> Result<(), ImapError> {
        let mut session = connect(creds).await?;
        if subscribe {
            session
                .subscribe(folder_name)
                .await
                .map_err(map_imap_error)?;
        } else {
            session
                .unsubscribe(folder_name)
                .await
                .map_err(map_imap_error)?;
        }
        session.logout().await.ok();
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Mock implementation (test-only)
// ---------------------------------------------------------------------------

#[cfg(test)]
pub mod mock {
    use super::*;
    use std::sync::Mutex;

    /// A mock IMAP client that returns pre-loaded data.
    ///
    /// Uses interior mutability (`Mutex`) so it can be shared behind `&self`.
    /// Build it up with the `.with_*()` builder methods, then pass it wherever
    /// an `&dyn ImapClient` is needed.
    pub struct MockImapClient {
        folders: Mutex<Vec<ImapFolder>>,
        headers: Mutex<Vec<ImapMessageHeader>>,
        bodies: Mutex<Vec<ImapMessageBody>>,
        folder_status: Mutex<Option<FolderStatus>>,
        should_fail: Mutex<Option<ImapError>>,
        pub appended: Mutex<Vec<(String, Vec<u8>)>>,
    }

    impl MockImapClient {
        /// Create a new empty mock.
        pub fn new() -> Self {
            Self {
                folders: Mutex::new(Vec::new()),
                headers: Mutex::new(Vec::new()),
                bodies: Mutex::new(Vec::new()),
                folder_status: Mutex::new(None),
                should_fail: Mutex::new(None),
                appended: Mutex::new(Vec::new()),
            }
        }

        /// Pre-load folders that `list_folders` will return.
        pub fn with_folders(self, folders: Vec<ImapFolder>) -> Self {
            *self.folders.lock().unwrap() = folders;
            self
        }

        /// Pre-load message headers that `fetch_headers` will return.
        pub fn with_headers(self, headers: Vec<ImapMessageHeader>) -> Self {
            *self.headers.lock().unwrap() = headers;
            self
        }

        /// Pre-load a folder status that `folder_status` will return.
        #[allow(dead_code)]
        pub fn with_folder_status(self, status: FolderStatus) -> Self {
            *self.folder_status.lock().unwrap() = Some(status);
            self
        }

        /// Pre-load message bodies that `fetch_body` will match against by UID.
        pub fn with_bodies(self, bodies: Vec<ImapMessageBody>) -> Self {
            *self.bodies.lock().unwrap() = bodies;
            self
        }

        /// Make every subsequent call return this error.
        pub fn with_error(self, error: ImapError) -> Self {
            *self.should_fail.lock().unwrap() = Some(error);
            self
        }
    }

    /// Helper to clone an `ImapError` for the mock (the real errors are not
    /// `Clone`, so we reconstruct them).
    fn clone_error(err: &ImapError) -> ImapError {
        match err {
            ImapError::ConnectionFailed(msg) => ImapError::ConnectionFailed(msg.clone()),
            ImapError::AuthenticationFailed => ImapError::AuthenticationFailed,
            ImapError::FolderNotFound(name) => ImapError::FolderNotFound(name.clone()),
            ImapError::MessageNotFound { uid, folder } => ImapError::MessageNotFound {
                uid: *uid,
                folder: folder.clone(),
            },
            ImapError::ProtocolError(msg) => ImapError::ProtocolError(msg.clone()),
        }
    }

    #[async_trait]
    impl ImapClient for MockImapClient {
        async fn list_folders(
            &self,
            _creds: &ImapCredentials,
        ) -> Result<Vec<ImapFolder>, ImapError> {
            if let Some(ref err) = *self.should_fail.lock().unwrap() {
                return Err(clone_error(err));
            }
            Ok(self.folders.lock().unwrap().clone())
        }

        async fn folder_status(
            &self,
            _creds: &ImapCredentials,
            _folder: &str,
        ) -> Result<FolderStatus, ImapError> {
            {
                let fail = self.should_fail.lock().unwrap();
                if let Some(ref err) = *fail {
                    return Err(clone_error(err));
                }
            }
            {
                let status = self.folder_status.lock().unwrap();
                if let Some(ref s) = *status {
                    return Ok(s.clone());
                }
            }
            // Derive from headers (separate lock scope).
            let headers = self.headers.lock().unwrap();
            let exists = headers.len() as u32;
            let uid_next = headers.iter().map(|h| h.uid).max().unwrap_or(0) + 1;
            Ok(FolderStatus {
                uid_validity: 1,
                exists,
                uid_next,
            })
        }

        async fn fetch_headers(
            &self,
            _creds: &ImapCredentials,
            _folder: &str,
            _uid_range: &str,
        ) -> Result<Vec<ImapMessageHeader>, ImapError> {
            if let Some(ref err) = *self.should_fail.lock().unwrap() {
                return Err(clone_error(err));
            }
            Ok(self.headers.lock().unwrap().clone())
        }

        async fn fetch_body(
            &self,
            _creds: &ImapCredentials,
            _folder: &str,
            uid: u32,
        ) -> Result<ImapMessageBody, ImapError> {
            if let Some(ref err) = *self.should_fail.lock().unwrap() {
                return Err(clone_error(err));
            }
            let bodies = self.bodies.lock().unwrap();
            bodies
                .iter()
                .find(|b| b.uid == uid)
                .cloned()
                .ok_or_else(|| ImapError::MessageNotFound {
                    uid,
                    folder: _folder.to_string(),
                })
        }

        async fn add_flags(
            &self,
            _creds: &ImapCredentials,
            _folder: &str,
            _uid: u32,
            _flags: &[&str],
        ) -> Result<(), ImapError> {
            if let Some(ref err) = *self.should_fail.lock().unwrap() {
                return Err(clone_error(err));
            }
            Ok(())
        }

        async fn remove_flags(
            &self,
            _creds: &ImapCredentials,
            _folder: &str,
            _uid: u32,
            _flags: &[&str],
        ) -> Result<(), ImapError> {
            if let Some(ref err) = *self.should_fail.lock().unwrap() {
                return Err(clone_error(err));
            }
            Ok(())
        }

        async fn set_flags(
            &self,
            _creds: &ImapCredentials,
            _folder: &str,
            _uid: u32,
            _flags: &[&str],
        ) -> Result<(), ImapError> {
            if let Some(ref err) = *self.should_fail.lock().unwrap() {
                return Err(clone_error(err));
            }
            Ok(())
        }

        async fn move_message(
            &self,
            _creds: &ImapCredentials,
            _from_folder: &str,
            _uid: u32,
            _to_folder: &str,
        ) -> Result<(), ImapError> {
            if let Some(ref err) = *self.should_fail.lock().unwrap() {
                return Err(clone_error(err));
            }
            Ok(())
        }

        async fn expunge_message(
            &self,
            _creds: &ImapCredentials,
            _folder: &str,
            _uid: u32,
        ) -> Result<(), ImapError> {
            if let Some(ref err) = *self.should_fail.lock().unwrap() {
                return Err(clone_error(err));
            }
            Ok(())
        }

        async fn append_message(
            &self,
            _creds: &ImapCredentials,
            folder: &str,
            message_bytes: &[u8],
            _flags: &[&str],
        ) -> Result<(), ImapError> {
            if let Some(ref err) = *self.should_fail.lock().unwrap() {
                return Err(clone_error(err));
            }
            self.appended
                .lock()
                .unwrap()
                .push((folder.to_string(), message_bytes.to_vec()));
            Ok(())
        }

        async fn create_folder(
            &self,
            _creds: &ImapCredentials,
            _folder_name: &str,
        ) -> Result<(), ImapError> {
            if let Some(ref err) = *self.should_fail.lock().unwrap() {
                return Err(clone_error(err));
            }
            Ok(())
        }

        async fn rename_folder(
            &self,
            _creds: &ImapCredentials,
            _from: &str,
            _to: &str,
        ) -> Result<(), ImapError> {
            if let Some(ref err) = *self.should_fail.lock().unwrap() {
                return Err(clone_error(err));
            }
            Ok(())
        }

        async fn delete_folder(
            &self,
            _creds: &ImapCredentials,
            _folder_name: &str,
        ) -> Result<(), ImapError> {
            if let Some(ref err) = *self.should_fail.lock().unwrap() {
                return Err(clone_error(err));
            }
            Ok(())
        }

        async fn subscribe_folder(
            &self,
            _creds: &ImapCredentials,
            _folder_name: &str,
            _subscribe: bool,
        ) -> Result<(), ImapError> {
            if let Some(ref err) = *self.should_fail.lock().unwrap() {
                return Err(clone_error(err));
            }
            Ok(())
        }
    }

    // -----------------------------------------------------------------------
    // Tests
    // -----------------------------------------------------------------------

    #[cfg(test)]
    mod tests {
        use super::*;

        /// Convenience helper to build dummy credentials for tests.
        fn test_creds() -> ImapCredentials {
            ImapCredentials {
                host: "imap.example.com".to_string(),
                port: 993,
                tls: true,
                email: "user@example.com".to_string(),
                password: "hunter2".to_string(),
            }
        }

        #[tokio::test]
        async fn mock_list_folders_returns_preloaded_data() {
            let mock = MockImapClient::new().with_folders(vec![
                ImapFolder {
                    name: "INBOX".to_string(),
                    delimiter: Some("/".to_string()),
                    attributes: vec!["\\HasNoChildren".to_string()],
                },
                ImapFolder {
                    name: "Sent".to_string(),
                    delimiter: Some("/".to_string()),
                    attributes: vec![],
                },
            ]);

            let folders = mock.list_folders(&test_creds()).await.unwrap();
            assert_eq!(folders.len(), 2);
            assert_eq!(folders[0].name, "INBOX");
            assert_eq!(folders[1].name, "Sent");
        }

        #[tokio::test]
        async fn mock_fetch_headers_returns_preloaded_data() {
            let mock = MockImapClient::new().with_headers(vec![ImapMessageHeader {
                uid: 42,
                subject: Some("Hello".to_string()),
                from: vec![EmailAddress {
                    name: Some("Alice".to_string()),
                    address: "alice@example.com".to_string(),
                }],
                to: vec![EmailAddress {
                    name: None,
                    address: "bob@example.com".to_string(),
                }],
                date: Some("Mon, 1 Jan 2024 00:00:00 +0000".to_string()),
                flags: vec!["\\Seen".to_string()],
                has_attachments: false,
                size: 1024,
                message_id: None,
                in_reply_to: None,
                references: None,
                cc: vec![],
            }]);

            let headers = mock
                .fetch_headers(&test_creds(), "INBOX", "1:*")
                .await
                .unwrap();
            assert_eq!(headers.len(), 1);
            assert_eq!(headers[0].uid, 42);
            assert_eq!(headers[0].subject.as_deref(), Some("Hello"));
        }

        #[tokio::test]
        async fn mock_fetch_body_returns_matching_uid() {
            let mock = MockImapClient::new().with_bodies(vec![
                ImapMessageBody {
                    uid: 1,
                    text_plain: Some("First message".to_string()),
                    text_html: None,
                    attachments: vec![],
                    raw_headers: String::new(),
                },
                ImapMessageBody {
                    uid: 2,
                    text_plain: None,
                    text_html: Some("<p>Second</p>".to_string()),
                    attachments: vec![ImapAttachment {
                        filename: Some("doc.pdf".to_string()),
                        content_type: "application/pdf".to_string(),
                        size: 1024,
                        data: vec![0u8; 1024],
                        content_id: None,
                    }],
                    raw_headers: String::new(),
                },
            ]);

            let body = mock.fetch_body(&test_creds(), "INBOX", 2).await.unwrap();
            assert_eq!(body.uid, 2);
            assert!(body.text_html.is_some());
            assert_eq!(body.attachments.len(), 1);
            assert_eq!(body.attachments[0].filename.as_deref(), Some("doc.pdf"));
        }

        #[tokio::test]
        async fn mock_fetch_body_returns_not_found_for_missing_uid() {
            let mock = MockImapClient::new().with_bodies(vec![ImapMessageBody {
                uid: 1,
                text_plain: Some("only message".to_string()),
                text_html: None,
                attachments: vec![],
                raw_headers: String::new(),
            }]);

            let err = mock
                .fetch_body(&test_creds(), "INBOX", 999)
                .await
                .unwrap_err();
            match err {
                ImapError::MessageNotFound { uid, folder } => {
                    assert_eq!(uid, 999);
                    assert_eq!(folder, "INBOX");
                }
                other => panic!("Expected MessageNotFound, got: {other}"),
            }
        }

        #[tokio::test]
        async fn mock_with_error_overrides_all_methods() {
            let mock = MockImapClient::new()
                .with_folders(vec![ImapFolder {
                    name: "INBOX".to_string(),
                    delimiter: None,
                    attributes: vec![],
                }])
                .with_error(ImapError::AuthenticationFailed);

            let err = mock.list_folders(&test_creds()).await.unwrap_err();
            assert!(matches!(err, ImapError::AuthenticationFailed));

            let err = mock
                .fetch_headers(&test_creds(), "INBOX", "1:*")
                .await
                .unwrap_err();
            assert!(matches!(err, ImapError::AuthenticationFailed));

            let err = mock
                .set_flags(&test_creds(), "INBOX", 1, &["\\Seen"])
                .await
                .unwrap_err();
            assert!(matches!(err, ImapError::AuthenticationFailed));

            let err = mock
                .move_message(&test_creds(), "INBOX", 1, "Trash")
                .await
                .unwrap_err();
            assert!(matches!(err, ImapError::AuthenticationFailed));

            let err = mock
                .expunge_message(&test_creds(), "INBOX", 1)
                .await
                .unwrap_err();
            assert!(matches!(err, ImapError::AuthenticationFailed));
        }

        #[tokio::test]
        async fn mock_set_flags_succeeds_without_error() {
            let mock = MockImapClient::new();
            let result = mock
                .set_flags(&test_creds(), "INBOX", 1, &["\\Seen", "\\Flagged"])
                .await;
            assert!(result.is_ok());
        }

        #[tokio::test]
        async fn mock_move_message_succeeds_without_error() {
            let mock = MockImapClient::new();
            let result = mock
                .move_message(&test_creds(), "INBOX", 1, "Archive")
                .await;
            assert!(result.is_ok());
        }

        #[tokio::test]
        async fn mock_expunge_message_succeeds_without_error() {
            let mock = MockImapClient::new();
            let result = mock.expunge_message(&test_creds(), "Trash", 5).await;
            assert!(result.is_ok());
        }

        #[tokio::test]
        async fn real_imap_client_connection_fails_with_bad_host() {
            let client = RealImapClient;
            let creds = test_creds();

            let err = client.list_folders(&creds).await.unwrap_err();
            // With a fake host the connection should fail.
            assert!(
                matches!(err, ImapError::ConnectionFailed(_)),
                "Expected ConnectionFailed, got: {err}"
            );
        }

        #[tokio::test]
        async fn imap_error_display_formats_correctly() {
            let cases: Vec<(ImapError, &str)> = vec![
                (
                    ImapError::ConnectionFailed("timeout".to_string()),
                    "Connection failed: timeout",
                ),
                (ImapError::AuthenticationFailed, "Authentication failed"),
                (
                    ImapError::FolderNotFound("Drafts".to_string()),
                    "Folder not found: Drafts",
                ),
                (
                    ImapError::MessageNotFound {
                        uid: 7,
                        folder: "INBOX".to_string(),
                    },
                    "Message UID 7 not found in folder INBOX",
                ),
                (
                    ImapError::ProtocolError("unexpected EOF".to_string()),
                    "Protocol error: unexpected EOF",
                ),
            ];

            for (err, expected) in cases {
                assert_eq!(err.to_string(), expected);
            }
        }

        #[tokio::test]
        async fn email_address_serializes_and_deserializes() {
            let addr = EmailAddress {
                name: Some("Test User".to_string()),
                address: "test@example.com".to_string(),
            };

            let json = serde_json::to_string(&addr).unwrap();
            let deserialized: EmailAddress = serde_json::from_str(&json).unwrap();
            assert_eq!(addr, deserialized);
        }

        #[tokio::test]
        async fn mock_append_message_succeeds() {
            let mock = MockImapClient::new();
            let result = mock
                .append_message(&test_creds(), "Sent", b"From: test\r\n\r\nBody", &["\\Seen"])
                .await;
            assert!(result.is_ok());
        }

        #[tokio::test]
        async fn mock_append_captures_data() {
            let mock = MockImapClient::new();
            mock.append_message(&test_creds(), "Sent", b"test message", &["\\Seen"])
                .await
                .unwrap();
            let appended = mock.appended.lock().unwrap();
            assert_eq!(appended.len(), 1);
            assert_eq!(appended[0].0, "Sent");
            assert_eq!(appended[0].1, b"test message");
        }

        #[tokio::test]
        async fn mock_empty_returns_empty_collections() {
            let mock = MockImapClient::new();
            let creds = test_creds();

            let folders = mock.list_folders(&creds).await.unwrap();
            assert!(folders.is_empty());

            let headers = mock.fetch_headers(&creds, "INBOX", "1:*").await.unwrap();
            assert!(headers.is_empty());
        }

        #[tokio::test]
        async fn mock_create_folder_succeeds() {
            let mock = MockImapClient::new();
            let result = mock.create_folder(&test_creds(), "NewFolder").await;
            assert!(result.is_ok());
        }

        #[tokio::test]
        async fn mock_rename_folder_succeeds() {
            let mock = MockImapClient::new();
            let result = mock
                .rename_folder(&test_creds(), "OldName", "NewName")
                .await;
            assert!(result.is_ok());
        }

        #[tokio::test]
        async fn mock_delete_folder_succeeds() {
            let mock = MockImapClient::new();
            let result = mock.delete_folder(&test_creds(), "OldFolder").await;
            assert!(result.is_ok());
        }

        #[tokio::test]
        async fn mock_subscribe_folder_succeeds() {
            let mock = MockImapClient::new();
            let result = mock
                .subscribe_folder(&test_creds(), "SomeFolder", true)
                .await;
            assert!(result.is_ok());

            let result = mock
                .subscribe_folder(&test_creds(), "SomeFolder", false)
                .await;
            assert!(result.is_ok());
        }

        // -------------------------------------------------------------------
        // Integration tests (run manually against a real IMAP server)
        // -------------------------------------------------------------------
        //
        //   cargo test --manifest-path backend/Cargo.toml real_imap -- --ignored
        //
        // Required env vars:
        //   TEST_IMAP_HOST     (e.g. "imap.gmail.com")
        //   TEST_IMAP_PORT     (e.g. "993")
        //   TEST_IMAP_EMAIL    (e.g. "you@gmail.com")
        //   TEST_IMAP_PASSWORD (e.g. "app-password")
        //   TEST_IMAP_TLS      (e.g. "true")

        fn real_creds() -> Option<ImapCredentials> {
            let host = std::env::var("TEST_IMAP_HOST").ok()?;
            let port: u16 = std::env::var("TEST_IMAP_PORT")
                .ok()?
                .parse()
                .ok()?;
            let email = std::env::var("TEST_IMAP_EMAIL").ok()?;
            let password = std::env::var("TEST_IMAP_PASSWORD").ok()?;
            let tls = std::env::var("TEST_IMAP_TLS")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true);
            Some(ImapCredentials {
                host,
                port,
                tls,
                email,
                password,
            })
        }

        #[tokio::test]
        #[ignore] // Run manually: cargo test real_imap_list_folders -- --ignored
        async fn real_imap_list_folders() {
            let creds = real_creds().expect("TEST_IMAP_* env vars required");
            let client = RealImapClient;
            let folders = client.list_folders(&creds).await.unwrap();
            assert!(!folders.is_empty(), "expected at least one folder");
            let names: Vec<_> = folders.iter().map(|f| f.name.as_str()).collect();
            assert!(
                names.iter().any(|n| n.eq_ignore_ascii_case("INBOX")),
                "expected INBOX in folder list, got: {names:?}"
            );
        }

        #[tokio::test]
        #[ignore] // Run manually: cargo test real_imap_fetch_headers -- --ignored
        async fn real_imap_fetch_headers() {
            let creds = real_creds().expect("TEST_IMAP_* env vars required");
            let client = RealImapClient;
            let headers = client
                .fetch_headers(&creds, "INBOX", "1:5")
                .await
                .unwrap();
            // The mailbox might be empty, so we just check it doesn't error.
            for h in &headers {
                assert!(h.uid > 0);
            }
        }

        #[tokio::test]
        #[ignore] // Run manually: cargo test real_imap_fetch_body -- --ignored
        async fn real_imap_fetch_body() {
            let creds = real_creds().expect("TEST_IMAP_* env vars required");
            let client = RealImapClient;

            // First fetch headers to find a UID.
            let headers = client
                .fetch_headers(&creds, "INBOX", "1:1")
                .await
                .unwrap();
            if let Some(h) = headers.first() {
                let body = client.fetch_body(&creds, "INBOX", h.uid).await.unwrap();
                assert_eq!(body.uid, h.uid);
            }
        }
    }
}
