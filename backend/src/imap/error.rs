use std::fmt;

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
