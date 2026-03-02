use async_trait::async_trait;
use futures::StreamExt;

use super::connection::map_imap_error;
use super::parse::{
    decode_rfc2047, flag_to_string, has_attachments, imap_address_to_email, name_attribute_to_string,
};

pub use super::error::ImapError;
pub use super::types::{
    EmailAddress, FolderStatus, FolderStatusExtended, ImapAttachment, ImapCredentials, ImapFolder,
    ImapMessageBody, ImapMessageHeader,
};

pub(crate) use super::connection::connect;

#[cfg(test)]
#[path = "mock.rs"]
pub mod mock;

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

    /// Fetch only UIDs and FLAGS for all messages in a folder.
    /// Used for periodic reconciliation to detect flag changes and deletions.
    async fn fetch_uids_and_flags(
        &self,
        creds: &ImapCredentials,
        folder: &str,
    ) -> Result<Vec<(u32, Vec<String>)>, ImapError>;

    /// Lightweight STATUS command to get folder metadata without SELECT.
    /// Returns UIDVALIDITY, EXISTS, UIDNEXT, UNSEEN, and HIGHESTMODSEQ.
    async fn folder_status_extended(
        &self,
        creds: &ImapCredentials,
        folder: &str,
    ) -> Result<FolderStatusExtended, ImapError>;

    /// Fetch only messages whose flags changed since `since_modseq` using CONDSTORE.
    /// Returns (vec of (uid, flags), new_highest_modseq).
    async fn fetch_changed_flags(
        &self,
        creds: &ImapCredentials,
        folder: &str,
        since_modseq: u64,
    ) -> Result<(Vec<(u32, Vec<String>)>, u64), ImapError>;
}

// ---------------------------------------------------------------------------
// Real implementation backed by async-imap
// ---------------------------------------------------------------------------

/// Production IMAP client that uses `async-imap` and `mail-parser`.
///
/// This is a stateless unit struct — every method creates a fresh connection,
/// performs the operation, and disconnects.
pub struct RealImapClient;


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

    async fn fetch_uids_and_flags(
        &self,
        creds: &ImapCredentials,
        folder: &str,
    ) -> Result<Vec<(u32, Vec<String>)>, ImapError> {
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

        let results = {
            let mut fetch_stream = session
                .uid_fetch("1:*", "(UID FLAGS)")
                .await
                .map_err(map_imap_error)?;

            let mut items = Vec::new();
            while let Some(result) = fetch_stream.next().await {
                let fetch = result.map_err(map_imap_error)?;
                if let Some(uid) = fetch.uid {
                    let flags: Vec<String> = fetch.flags().map(|f| flag_to_string(&f)).collect();
                    items.push((uid, flags));
                }
            }
            items
        };

        let _ = session.logout().await;
        Ok(results)
    }

    async fn folder_status_extended(
        &self,
        creds: &ImapCredentials,
        folder: &str,
    ) -> Result<FolderStatusExtended, ImapError> {
        let mut session = connect(creds).await?;

        let mailbox = session
            .status(folder, "(MESSAGES UIDNEXT UIDVALIDITY UNSEEN HIGHESTMODSEQ)")
            .await
            .map_err(map_imap_error)?;

        let result = FolderStatusExtended {
            uid_validity: mailbox.uid_validity.unwrap_or(0),
            exists: mailbox.exists,
            uid_next: mailbox.uid_next.unwrap_or(0),
            unseen: mailbox.unseen.unwrap_or(0),
            highest_modseq: mailbox.highest_modseq.unwrap_or(0),
        };

        let _ = session.logout().await;
        Ok(result)
    }

    async fn fetch_changed_flags(
        &self,
        creds: &ImapCredentials,
        folder: &str,
        since_modseq: u64,
    ) -> Result<(Vec<(u32, Vec<String>)>, u64), ImapError> {
        let mut session = connect(creds).await?;

        let mailbox = session
            .select_condstore(folder)
            .await
            .map_err(map_imap_error)?;

        let new_modseq = mailbox.highest_modseq.unwrap_or(0);

        let items = {
            let mut fetch_stream = session
                .uid_fetch("1:*", format!("(UID FLAGS) (CHANGEDSINCE {})", since_modseq))
                .await
                .map_err(map_imap_error)?;

            let mut items = Vec::new();
            while let Some(result) = fetch_stream.next().await {
                let fetch = result.map_err(map_imap_error)?;
                if let Some(uid) = fetch.uid {
                    let flags: Vec<String> = fetch.flags().map(|f| flag_to_string(&f)).collect();
                    items.push((uid, flags));
                }
            }
            items
        };

        let _ = session.logout().await;
        Ok((items, new_modseq))
    }
}
