use std::sync::Arc;

use rmcp::model::Tool;
use serde_json::json;

use crate::macos::escape::escape_applescript_string;
use crate::macos::eventkit;
use crate::permissions::{PermissionManager, PermissionType};
use crate::registry::{ServiceRegistry, ToolHandler, error_result, schema_from_json, text_result};

/// Register all calendar tools with the service registry.
pub fn register(registry: &mut ServiceRegistry) {
    registry.register(
        "pim_calendar_list_calendars",
        Tool::new(
            "pim_calendar_list_calendars",
            "[READ] List all calendars available on this Mac.",
            schema_from_json(json!({
                "type": "object",
                "properties": {},
            })),
        ),
        handler_list_calendars(),
    );

    registry.register(
        "pim_calendar_search_events",
        Tool::new(
            "pim_calendar_search_events",
            "[READ] Search calendar events within a date range. Returns matching events with title, time, location, and notes.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "start_date": {
                        "type": "string",
                        "description": "Start of search range as Unix timestamp (seconds). Defaults to now. Past timestamps are accepted for historical queries."
                    },
                    "end_date": {
                        "type": "string",
                        "description": "End of search range as Unix timestamp (seconds). If only start_date is given, defaults to start + 7 days; otherwise defaults to 7 days from now."
                    },
                    "query": {
                        "type": "string",
                        "description": "Optional text to filter events by title (case-insensitive substring)."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of events to return. Default 50."
                    }
                }
            })),
        ),
        handler_search_events(),
    );

    registry.register(
        "pim_calendar_create_event",
        Tool::new(
            "pim_calendar_create_event",
            "[CREATE] Create a new calendar event.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "title": {
                        "type": "string",
                        "description": "Event title."
                    },
                    "start_date": {
                        "type": "string",
                        "description": "Event start as Unix timestamp (seconds)."
                    },
                    "end_date": {
                        "type": "string",
                        "description": "Event end as Unix timestamp (seconds)."
                    },
                    "is_all_day": {
                        "type": "boolean",
                        "description": "Whether this is an all-day event. Default false."
                    },
                    "location": {
                        "type": "string",
                        "description": "Event location."
                    },
                    "notes": {
                        "type": "string",
                        "description": "Event notes."
                    }
                },
                "required": ["title", "start_date", "end_date"]
            })),
        ),
        handler_create_event(),
    );

    registry.register(
        "pim_calendar_reschedule_event",
        Tool::new(
            "pim_calendar_reschedule_event",
            "[UPDATE] Find a calendar event by title and change its start and end dates.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "title": {
                        "type": "string",
                        "description": "Title of the event to reschedule."
                    },
                    "new_start_date": {
                        "type": "string",
                        "description": "New start date as Unix timestamp (seconds)."
                    },
                    "new_end_date": {
                        "type": "string",
                        "description": "New end date as Unix timestamp (seconds)."
                    }
                },
                "required": ["title", "new_start_date", "new_end_date"]
            })),
        ),
        handler_reschedule_event(),
    );

    registry.register(
        "pim_calendar_cancel_event",
        Tool::new(
            "pim_calendar_cancel_event",
            "[DELETE] Delete a calendar event by title.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "title": {
                        "type": "string",
                        "description": "Title of the event to delete."
                    }
                },
                "required": ["title"]
            })),
        ),
        handler_cancel_event(),
    );

    registry.register(
        "pim_calendar_update_event",
        Tool::new(
            "pim_calendar_update_event",
            "[UPDATE] Update properties of a calendar event found by title. Can change the title, location, and notes.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "title": {
                        "type": "string",
                        "description": "Current title of the event to update."
                    },
                    "new_title": {
                        "type": "string",
                        "description": "New title for the event."
                    },
                    "new_location": {
                        "type": "string",
                        "description": "New location for the event."
                    },
                    "new_notes": {
                        "type": "string",
                        "description": "New notes for the event."
                    }
                },
                "required": ["title"]
            })),
        ),
        handler_update_event(),
    );

    registry.register(
        "pim_calendar_open_event",
        Tool::new(
            "pim_calendar_open_event",
            "[READ] Open Calendar.app and navigate to the date of an event found by title.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "title": {
                        "type": "string",
                        "description": "Title of the event to open in Calendar.app."
                    }
                },
                "required": ["title"]
            })),
        ),
        handler_open_event(),
    );

    registry.register(
        "pim_calendar_find_available_times",
        Tool::new(
            "pim_calendar_find_available_times",
            "[READ] Find free time slots within a date range by checking existing calendar events. Returns available blocks of time.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "start_date": {
                        "type": "string",
                        "description": "Start of search range as Unix timestamp (seconds)."
                    },
                    "end_date": {
                        "type": "string",
                        "description": "End of search range as Unix timestamp (seconds)."
                    },
                    "min_duration_minutes": {
                        "type": "integer",
                        "description": "Minimum duration in minutes for a free slot. Default 30."
                    },
                    "working_hours_only": {
                        "type": "boolean",
                        "description": "Only return slots within working hours (9 AM - 5 PM). Default true."
                    }
                },
                "required": ["start_date", "end_date"]
            })),
        ),
        handler_find_available_times(),
    );
}

