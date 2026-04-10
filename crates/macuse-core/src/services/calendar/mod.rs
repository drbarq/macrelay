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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_schemas_valid() {
        let mut registry = ServiceRegistry::new();
        register(&mut registry);
        let tools = registry.list_tools();
        assert!(tools.len() >= 3, "Expected at least 3 calendar tools");

        let names: Vec<_> = tools.iter().map(|t| t.name.as_ref()).collect();
        assert!(names.contains(&"calendar_list_calendars"));
        assert!(names.contains(&"calendar_search_events"));
        assert!(names.contains(&"calendar_create_event"));
    }
}
