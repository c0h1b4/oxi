use ical::parser::ical::component::IcalCalendar;
use ical::IcalParser;
use std::io::BufReader;

use crate::db::calendar::CreateEvent;

/// Parse iCalendar text and return a list of events ready for database insertion.
pub fn parse_ics(ics_text: &str) -> Result<Vec<CreateEvent>, String> {
    let reader = BufReader::new(ics_text.as_bytes());
    let parser = IcalParser::new(reader);

    let mut events = Vec::new();

    for calendar_result in parser {
        let calendar: IcalCalendar =
            calendar_result.map_err(|e| format!("Failed to parse ICS: {e}"))?;

        for vevent in calendar.events {
            let mut title = String::new();
            let mut description = String::new();
            let mut location = String::new();
            let mut start_time = String::new();
            let mut end_time = String::new();
            let mut all_day = false;
            let mut uid = None;
            let mut rrule = None;
            let mut organizer = String::new();
            let mut attendees: Vec<String> = Vec::new();
            let mut url = None;

            for prop in &vevent.properties {
                let value = prop.value.as_deref().unwrap_or("");

                match prop.name.as_str() {
                    "SUMMARY" => {
                        title = value.to_string();
                    }
                    "DESCRIPTION" => {
                        description = value.to_string();
                    }
                    "LOCATION" => {
                        location = value.to_string();
                    }
                    "DTSTART" => {
                        let parsed = parse_ical_datetime(value, &prop.params);
                        all_day = is_date_only(value);
                        start_time = parsed;
                    }
                    "DTEND" => {
                        end_time = parse_ical_datetime(value, &prop.params);
                    }
                    "UID" => {
                        uid = Some(value.to_string());
                    }
                    "RRULE" => {
                        rrule = Some(value.to_string());
                    }
                    "ORGANIZER" => {
                        organizer = extract_email_from_cal_address(value);
                    }
                    "ATTENDEE" => {
                        attendees.push(extract_email_from_cal_address(value));
                    }
                    "URL" => {
                        if !value.is_empty() {
                            url = Some(value.to_string());
                        }
                    }
                    _ => {}
                }
            }

            if title.is_empty() {
                title = "(No title)".to_string();
            }

            // If end_time is empty, default to start_time + 1 hour (or same day for all-day)
            if end_time.is_empty() {
                end_time = start_time.clone();
            }

            if start_time.is_empty() {
                // Skip events without a start time
                continue;
            }

            let attendees_json =
                serde_json::to_string(&attendees).unwrap_or_else(|_| "[]".to_string());

            events.push(CreateEvent {
                title,
                description,
                location,
                start_time,
                end_time,
                all_day,
                recurrence_rule: rrule,
                attendees: attendees_json,
                organizer,
                status: "confirmed".to_string(),
                source: "ics".to_string(),
                source_uid: uid,
                source_message_uid: None,
                source_message_folder: None,
                meeting_url: url,
                color: None,
                reminder_minutes: None,
            });
        }
    }

    Ok(events)
}

/// Check if a DTSTART/DTEND value is date-only (no time component).
fn is_date_only(value: &str) -> bool {
    // Date-only values are 8 characters: YYYYMMDD
    // Date-time values are at least 15: YYYYMMDDTHHmmSS
    value.len() == 8 && value.chars().all(|c| c.is_ascii_digit())
}

/// Parse an iCalendar date/datetime value to ISO 8601 format.
fn parse_ical_datetime(
    value: &str,
    params: &Option<Vec<(String, Vec<String>)>>,
) -> String {
    // Check for TZID parameter
    let _tzid = params.as_ref().and_then(|ps| {
        ps.iter()
            .find(|(k, _)| k == "TZID")
            .and_then(|(_, v)| v.first())
            .cloned()
    });

    let value = value.trim();

    if value.len() == 8 && value.chars().all(|c| c.is_ascii_digit()) {
        // Date only: YYYYMMDD -> YYYY-MM-DD
        return format!(
            "{}-{}-{}",
            &value[0..4],
            &value[4..6],
            &value[6..8]
        );
    }

    if value.len() >= 15 && value.contains('T') {
        // DateTime: YYYYMMDDTHHmmSS or YYYYMMDDTHHmmSSZ
        let date_part = &value[0..8];
        let time_part = &value[9..];

        let year = &date_part[0..4];
        let month = &date_part[4..6];
        let day = &date_part[6..8];

        let hour = &time_part[0..2];
        let min = &time_part[2..4];
        let sec = if time_part.len() >= 6 {
            &time_part[4..6]
        } else {
            "00"
        };

        let is_utc = value.ends_with('Z');
        let suffix = if is_utc { "Z" } else { "" };

        return format!("{year}-{month}-{day}T{hour}:{min}:{sec}{suffix}");
    }

    // Fallback: return as-is
    value.to_string()
}