fn handler_list_calendars() -> ToolHandler {
    Arc::new(|_args| {
        Box::pin(async move {
            if let Err(msg) = PermissionManager::require(PermissionType::Calendar) {
                return Ok(error_result(msg));
            }
            match eventkit::list_calendars().await {
                Ok(calendars) => {
                    let json = serde_json::to_string_pretty(&calendars)?;
                    Ok(text_result(json))
                }
                Err(e) => Ok(error_result(format!("Failed to list calendars: {e}"))),
            }
        })
    })
}

fn handler_search_events() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            if let Err(msg) = PermissionManager::require(PermissionType::Calendar) {
                return Ok(error_result(msg));
            }
            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;
            let query_filter = args.get("query").and_then(|v| v.as_str());

            let now_ts = current_unix_secs();
            let (start_ts, end_ts) = match parse_search_window(&args, now_ts) {
                Ok(window) => window,
                Err(msg) => return Ok(error_result(msg)),
            };

            match eventkit::search_events_in_range(start_ts, end_ts, query_filter).await {
                Ok(mut events) => {
                    events.truncate(limit);
                    if events.is_empty() {
                        Ok(text_result("No events found in the specified date range."))
                    } else {
                        let json = serde_json::to_string_pretty(&events)?;
                        Ok(text_result(format!(
                            "Found {} event(s):\n\n{json}",
                            events.len()
                        )))
                    }
                }
                Err(e) => Ok(error_result(format!("Failed to search events: {e}"))),
            }
        })
    })
}

/// Wallclock-now as Unix epoch seconds. Extracted so `parse_search_window`
/// can be deterministic in tests.
fn current_unix_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Parse `start_date`/`end_date` from the search_events args.
///
/// Defaults:
/// - both omitted -> `(now, now + 7d)`
/// - only `start_date` -> `(start, start + 7d)`
/// - only `end_date`   -> `(now, end)`
/// - both present      -> `(start, end)`
///
/// Returns `Err(message)` if either present field can't be parsed as a
/// Unix timestamp. Validation of `start <= end` is intentionally not
/// enforced here — EventKit's predicate accepts inverted ranges and just
/// returns zero events, which is the right behaviour ("you asked for an
/// empty window, you got an empty window") and matches what the previous
/// AppleScript path did.
fn parse_search_window(
    args: &std::collections::HashMap<String, serde_json::Value>,
    now_ts: i64,
) -> Result<(i64, i64), String> {
    const DEFAULT_WINDOW_SECS: i64 = 7 * 86400;

    let parse_ts = |key: &str| -> Result<Option<i64>, String> {
        match args.get(key).and_then(|v| v.as_str()) {
            None => Ok(None),
            Some(s) => s
                .parse::<i64>()
                .map(Some)
                .map_err(|_| format!("{key} must be a Unix timestamp (integer seconds)")),
        }
    };

    let start_opt = parse_ts("start_date")?;
    let end_opt = parse_ts("end_date")?;

    let (start_ts, end_ts) = match (start_opt, end_opt) {
        (None, None) => (now_ts, now_ts + DEFAULT_WINDOW_SECS),
        (Some(s), None) => (s, s + DEFAULT_WINDOW_SECS),
        (None, Some(e)) => (now_ts, e),
        (Some(s), Some(e)) => (s, e),
    };

    Ok((start_ts, end_ts))
}

fn handler_create_event() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            if let Err(msg) = PermissionManager::require(PermissionType::Calendar) {
                return Ok(error_result(msg));
            }
            let title = match args.get("title").and_then(|v| v.as_str()) {
                Some(t) => t,
                None => return Ok(error_result("title is required")),
            };
            let start_ts: i64 = match args.get("start_date").and_then(|v| v.as_str()) {
                Some(s) => match s.parse() {
                    Ok(n) => n,
                    Err(_) => {
                        return Ok(error_result(
                            "start_date must be a Unix timestamp (integer)",
                        ));
                    }
                },
                None => return Ok(error_result("start_date is required")),
            };
            let end_ts: i64 = match args.get("end_date").and_then(|v| v.as_str()) {
                Some(e) => match e.parse() {
                    Ok(n) => n,
                    Err(_) => {
                        return Ok(error_result("end_date must be a Unix timestamp (integer)"));
                    }
                },
                None => return Ok(error_result("end_date is required")),
            };

            let is_all_day = args
                .get("is_all_day")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let location = args.get("location").and_then(|v| v.as_str()).unwrap_or("");
            let notes = args.get("notes").and_then(|v| v.as_str()).unwrap_or("");

            let escaped_title = escape_applescript_string(title);
            let escaped_location = escape_applescript_string(location);
            let escaped_notes = escape_applescript_string(notes);

            // Build AppleScript for event creation
            // Dates come as Unix timestamps, convert to AppleScript date
            let allday_str = if is_all_day { "true" } else { "false" };

            let script = format!(
                r#"
                set startEpoch to {start_ts} as number
                set endEpoch to {end_ts} as number
                set startDate to current date
                set time of startDate to 0
                set startDate to startDate - (startDate - (date "Thursday, January 1, 1970 at 12:00:00 AM")) + startEpoch
                set endDate to current date
                set time of endDate to 0
                set endDate to endDate - (endDate - (date "Thursday, January 1, 1970 at 12:00:00 AM")) + endEpoch

                tell application "Calendar"
                    tell calendar 1
                        set newEvent to make new event with properties {{summary:"{escaped_title}", start date:startDate, end date:endDate, location:"{escaped_location}", description:"{escaped_notes}", allday event:{allday_str}}}
                    end tell
                end tell
                return "Event created: {escaped_title}"
                "#
            );

            match crate::macos::applescript::run_applescript(&script) {
                Ok(result) => Ok(text_result(result)),
                Err(e) => Ok(error_result(format!("Failed to create event: {e}"))),
            }
        })
    })
}

