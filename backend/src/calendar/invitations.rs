//! Outgoing iTIP requests for organizer-owned calendar events.

use rusqlite::Connection;

use crate::db::calendar::CalendarEvent;
use crate::smtp::client::{AttachmentData, SendableMessage};

pub fn ensure_organizer(event: &CalendarEvent, email: &str) -> Result<(), String> {
    if event.source != "manual"
        || (!event.organizer.trim().is_empty()
            && !event.organizer.trim().eq_ignore_ascii_case(email))
    {
        return Err("Only the organizer of a manually created event can send invitations".into());
    }
    Ok(())
}

pub fn recipients(attendees: &str, organizer: &str) -> Result<Vec<String>, String> {
    let attendees: Vec<String> = serde_json::from_str(attendees)
        .map_err(|_| "Attendees must be a JSON array of email addresses".to_string())?;
    let mut recipients: Vec<String> = Vec::new();
    for attendee in attendees {
        let email = attendee.trim();
        email
            .parse::<lettre::Address>()
            .map_err(|_| format!("Invalid attendee email address: {email}"))?;
        if !email.eq_ignore_ascii_case(organizer)
            && !recipients.iter().any(|r| r.eq_ignore_ascii_case(email))
        {
            recipients.push(email.to_string());
        }
    }
    Ok(recipients)
}

pub fn build_invitation(
    conn: &Connection,
    event: &CalendarEvent,
    organizer: &str,
) -> Result<SendableMessage, String> {
    ensure_organizer(event, organizer)?;
    organizer
        .parse::<lettre::Address>()
        .map_err(|_| "Invalid organizer email address")?;
    let to = recipients(&event.attendees, organizer)?;
    let stamp: String = conn
        .query_row("SELECT strftime('%Y%m%dT%H%M%SZ', 'now')", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    let start = event_datetime(conn, &event.start_time, event.all_day, false)?;
    let end = event_datetime(conn, &event.end_time, event.all_day, true)?;
    let date_parameter = if event.all_day { ";VALUE=DATE" } else { "" };
    let mut lines = vec![
        "BEGIN:VCALENDAR".to_string(),
        "VERSION:2.0".to_string(),
        "PRODID:-//Oxi//Calendar//EN".to_string(),
        "METHOD:REQUEST".to_string(),
        "BEGIN:VEVENT".to_string(),
        format!("UID:{}@oxi.email", event.id),
        format!("SEQUENCE:{}", event.invitation_sequence),
        format!("DTSTAMP:{stamp}"),
        format!("DTSTART{date_parameter}:{start}"),
        format!("DTEND{date_parameter}:{end}"),
        format!("SUMMARY:{}", escape_text(&event.title)),
        format!("DESCRIPTION:{}", escape_text(&event.description)),
        format!("LOCATION:{}", escape_text(&event.location)),
        format!("ORGANIZER:mailto:{organizer}"),
        format!(
            "STATUS:{}",
            match event.status.as_str() {
                "cancelled" => "CANCELLED",
                "tentative" => "TENTATIVE",
                _ => "CONFIRMED",
            }
        ),
    ];
    for email in &to {
        lines.push(format!(
            "ATTENDEE;RSVP=TRUE;PARTSTAT=NEEDS-ACTION:mailto:{email}"
        ));
    }
    if let Some(rule) = &event.recurrence_rule {
        if rule.contains(['\r', '\n']) {
            return Err("Invalid recurrence rule".into());
        }
        lines.push(format!("RRULE:{rule}"));
    }
    if let Some(url) = &event.meeting_url {
        lines.push(format!("URL:{}", escape_text(url)));
    }
    lines.extend(["END:VEVENT".to_string(), "END:VCALENDAR".to_string()]);
    let calendar = lines
        .iter()
        .map(|line| fold_line(line))
        .collect::<Vec<_>>()
        .join("\r\n")
        + "\r\n";
    let mut text = format!(
        "{}\n{} – {}\n{}\n\n{}",
        event.title, event.start_time, event.end_time, event.location, event.description
    );
    if let Some(url) = &event.meeting_url {
        text.push_str(&format!("\n\n{url}"));
    }
    Ok(SendableMessage {
        from: organizer.to_string(),
        to,
        cc: vec![],
        bcc: vec![],
        subject: format!("Invitation: {}", event.title),
        text_body: text,
        html_body: None,
        in_reply_to: None,
        references: None,
        attachments: vec![AttachmentData {
            filename: "invite.ics".to_string(),
            content_type: "text/calendar; charset=utf-8; method=REQUEST".to_string(),
            data: calendar.into_bytes(),
            content_id: None,
        }],
    })
}

fn event_datetime(
    conn: &Connection,
    value: &str,
    all_day: bool,
    end: bool,
) -> Result<String, String> {
    // The event form stores all-day end times as 23:59:59; iCalendar uses an exclusive date.
    let modifier = if all_day && end && value.contains("T23:59:59") {
        "+1 day"
    } else {
        "+0 days"
    };
    let format = if all_day { "%Y%m%d" } else { "%Y%m%dT%H%M%S" };
    let parsed: Option<String> = conn
        .query_row(
            "SELECT strftime(?1, ?2, ?3)",
            [format, value, modifier],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let mut parsed = parsed.ok_or_else(|| format!("Invalid event date: {value}"))?;
    // Keep floating local date-times floating; explicit offsets are normalized to UTC by SQLite.
    if !all_day && (value.ends_with('Z') || value.get(10..).is_some_and(|t| t.contains(['+', '-'])))
    {
        parsed.push('Z');
    }
    Ok(parsed)
}

fn escape_text(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\n', "\\n")
        .replace(';', "\\;")
        .replace(',', "\\,")
}

fn fold_line(line: &str) -> String {
    let mut folded = String::new();
    let mut width = 0;
    for c in line.chars() {
        if width + c.len_utf8() > 75 {
            folded.push_str("\r\n ");
            width = 1;
        }
        folded.push(c);
        width += c.len_utf8();
    }
    folded
}