/// Extract email address from a cal-address value (e.g., "mailto:user@example.com").
fn extract_email_from_cal_address(value: &str) -> String {
    if let Some(stripped) = value.strip_prefix("mailto:") {
        stripped.to_string()
    } else if let Some(stripped) = value.strip_prefix("MAILTO:") {
        stripped.to_string()
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_ICS: &str = r#"BEGIN:VCALENDAR
VERSION:2.0
PRODID:-//Test//Test//EN
BEGIN:VEVENT
DTSTART:20250306T090000Z
DTEND:20250306T100000Z
SUMMARY:Team Meeting
DESCRIPTION:Weekly team sync
LOCATION:Conference Room A
UID:event-123@example.com
ORGANIZER:mailto:boss@example.com
ATTENDEE:mailto:alice@example.com
ATTENDEE:mailto:bob@example.com
URL:https://zoom.us/j/12345
END:VEVENT
END:VCALENDAR"#;

    #[test]
    fn test_parse_basic_event() {
        let events = parse_ics(SAMPLE_ICS).unwrap();
        assert_eq!(events.len(), 1);

        let event = &events[0];
        assert_eq!(event.title, "Team Meeting");
        assert_eq!(event.description, "Weekly team sync");
        assert_eq!(event.location, "Conference Room A");
        assert_eq!(event.start_time, "2025-03-06T09:00:00Z");
        assert_eq!(event.end_time, "2025-03-06T10:00:00Z");
        assert!(!event.all_day);
        assert_eq!(event.organizer, "boss@example.com");
        assert_eq!(
            event.source_uid.as_deref(),
            Some("event-123@example.com")
        );
        assert_eq!(event.meeting_url.as_deref(), Some("https://zoom.us/j/12345"));
        assert_eq!(event.source, "ics");

        let attendees: Vec<String> = serde_json::from_str(&event.attendees).unwrap();
        assert_eq!(attendees.len(), 2);
        assert!(attendees.contains(&"alice@example.com".to_string()));
        assert!(attendees.contains(&"bob@example.com".to_string()));
    }

    #[test]
    fn test_parse_all_day_event() {
        let ics = r#"BEGIN:VCALENDAR
VERSION:2.0
BEGIN:VEVENT
DTSTART;VALUE=DATE:20250306
DTEND;VALUE=DATE:20250307
SUMMARY:All Day Event
UID:allday-1@example.com
END:VEVENT
END:VCALENDAR"#;

        let events = parse_ics(ics).unwrap();
        assert_eq!(events.len(), 1);
        assert!(events[0].all_day);
        assert_eq!(events[0].start_time, "2025-03-06");
        assert_eq!(events[0].end_time, "2025-03-07");
    }

    #[test]
    fn test_parse_multiple_events() {
        let ics = r#"BEGIN:VCALENDAR
VERSION:2.0
BEGIN:VEVENT
DTSTART:20250306T090000Z
DTEND:20250306T100000Z
SUMMARY:Event 1
UID:e1@example.com
END:VEVENT
BEGIN:VEVENT
DTSTART:20250306T140000Z
DTEND:20250306T150000Z
SUMMARY:Event 2
UID:e2@example.com
END:VEVENT
END:VCALENDAR"#;

        let events = parse_ics(ics).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].title, "Event 1");
        assert_eq!(events[1].title, "Event 2");
    }

    #[test]
    fn test_parse_ical_datetime_utc() {
        assert_eq!(
            parse_ical_datetime("20250306T090000Z", &None),
            "2025-03-06T09:00:00Z"
        );
    }

    #[test]
    fn test_parse_ical_datetime_local() {
        assert_eq!(
            parse_ical_datetime("20250306T090000", &None),
            "2025-03-06T09:00:00"
        );
    }

    #[test]
    fn test_parse_ical_date_only() {
        assert_eq!(parse_ical_datetime("20250306", &None), "2025-03-06");
    }

    #[test]
    fn test_extract_email() {
        assert_eq!(
            extract_email_from_cal_address("mailto:user@example.com"),
            "user@example.com"
        );
        assert_eq!(
            extract_email_from_cal_address("MAILTO:user@example.com"),
            "user@example.com"
        );
        assert_eq!(
            extract_email_from_cal_address("user@example.com"),
            "user@example.com"
        );
    }
}