fn handler_reschedule_event() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            if let Err(msg) = PermissionManager::require(PermissionType::Calendar) {
                return Ok(error_result(msg));
            }
            let title = match args.get("title").and_then(|v| v.as_str()) {
                Some(t) => t,
                None => return Ok(error_result("title is required")),
            };

            let new_start_ts: i64 = match args.get("new_start_date").and_then(|v| v.as_str()) {
                Some(s) => match s.parse() {
                    Ok(n) => n,
                    Err(_) => {
                        return Ok(error_result(
                            "new_start_date must be a Unix timestamp (integer)",
                        ));
                    }
                },
                None => return Ok(error_result("new_start_date is required")),
            };

            let new_end_ts: i64 = match args.get("new_end_date").and_then(|v| v.as_str()) {
                Some(e) => match e.parse() {
                    Ok(n) => n,
                    Err(_) => {
                        return Ok(error_result(
                            "new_end_date must be a Unix timestamp (integer)",
                        ));
                    }
                },
                None => return Ok(error_result("new_end_date is required")),
            };

            let escaped_title = escape_applescript_string(title);

            let script = format!(
                r#"
                set newStartEpoch to {new_start_ts} as number
                set newEndEpoch to {new_end_ts} as number
                set newStartDate to current date
                set time of newStartDate to 0
                set newStartDate to newStartDate - (newStartDate - (date "Thursday, January 1, 1970 at 12:00:00 AM")) + newStartEpoch
                set newEndDate to current date
                set time of newEndDate to 0
                set newEndDate to newEndDate - (newEndDate - (date "Thursday, January 1, 1970 at 12:00:00 AM")) + newEndEpoch

                set searchTitle to "{escaped_title}"
                set eventFound to false

                tell application "Calendar"
                    repeat with cal in calendars
                        set allEvents to (every event of cal whose summary is searchTitle)
                        repeat with evt in allEvents
                            set start date of evt to newStartDate
                            set end date of evt to newEndDate
                            set eventFound to true
                        end repeat
                    end repeat
                end tell

                if eventFound then
                    return "Event rescheduled: " & searchTitle
                else
                    return "No event found with title: " & searchTitle
                end if
                "#
            );

            match crate::macos::applescript::run_applescript(&script) {
                Ok(result) => Ok(text_result(result)),
                Err(e) => Ok(error_result(format!("Failed to reschedule event: {e}"))),
            }
        })
    })
}

fn handler_cancel_event() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            if let Err(msg) = PermissionManager::require(PermissionType::Calendar) {
                return Ok(error_result(msg));
            }
            let title = match args.get("title").and_then(|v| v.as_str()) {
                Some(t) => t,
                None => return Ok(error_result("title is required")),
            };

            let escaped_title = escape_applescript_string(title);

            let script = format!(
                r#"
                set searchTitle to "{escaped_title}"
                set eventFound to false

                tell application "Calendar"
                    repeat with cal in calendars
                        set allEvents to (every event of cal whose summary is searchTitle)
                        repeat with evt in allEvents
                            delete evt
                            set eventFound to true
                        end repeat
                    end repeat
                end tell

                if eventFound then
                    return "Event deleted: " & searchTitle
                else
                    return "No event found with title: " & searchTitle
                end if
                "#
            );

            match crate::macos::applescript::run_applescript(&script) {
                Ok(result) => Ok(text_result(result)),
                Err(e) => Ok(error_result(format!("Failed to cancel event: {e}"))),
            }
        })
    })
}

