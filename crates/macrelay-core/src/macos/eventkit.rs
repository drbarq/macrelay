use anyhow::Result;

use super::escape::escape_applescript_string;

/// List all calendars using AppleScript.
pub async fn list_calendars() -> Result<Vec<CalendarInfo>> {
    let script = r#"
        tell application "Calendar"
            set output to ""
            repeat with c in calendars
                set output to output & name of c & "|" & writable of c & linefeed
            end repeat
            return output
        end tell
    "#;

    let output = crate::macos::applescript::run_applescript(script)?;
    let mut calendars = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split('|').collect();
        let title = parts.first().unwrap_or(&"").to_string();
        let writable = parts.get(1).map(|s| *s == "true").unwrap_or(false);
        calendars.push(CalendarInfo {
            title,
            calendar_type: "local".to_string(),
            allows_modify: writable,
        });
    }
    Ok(calendars)
}

/// Search calendar events using AppleScript.
pub async fn search_events_applescript(
    days_ahead: u32,
    query: Option<&str>,
) -> Result<Vec<EventInfo>> {
    let script = format!(
        r#"
        set now to current date
        set endDate to now + ({days_ahead} * days)
        tell application "Calendar"
            set output to ""
            repeat with c in calendars
                try
                    set evts to (every event of c whose start date >= now and start date <= endDate)
                    repeat with e in evts
                        set evtTitle to summary of e
                        set evtStart to start date of e as string
                        set evtEnd to end date of e as string
                        set evtLoc to ""
                        try
                            set evtLoc to location of e
                        end try
                        set evtAllDay to allday event of e
                        set output to output & evtTitle & "||" & evtStart & "||" & evtEnd & "||" & evtLoc & "||" & (name of c) & "||" & evtAllDay & linefeed
                    end repeat
                end try
            end repeat
            return output
        end tell
        "#
    );

    let output = crate::macos::applescript::run_applescript(&script)?;
    let mut events = Vec::new();

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split("||").collect();
        let title = parts.first().unwrap_or(&"").to_string();

        if let Some(q) = query
            && !title.to_lowercase().contains(&q.to_lowercase())
        {
            continue;
        }

        events.push(EventInfo {
            title,
            start_date: parts.get(1).unwrap_or(&"").to_string(),
            end_date: parts.get(2).unwrap_or(&"").to_string(),
            is_all_day: parts.get(5).map(|s| *s == "true").unwrap_or(false),
            location: parts
                .get(3)
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty()),
            notes: None,
            calendar: parts.get(4).unwrap_or(&"").to_string(),
        });
    }
    Ok(events)
}

/// Create a calendar event using AppleScript.
pub async fn create_event(
    title: &str,
    _start_date: &str,
    _end_date: &str,
    is_all_day: bool,
    location: &str,
    notes: &str,
) -> Result<String> {
    let allday_str = if is_all_day { "true" } else { "false" };
    let escaped_title = escape_applescript_string(title);
    let escaped_location = escape_applescript_string(location);
    let escaped_notes = escape_applescript_string(notes);

    // For now, use descriptive date strings. Later we'll add Unix timestamp conversion.
    let script = format!(
        r#"
        tell application "Calendar"
            tell calendar 1
                set newEvent to make new event with properties {{summary:"{escaped_title}", start date:(current date), end date:((current date) + 3600), location:"{escaped_location}", description:"{escaped_notes}", allday event:{allday_str}}}
                return "Event created: {escaped_title}"
            end tell
        end tell
        "#
    );

    crate::macos::applescript::run_applescript(&script)
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CalendarInfo {
    pub title: String,
    pub calendar_type: String,
    pub allows_modify: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct EventInfo {
    pub title: String,
    pub start_date: String,
    pub end_date: String,
    pub is_all_day: bool,
    pub location: Option<String>,
    pub notes: Option<String>,
    pub calendar: String,
}
