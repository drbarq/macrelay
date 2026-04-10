use std::sync::Arc;

use rmcp::model::Tool;
use serde_json::json;

use crate::macos::eventkit;
use crate::registry::{error_result, schema_from_json, text_result, ServiceRegistry, ToolHandler};

/// Register all calendar tools with the service registry.
pub fn register(registry: &mut ServiceRegistry) {
    registry.register(
        "calendar_list_calendars",
        Tool::new(
            "calendar_list_calendars",
            "List all calendars available on this Mac.",
            schema_from_json(json!({
                "type": "object",
                "properties": {},
            })),
        ),
        handler_list_calendars(),
    );

    registry.register(
        "calendar_search_events",
        Tool::new(
            "calendar_search_events",
            "Search calendar events within a date range. Returns matching events with title, time, location, and notes.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "start_date": {
                        "type": "string",
                        "description": "Start of search range as Unix timestamp (seconds). Defaults to now."
                    },
                    "end_date": {
                        "type": "string",
                        "description": "End of search range as Unix timestamp (seconds). Defaults to 7 days from now."
                    },
                    "query": {
                        "type": "string",
                        "description": "Optional text to filter events by title."
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
        "calendar_create_event",
        Tool::new(
            "calendar_create_event",
            "Create a new calendar event.",
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
        "calendar_reschedule_event",
        Tool::new(
            "calendar_reschedule_event",
            "Find a calendar event by title and change its start and end dates.",
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
        "calendar_cancel_event",
        Tool::new(
            "calendar_cancel_event",
            "Delete a calendar event by title.",
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
        "calendar_update_event",
        Tool::new(
            "calendar_update_event",
            "Update properties of a calendar event found by title. Can change the title, location, and notes.",
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
        "calendar_open_event",
        Tool::new(
            "calendar_open_event",
            "Open Calendar.app and navigate to the date of an event found by title.",
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
        "calendar_find_available_times",
        Tool::new(
            "calendar_find_available_times",
            "Find free time slots within a date range by checking existing calendar events. Returns available blocks of time.",
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
            let days_ahead = 7u32; // Default 7 days
            let limit = args
                .get("limit")
                .and_then(|v| v.as_u64())
                .unwrap_or(50) as usize;

            let query_filter = args
                .get("query")
                .and_then(|v| v.as_str());

            match eventkit::search_events_applescript(days_ahead, query_filter).await {
                Ok(mut events) => {
                    events.truncate(limit);
                    if events.is_empty() {
                        Ok(text_result("No events found in the specified date range."))
                    } else {
                        let json = serde_json::to_string_pretty(&events)?;
                        Ok(text_result(format!("Found {} event(s):\n\n{json}", events.len())))
                    }
                }
                Err(e) => Ok(error_result(format!("Failed to search events: {e}"))),
            }
        })
    })
}

fn handler_create_event() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let title = args
                .get("title")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("title is required"))?;

            let start_str = args
                .get("start_date")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("start_date is required"))?;

            let end_str = args
                .get("end_date")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("end_date is required"))?;

            let is_all_day = args
                .get("is_all_day")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let location = args.get("location").and_then(|v| v.as_str()).unwrap_or("");
            let notes = args.get("notes").and_then(|v| v.as_str()).unwrap_or("");

            // Build AppleScript for event creation
            // Dates come as Unix timestamps, convert to AppleScript date
            let allday_str = if is_all_day { "true" } else { "false" };

            let script = format!(
                r#"
                set startEpoch to {start_str} as number
                set endEpoch to {end_str} as number
                set startDate to current date
                set time of startDate to 0
                set startDate to startDate - (startDate - (date "Thursday, January 1, 1970 at 12:00:00 AM")) + startEpoch
                set endDate to current date
                set time of endDate to 0
                set endDate to endDate - (endDate - (date "Thursday, January 1, 1970 at 12:00:00 AM")) + endEpoch

                tell application "Calendar"
                    tell calendar 1
                        set newEvent to make new event with properties {{summary:"{title}", start date:startDate, end date:endDate, location:"{location}", description:"{notes}", allday event:{allday_str}}}
                    end tell
                end tell
                return "Event created: {title}"
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
            let title = args
                .get("title")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("title is required"))?;

            let new_start_str = args
                .get("new_start_date")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("new_start_date is required"))?;

            let new_end_str = args
                .get("new_end_date")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("new_end_date is required"))?;

            let escaped_title = title.replace('"', "\\\"");

            let script = format!(
                r#"
                set newStartEpoch to {new_start_str} as number
                set newEndEpoch to {new_end_str} as number
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
            let title = args
                .get("title")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("title is required"))?;

            let escaped_title = title.replace('"', "\\\"");

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
            let title = args
                .get("title")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("title is required"))?;

            let new_title = args.get("new_title").and_then(|v| v.as_str());
            let new_location = args.get("new_location").and_then(|v| v.as_str());
            let new_notes = args.get("new_notes").and_then(|v| v.as_str());

            if new_title.is_none() && new_location.is_none() && new_notes.is_none() {
                return Ok(error_result(
                    "At least one of new_title, new_location, or new_notes must be provided.",
                ));
            }

            let escaped_title = title.replace('"', "\\\"");

            // Build the property update lines dynamically
            let mut update_lines = Vec::new();
            if let Some(t) = new_title {
                let escaped = t.replace('"', "\\\"");
                update_lines.push(format!(r#"set summary of evt to "{escaped}""#));
            }
            if let Some(loc) = new_location {
                let escaped = loc.replace('"', "\\\"");
                update_lines.push(format!(r#"set location of evt to "{escaped}""#));
            }
            if let Some(notes) = new_notes {
                let escaped = notes.replace('"', "\\\"");
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
            let title = args
                .get("title")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("title is required"))?;

            let escaped_title = title.replace('"', "\\\"");

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
            let start_str = args
                .get("start_date")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("start_date is required"))?;

            let end_str = args
                .get("end_date")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("end_date is required"))?;

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

            // AppleScript fetches all events in the range; we process free slots from the output
            let script = format!(
                r#"
                set rangeStartEpoch to {start_str} as number
                set rangeEndEpoch to {end_str} as number

                set rangeStart to current date
                set time of rangeStart to 0
                set rangeStart to rangeStart - (rangeStart - (date "Thursday, January 1, 1970 at 12:00:00 AM")) + rangeStartEpoch

                set rangeEnd to current date
                set time of rangeEnd to 0
                set rangeEnd to rangeEnd - (rangeEnd - (date "Thursday, January 1, 1970 at 12:00:00 AM")) + rangeEndEpoch

                set busyTimes to {{}}

                tell application "Calendar"
                    repeat with cal in calendars
                        set evts to (every event of cal whose start date >= rangeStart and start date <= rangeEnd)
                        repeat with evt in evts
                            set evtStart to start date of evt
                            set evtEnd to end date of evt
                            -- Convert to epoch seconds
                            set epochRef to date "Thursday, January 1, 1970 at 12:00:00 AM"
                            set startSec to (evtStart - epochRef)
                            set endSec to (evtEnd - epochRef)
                            set end of busyTimes to (startSec as text) & "," & (endSec as text)
                        end repeat
                    end repeat
                end tell

                set output to ""
                repeat with i from 1 to count of busyTimes
                    if i > 1 then set output to output & "|"
                    set output to output & (item i of busyTimes)
                end repeat

                return output
                "#
            );

            match crate::macos::applescript::run_applescript(&script) {
                Ok(raw_output) => {
                    let range_start: i64 = start_str.parse().unwrap_or(0);
                    let range_end: i64 = end_str.parse().unwrap_or(0);
                    let min_duration_secs = (min_duration_minutes * 60) as i64;

                    // Parse busy intervals from the AppleScript output
                    let mut busy: Vec<(i64, i64)> = Vec::new();
                    if !raw_output.trim().is_empty() {
                        for pair in raw_output.trim().split('|') {
                            let parts: Vec<&str> = pair.split(',').collect();
                            if parts.len() == 2 {
                                if let (Ok(s), Ok(e)) =
                                    (parts[0].trim().parse::<i64>(), parts[1].trim().parse::<i64>())
                                {
                                    busy.push((s, e));
                                }
                            }
                        }
                    }

                    // Sort by start time
                    busy.sort_by_key(|&(s, _)| s);

                    // Merge overlapping intervals
                    let mut merged: Vec<(i64, i64)> = Vec::new();
                    for (s, e) in &busy {
                        if let Some(last) = merged.last_mut() {
                            if *s <= last.1 {
                                last.1 = last.1.max(*e);
                                continue;
                            }
                        }
                        merged.push((*s, *e));
                    }

                    // Find free slots between busy intervals
                    let mut free_slots: Vec<serde_json::Value> = Vec::new();
                    let mut cursor = range_start;

                    // Helper closure to clamp to working hours
                    let clamp_to_working = |epoch: i64| -> i64 {
                        if !working_hours_only {
                            return epoch;
                        }
                        // Determine the hour of day for this epoch
                        let secs_in_day = ((epoch % 86400) + 86400) % 86400;
                        let hour = secs_in_day / 3600;
                        let day_start = epoch - secs_in_day;
                        if hour < work_start_hour {
                            day_start + work_start_hour * 3600
                        } else if hour >= work_end_hour {
                            // Push to next day's work start
                            day_start + 86400 + work_start_hour * 3600
                        } else {
                            epoch
                        }
                    };

                    for (busy_start, busy_end) in &merged {
                        let slot_start = clamp_to_working(cursor);
                        let slot_end = clamp_to_working(*busy_start);
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
                    let slot_start = clamp_to_working(cursor);
                    let slot_end = clamp_to_working(range_end);
                    if slot_end - slot_start >= min_duration_secs && slot_start < slot_end {
                        free_slots.push(json!({
                            "start": slot_start,
                            "end": slot_end,
                            "duration_minutes": (slot_end - slot_start) / 60
                        }));
                    }

                    if free_slots.is_empty() {
                        Ok(text_result("No available time slots found in the specified range."))
                    } else {
                        let json = serde_json::to_string_pretty(&free_slots)?;
                        Ok(text_result(format!(
                            "Found {} available time slot(s):\n\n{json}",
                            free_slots.len()
                        )))
                    }
                }
                Err(e) => Ok(error_result(format!("Failed to find available times: {e}"))),
            }
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_schemas_valid() {
        let mut registry = ServiceRegistry::new();
        register(&mut registry);
        let tools = registry.list_tools();
        assert_eq!(tools.len(), 8, "Expected exactly 8 calendar tools");

        let names: Vec<_> = tools.iter().map(|t| t.name.as_ref()).collect();
        assert!(names.contains(&"calendar_list_calendars"));
        assert!(names.contains(&"calendar_search_events"));
        assert!(names.contains(&"calendar_create_event"));
        assert!(names.contains(&"calendar_reschedule_event"));
        assert!(names.contains(&"calendar_cancel_event"));
        assert!(names.contains(&"calendar_update_event"));
        assert!(names.contains(&"calendar_open_event"));
        assert!(names.contains(&"calendar_find_available_times"));
    }
}