fn handler_update_event() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            if let Err(msg) = PermissionManager::require(PermissionType::Calendar) {
                return Ok(error_result(msg));
            }
            let title = match args.get("title").and_then(|v| v.as_str()) {
                Some(t) => t,
                None => return Ok(error_result("title is required")),
            };

            let new_title = args.get("new_title").and_then(|v| v.as_str());
            let new_location = args.get("new_location").and_then(|v| v.as_str());
            let new_notes = args.get("new_notes").and_then(|v| v.as_str());

            if new_title.is_none() && new_location.is_none() && new_notes.is_none() {
                return Ok(error_result(
                    "At least one of new_title, new_location, or new_notes must be provided.",
                ));
            }

            let escaped_title = escape_applescript_string(title);

            // Build the property update lines dynamically
            let mut update_lines = Vec::new();
            if let Some(t) = new_title {
                let escaped = escape_applescript_string(t);
                update_lines.push(format!(r#"set summary of evt to "{escaped}""#));
            }
            if let Some(loc) = new_location {
                let escaped = escape_applescript_string(loc);
                update_lines.push(format!(r#"set location of evt to "{escaped}""#));
            }
            if let Some(notes) = new_notes {
                let escaped = escape_applescript_string(notes);
                update_lines.push(format!(r#"set description of evt to "{escaped}""#));
            }
            let updates_block = update_lines.join("\n                            ");

            let script = format!(
                r#"
                set searchTitle to "{escaped_title}"
                set eventFound to false

                tell application "Calendar"
                    repeat with cal in calendars
                        set allEvents to (every event of cal whose summary is searchTitle)
                        repeat with evt in allEvents
                            {updates_block}
                            set eventFound to true
                        end repeat
                    end repeat
                end tell

                if eventFound then
                    return "Event updated: " & searchTitle
                else
                    return "No event found with title: " & searchTitle
                end if
                "#
            );

            match crate::macos::applescript::run_applescript(&script) {
                Ok(result) => Ok(text_result(result)),
                Err(e) => Ok(error_result(format!("Failed to update event: {e}"))),
            }
        })
    })
}

fn handler_open_event() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            if let Err(msg) = PermissionManager::require(PermissionType::Calendar) {
                return Ok(error_result(msg));
            }
            let title = match args.get("title").and_then(|v| v.as_str()) {
                Some(t) => t,
                None => return Ok(error_result("title is required")),
            };

            let escaped_title = escape_applescript_string(title);

            let script = format!(
                r#"
                set searchTitle to "{escaped_title}"
                set eventDate to missing value

                tell application "Calendar"
                    repeat with cal in calendars
                        set allEvents to (every event of cal whose summary is searchTitle)
                        repeat with evt in allEvents
                            set eventDate to start date of evt
                            exit repeat
                        end repeat
                        if eventDate is not missing value then exit repeat
                    end repeat
                end tell

                if eventDate is missing value then
                    return "No event found with title: " & searchTitle
                end if

                tell application "Calendar"
                    activate
                    switch view to day view
                    view calendar at eventDate
                end tell

                return "Opened Calendar.app at event: " & searchTitle
                "#
            );

            match crate::macos::applescript::run_applescript(&script) {
                Ok(result) => Ok(text_result(result)),
                Err(e) => Ok(error_result(format!("Failed to open event: {e}"))),
            }
        })
    })
}

fn handler_find_available_times() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            if let Err(msg) = PermissionManager::require(PermissionType::Calendar) {
                return Ok(error_result(msg));
            }
            let start_ts: i64 = match args.get("start_date").and_then(|v| v.as_str()) {
                Some(s) => match s.parse() {
                    Ok(n) => n,
                    Err(_) => {
                        return Ok(error_result(
                            "start_date must be a Unix timestamp (integer)",
                        ));
                    }
                },
                None => return Ok(error_result("start_date is required")),
            };

            let end_ts: i64 = match args.get("end_date").and_then(|v| v.as_str()) {
                Some(e) => match e.parse() {
                    Ok(n) => n,
                    Err(_) => {
                        return Ok(error_result("end_date must be a Unix timestamp (integer)"));
                    }
                },
                None => return Ok(error_result("end_date is required")),
            };

            let min_duration_minutes = args
                .get("min_duration_minutes")
                .and_then(|v| v.as_u64())
                .unwrap_or(30);

            let working_hours_only = args
                .get("working_hours_only")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);

            let work_start_hour = if working_hours_only { 9 } else { 0 };
            let work_end_hour = if working_hours_only { 17 } else { 24 };

            // Fetch busy intervals via EventKit. The previous AppleScript
            // implementation iterated `every event of cal whose start date
            // >= ...` across every calendar, which is an O(N) linear scan
            // per calendar (~147s/calendar measured) — see the same note
            // on `search_events_in_range` for the full story. EventKit
            // talks to Calendar's indexed SQLite store directly: same data,
            // milliseconds instead of minutes.
            let busy = match eventkit::fetch_busy_intervals(start_ts, end_ts).await {
                Ok(b) => b,
                Err(e) => return Ok(error_result(format!("Failed to find available times: {e}"))),
            };

            let range_start: i64 = start_ts;
            let range_end: i64 = end_ts;
            let min_duration_secs = (min_duration_minutes * 60) as i64;

            let free_slots = compute_free_slots(
                busy,
                range_start,
                range_end,
                min_duration_secs,
                working_hours_only,
                work_start_hour,
                work_end_hour,
            );

            if free_slots.is_empty() {
                Ok(text_result(
                    "No available time slots found in the specified range.",
                ))
            } else {
                let json = serde_json::to_string_pretty(&free_slots)?;
                Ok(text_result(format!(
                    "Found {} available time slot(s):\n\n{json}",
                    free_slots.len()
                )))
            }
        })
    })
}

