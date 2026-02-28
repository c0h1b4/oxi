use super::types::EmailAddress;

// ---- Helper: convert NameAttribute to string ------------------------------

pub(crate) fn name_attribute_to_string(attr: &async_imap::types::NameAttribute<'_>) -> String {
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

pub(crate) fn flag_to_string(flag: &async_imap::types::Flag<'_>) -> String {
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
pub(crate) fn decode_rfc2047(input: &str) -> String {
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
pub(crate) fn decode_quoted_printable_header(input: &str) -> Option<Vec<u8>> {
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
pub(crate) fn decode_charset(charset: &str, bytes: &[u8]) -> String {
    match charset.to_ascii_lowercase().as_str() {
        "utf-8" | "utf8" => String::from_utf8_lossy(bytes).into_owned(),
        "iso-8859-1" | "latin1" | "latin-1" => bytes.iter().map(|&b| b as char).collect(),
        // For other charsets, try UTF-8 first, then fall back to Latin-1.
        _ => String::from_utf8(bytes.to_vec())
            .unwrap_or_else(|_| bytes.iter().map(|&b| b as char).collect()),
    }
}

// ---- Helper: convert imap_proto::Address to EmailAddress ------------------

pub(crate) fn imap_address_to_email(addr: &async_imap::imap_proto::types::Address<'_>) -> EmailAddress {
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

pub(crate) fn has_attachments(bs: &async_imap::imap_proto::types::BodyStructure<'_>) -> bool {
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