/// Compute free slots from a list of busy intervals. Pure function,
/// extracted so the working-hours and merge logic is unit-testable
/// without going through EventKit. Returns JSON values in the same
/// shape the handler emits.
fn compute_free_slots(
    mut busy: Vec<(i64, i64)>,
    range_start: i64,
    range_end: i64,
    min_duration_secs: i64,
    working_hours_only: bool,
    work_start_hour: i64,
    work_end_hour: i64,
) -> Vec<serde_json::Value> {
    busy.sort_by_key(|&(s, _)| s);

    // Merge overlapping intervals
    let mut merged: Vec<(i64, i64)> = Vec::new();
    for (s, e) in &busy {
        if let Some(last) = merged.last_mut()
            && *s <= last.1
        {
            last.1 = last.1.max(*e);
            continue;
        }
        merged.push((*s, *e));
    }

    // Helper: clamp an epoch into the working-hours band. `is_end=false`
    // means "this is a slot start, push forward to next valid time";
    // `is_end=true` means "this is a slot end, pull back to last valid
    // time in the same day."
    let clamp_to_working = |epoch: i64, is_end: bool| -> i64 {
        if !working_hours_only {
            return epoch;
        }
        let secs_in_day = epoch.rem_euclid(86400);
        let hour = secs_in_day / 3600;
        let day_start = epoch - secs_in_day;
        if hour < work_start_hour {
            day_start + work_start_hour * 3600
        } else if hour >= work_end_hour {
            if is_end {
                day_start + work_end_hour * 3600
            } else {
                // Push to next day's work start
                day_start + 86400 + work_start_hour * 3600
            }
        } else {
            epoch
        }
    };

    let mut free_slots: Vec<serde_json::Value> = Vec::new();
    let mut cursor = range_start;

    for (busy_start, busy_end) in &merged {
        let slot_start = clamp_to_working(cursor, false);
        let slot_end = clamp_to_working(*busy_start, true);
        if slot_end - slot_start >= min_duration_secs && slot_start < slot_end {
            free_slots.push(json!({
                "start": slot_start,
                "end": slot_end,
                "duration_minutes": (slot_end - slot_start) / 60
            }));
        }
        cursor = *busy_end;
    }

    // Final slot from last busy end to range end
    let slot_start = clamp_to_working(cursor, false);
    let slot_end = clamp_to_working(range_end, true);
    if slot_end - slot_start >= min_duration_secs && slot_start < slot_end {
        free_slots.push(json!({
            "start": slot_start,
            "end": slot_end,
            "duration_minutes": (slot_end - slot_start) / 60
        }));
    }

    free_slots
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::macos::applescript::{MOCK_RUNNER, ScriptRunner};
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::time::Duration;

    struct AssertingMock {
        expected_fragments: Vec<&'static str>,
        response: String,
    }

    impl ScriptRunner for AssertingMock {
        fn run_applescript(&self, script: &str) -> anyhow::Result<String> {
            for fragment in &self.expected_fragments {
                assert!(
                    script.contains(fragment),
                    "Script missing fragment: {}\nScript content:\n{}",
                    fragment,
                    script
                );
            }
            Ok(self.response.clone())
        }
        fn run_applescript_with_timeout(
            &self,
            script: &str,
            _timeout: Duration,
        ) -> anyhow::Result<String> {
            self.run_applescript(script)
        }
        fn run_jxa(&self, _script: &str) -> anyhow::Result<String> {
            unimplemented!()
        }
    }

    #[test]
    fn test_tool_schemas_valid() {
        let mut registry = ServiceRegistry::new();
        register(&mut registry);
        let tools = registry.list_tools();
        assert_eq!(tools.len(), 8, "Expected exactly 8 calendar tools");

        let names: Vec<_> = tools.iter().map(|t| t.name.as_ref()).collect();
        assert!(names.contains(&"pim_calendar_list_calendars"));
        assert!(names.contains(&"pim_calendar_search_events"));
        assert!(names.contains(&"pim_calendar_create_event"));
        assert!(names.contains(&"pim_calendar_reschedule_event"));
        assert!(names.contains(&"pim_calendar_cancel_event"));
        assert!(names.contains(&"pim_calendar_update_event"));
        assert!(names.contains(&"pim_calendar_open_event"));
        assert!(names.contains(&"pim_calendar_find_available_times"));
    }

    #[tokio::test]
    async fn test_mock_list_calendars() {
        let mock = Arc::new(AssertingMock {
            expected_fragments: vec![
                "tell application \"Calendar\"",
                "name of c & \"|\" & writable of c",
            ],
            response: "Home|true\nWork|false\n".to_string(),
        });

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_list_calendars();
                let args = HashMap::new();
                let result = handler(args).await.expect("Handler should not fail");

                assert_eq!(result.is_error, Some(false));

                let content_text = result
                    .content
                    .first()
                    .and_then(|c| c.as_text())
                    .map(|t| t.text.as_str())
                    .expect("Expected text content");

                let calendars: Vec<serde_json::Value> =
                    serde_json::from_str(content_text).expect("Expected valid JSON array");

                assert_eq!(calendars.len(), 2);
                assert_eq!(calendars[0]["title"], "Home");
                assert_eq!(calendars[0]["allows_modify"], true);
                assert_eq!(calendars[1]["title"], "Work");
                assert_eq!(calendars[1]["allows_modify"], false);
            })
            .await;
    }

    /// Tier 3 (integration) — hits the real EventKit framework against
    /// the user's Calendar database. Run locally with
    /// `cargo test -p macrelay-core --lib -- --include-ignored
    ///   test_search_events_eventkit_smoke`.
    ///
    /// This replaced the prior `test_mock_search_events`: that one used
    /// the AppleScript MOCK_RUNNER to inject a synthetic event response,
    /// which made sense when the search path went through osascript. The
    /// new path goes through `EKEventStore::eventsMatchingPredicate`, so
    /// there's no script to intercept. Faking EventKit at the objc2 layer
    /// would require a trait abstraction over the framework — not worth
    /// the indirection for what is effectively a thin wrapper around an
    /// indexed Apple API. Verifying it against the real store is faster
    /// and catches more (e.g. predicate construction errors that would
    /// raise an Objective-C exception).
    #[tokio::test]
    #[ignore] // Requires Calendar permission + a real EKEventStore — local only
    async fn test_search_events_eventkit_smoke() {
        let handler = handler_search_events();
        let mut args = HashMap::new();
        args.insert("limit".to_string(), json!(50));

        let result = handler(args).await.expect("Handler should not fail");

        // Either we have permission and got events (or zero), or we
        // surfaced a permission error. Both are acceptable here — the
        // important guarantee is "doesn't hang, doesn't panic, returns
        // a structured response in well under a second on any library".
        let content = result.content[0].as_text().unwrap().text.as_str();
        assert!(
            content.starts_with("Found ")
                || content.contains("No events found")
                || content.to_lowercase().contains("permission")
                || content.to_lowercase().contains("not authorized"),
            "unexpected content: {content}"
        );
    }

    #[tokio::test]
    async fn test_mock_create_event() {
        let mock = Arc::new(AssertingMock {
            expected_fragments: vec![
                "tell application \"Calendar\"",
                "make new event with properties",
                "summary:\"New Meeting\"",
                "location:\"Room 101\"",
                "description:\"Important notes\"",
            ],
            response: "Event created: New Meeting".to_string(),
        });

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_create_event();
                let mut args = HashMap::new();
                args.insert("title".to_string(), json!("New Meeting"));
                args.insert("start_date".to_string(), json!("1712761335"));
                args.insert("end_date".to_string(), json!("1712764935"));
                args.insert("location".to_string(), json!("Room 101"));
                args.insert("notes".to_string(), json!("Important notes"));

                let result = handler(args).await.expect("Handler should not fail");
                assert_eq!(result.is_error, Some(false));

                let content = result.content[0].as_text().unwrap().text.as_str();
                assert_eq!(content, "Event created: New Meeting");
            })
            .await;
    }

    #[tokio::test]
    async fn test_mock_create_event_escaping() {
        let mock = Arc::new(AssertingMock {
            expected_fragments: vec!["summary:\"Meeting with \\\"quotes\\\" and \\\\backslash\""],
            response: "Event created: Meeting with \"quotes\" and \\backslash".to_string(),
        });

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_create_event();
                let mut args = HashMap::new();
                args.insert(
                    "title".to_string(),
                    json!("Meeting with \"quotes\" and \\backslash"),
                );
                args.insert("start_date".to_string(), json!("1712761335"));
                args.insert("end_date".to_string(), json!("1712764935"));

                let result = handler(args).await.expect("Handler should not fail");
                assert_eq!(result.is_error, Some(false));
            })
            .await;
    }

    #[tokio::test]
    async fn test_mock_reschedule_event() {
        let mock = Arc::new(AssertingMock {
            expected_fragments: vec![
                "set searchTitle to \"Old Meeting\"",
                "set start date of evt to newStartDate",
                "set end date of evt to newEndDate",
            ],
            response: "Event rescheduled: Old Meeting".to_string(),
        });

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_reschedule_event();
                let mut args = HashMap::new();
                args.insert("title".to_string(), json!("Old Meeting"));
                args.insert("new_start_date".to_string(), json!("1712761335"));
                args.insert("new_end_date".to_string(), json!("1712764935"));

                let result = handler(args).await.expect("Handler should not fail");
                assert_eq!(result.is_error, Some(false));

                let content = result.content[0].as_text().unwrap().text.as_str();
                assert_eq!(content, "Event rescheduled: Old Meeting");
            })
            .await;
    }

    #[tokio::test]
    async fn test_mock_cancel_event() {
        let mock = Arc::new(AssertingMock {
            expected_fragments: vec!["set searchTitle to \"Cancel Me\"", "delete evt"],
            response: "Event deleted: Cancel Me".to_string(),
        });

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_cancel_event();
                let mut args = HashMap::new();
                args.insert("title".to_string(), json!("Cancel Me"));

                let result = handler(args).await.expect("Handler should not fail");
                assert_eq!(result.is_error, Some(false));

                let content = result.content[0].as_text().unwrap().text.as_str();
                assert_eq!(content, "Event deleted: Cancel Me");
            })
            .await;
    }

    #[tokio::test]
    async fn test_mock_update_event() {
        let mock = Arc::new(AssertingMock {
            expected_fragments: vec![
                "set searchTitle to \"Update Me\"",
                "set summary of evt to \"Updated Title\"",
                "set location of evt to \"Updated Location\"",
            ],
            response: "Event updated: Update Me".to_string(),
        });

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_update_event();
                let mut args = HashMap::new();
                args.insert("title".to_string(), json!("Update Me"));
                args.insert("new_title".to_string(), json!("Updated Title"));
                args.insert("new_location".to_string(), json!("Updated Location"));

                let result = handler(args).await.expect("Handler should not fail");
                assert_eq!(result.is_error, Some(false));

                let content = result.content[0].as_text().unwrap().text.as_str();
                assert_eq!(content, "Event updated: Update Me");
            })
            .await;
    }

    #[tokio::test]
    async fn test_mock_open_event() {
        let mock = Arc::new(AssertingMock {
            expected_fragments: vec![
                "set searchTitle to \"Open Me\"",
                "view calendar at eventDate",
            ],
            response: "Opened Calendar.app at event: Open Me".to_string(),
        });

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_open_event();
                let mut args = HashMap::new();
                args.insert("title".to_string(), json!("Open Me"));

                let result = handler(args).await.expect("Handler should not fail");
                assert_eq!(result.is_error, Some(false));

                let content = result.content[0].as_text().unwrap().text.as_str();
                assert_eq!(content, "Opened Calendar.app at event: Open Me");
            })
            .await;
    }

    /// Tier 1 — exercises the pure free-slot calculator without going
    /// through EventKit. This replaced the prior Tier 2 mock test that
    /// injected an AppleScript response: after the EventKit migration
    /// `find_available_times` no longer issues any AppleScript, so
    /// MOCK_RUNNER is never consulted and the asserted-fragment approach
    /// can't fire. The free-slot math is the part actually worth
    /// testing — `fetch_busy_intervals` is verified separately by the
    /// Tier 3 smoke below.
    #[test]
    fn test_compute_free_slots_with_working_hours() {
        // Range: Apr 10 08:00 UTC to Apr 10 20:00 UTC
        // Busy:  Apr 10 10:00 UTC to Apr 10 11:00 UTC
        // Work hours: 09:00 to 17:00
        // Expected:
        //   Slot 1: 09:00 → 10:00 (60 min)
        //   Slot 2: 11:00 → 17:00 (360 min)
        let busy = vec![(1712743200, 1712746800)];
        let slots = compute_free_slots(busy, 1712736000, 1712779200, 30 * 60, true, 9, 17);

        assert_eq!(slots.len(), 2);
        assert_eq!(slots[0]["start"], 1712739600);
        assert_eq!(slots[0]["end"], 1712743200);
        assert_eq!(slots[1]["start"], 1712746800);
        assert_eq!(slots[1]["end"], 1712768400);
    }

    #[test]
    fn test_compute_free_slots_no_busy() {
        // Empty calendar → one big free slot covering the working window.
        let slots = compute_free_slots(vec![], 1712736000, 1712779200, 30 * 60, true, 9, 17);
        assert_eq!(slots.len(), 1);
        assert_eq!(slots[0]["start"], 1712739600);
        assert_eq!(slots[0]["end"], 1712768400);
    }

    #[test]
    fn test_compute_free_slots_skips_below_min_duration() {
        // Two back-to-back busy blocks with a 15-min gap; min_duration=30
        // should drop the gap.
        let busy = vec![(1712743200, 1712746800), (1712747700, 1712750400)];
        let slots = compute_free_slots(busy, 1712736000, 1712779200, 30 * 60, false, 0, 24);
        // Expected slots:
        //   range_start (08:00) → first busy (10:00) = 120 min ✓
        //   gap between (11:00 → 11:15) = 15 min ✗ (filtered)
        //   second busy end (12:00) → range_end (20:00) = 480 min ✓
        assert_eq!(slots.len(), 2);
    }

    /// Tier 1 — `parse_search_window` defaulting matrix.
    #[test]
    fn test_parse_search_window_defaults() {
        let now = 1_700_000_000_i64;
        let week = 7 * 86400;

        // Both omitted -> (now, now + 7d)
        let args = HashMap::new();
        assert_eq!(parse_search_window(&args, now), Ok((now, now + week)));

        // Only start -> (start, start + 7d)
        let mut args = HashMap::new();
        args.insert("start_date".to_string(), json!("1262304000"));
        assert_eq!(
            parse_search_window(&args, now),
            Ok((1262304000, 1262304000 + week))
        );

        // Only end -> (now, end)
        let mut args = HashMap::new();
        args.insert("end_date".to_string(), json!("1293753599"));
        assert_eq!(parse_search_window(&args, now), Ok((now, 1293753599)));

        // Both -> verbatim
        let mut args = HashMap::new();
        args.insert("start_date".to_string(), json!("100"));
        args.insert("end_date".to_string(), json!("200"));
        assert_eq!(parse_search_window(&args, now), Ok((100, 200)));
    }

    #[test]
    fn test_parse_search_window_rejects_garbage() {
        let mut args = HashMap::new();
        args.insert("start_date".to_string(), json!("not-a-number"));
        let err = parse_search_window(&args, 1_700_000_000).expect_err("should reject");
        assert!(err.contains("start_date"));
        assert!(err.contains("Unix timestamp"));
    }

    /// Tier 3 (integration) — replaces the prior AppleScript-mock based
    /// test for `find_available_times`. Hits the real EventKit framework
    /// against the user's Calendar database. Run locally with
    /// `cargo test -p macrelay-core --lib -- --include-ignored
    ///   test_find_available_times_eventkit_smoke`.
    #[tokio::test]
    #[ignore] // Requires Calendar permission + a real EKEventStore — local only
    async fn test_find_available_times_eventkit_smoke() {
        let handler = handler_find_available_times();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let mut args = HashMap::new();
        args.insert("start_date".to_string(), json!(now.to_string()));
        args.insert("end_date".to_string(), json!((now + 86400).to_string()));
        args.insert("working_hours_only".to_string(), json!(false));

        let result = handler(args).await.expect("Handler should not fail");

        let content = result.content[0].as_text().unwrap().text.as_str();
        assert!(
            content.starts_with("Found ")
                || content.contains("No available time slots")
                || content.to_lowercase().contains("permission")
                || content.to_lowercase().contains("not authorized"),
            "unexpected content: {content}"
        );
    }

    /// When osascript fails, the handler must return a graceful error result
    /// (is_error == Some(true) with a human-readable message) instead of
    /// panicking or propagating the raw anyhow error up to the MCP layer.
    #[tokio::test]
    async fn test_create_event_returns_error_result_on_osascript_failure() {
        struct ErrorMock;
        impl ScriptRunner for ErrorMock {
            fn run_applescript(&self, _script: &str) -> anyhow::Result<String> {
                Err(anyhow::anyhow!(
                    "osascript: execution error: Calendar got an error: Not authorized"
                ))
            }
            fn run_applescript_with_timeout(
                &self,
                _script: &str,
                _timeout: Duration,
            ) -> anyhow::Result<String> {
                self.run_applescript(_script)
            }
            fn run_jxa(&self, _script: &str) -> anyhow::Result<String> {
                unimplemented!()
            }
        }

        let mock = Arc::new(ErrorMock);
        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_create_event();
                let mut args = HashMap::new();
                args.insert("title".to_string(), json!("Test"));
                args.insert("start_date".to_string(), json!("1712761335"));
                args.insert("end_date".to_string(), json!("1712764935"));

                let result = handler(args)
                    .await
                    .expect("Handler should not panic on osascript error");
                assert_eq!(
                    result.is_error,
                    Some(true),
                    "Expected is_error=Some(true) when osascript fails"
                );

                let content = result.content[0].as_text().unwrap().text.as_str();
                assert!(
                    content.contains("Failed to create event"),
                    "Expected 'Failed to create event' prefix, got: {}",
                    content
                );
                assert!(
                    content.contains("Not authorized"),
                    "Expected the underlying error to be surfaced, got: {}",
                    content
                );
            })
            .await;
    }

    #[tokio::test]
    async fn test_validation_create_event_requires_title() {
        let handler = handler_create_event();
        let mut args = HashMap::new();
        args.insert("start_date".to_string(), json!("1712761335"));
        args.insert("end_date".to_string(), json!("1712764935"));

        let result = handler(args).await.expect("Handler should not panic");
        assert_eq!(result.is_error, Some(true));
        assert!(
            result.content[0]
                .as_text()
                .unwrap()
                .text
                .contains("title is required")
        );
    }

    #[tokio::test]
    async fn test_validation_cancel_event_requires_title() {
        let handler = handler_cancel_event();
        let args = HashMap::new();

        let result = handler(args).await.expect("Handler should not panic");
        assert_eq!(result.is_error, Some(true));
        assert!(
            result.content[0]
                .as_text()
                .unwrap()
                .text
                .contains("title is required")
        );
    }
}
