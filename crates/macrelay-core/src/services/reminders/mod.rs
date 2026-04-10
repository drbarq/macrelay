use std::sync::Arc;

use rmcp::model::Tool;
use serde_json::json;

use crate::registry::{error_result, schema_from_json, text_result, ServiceRegistry, ToolHandler};

/// Register all reminder tools with the service registry.
pub fn register(registry: &mut ServiceRegistry) {
    registry.register(
        "reminders_list_lists",
        Tool::new(
            "reminders_list_lists",
            "List all reminder lists.",
            schema_from_json(json!({
                "type": "object",
                "properties": {},
            })),
        ),
        handler_list_lists(),
    );

    registry.register(
        "reminders_search_reminders",
        Tool::new(
            "reminders_search_reminders",
            "Search and filter reminders. Returns incomplete reminders by default.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Text to search for in reminder titles."
                    },
                    "list_name": {
                        "type": "string",
                        "description": "Filter to a specific reminder list."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum results to return. Default 50."
                    }
                }
            })),
        ),
        handler_search_reminders(),
    );

    registry.register(
        "reminders_create_reminder",
        Tool::new(
            "reminders_create_reminder",
            "Create a new reminder with title and optional details.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "title": {
                        "type": "string",
                        "description": "Reminder title."
                    },
                    "notes": {
                        "type": "string",
                        "description": "Additional notes."
                    },
                    "list_name": {
                        "type": "string",
                        "description": "Reminder list to add to. Uses default list if omitted."
                    },
                    "priority": {
                        "type": "string",
                        "enum": ["none", "low", "medium", "high"],
                        "description": "Priority level. Default none."
                    }
                },
                "required": ["title"]
            })),
        ),
        handler_create_reminder(),
    );

    registry.register(
        "reminders_complete_reminder",
        Tool::new(
            "reminders_complete_reminder",
            "Mark a reminder as complete without deleting it.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "title": {
                        "type": "string",
                        "description": "Title of the reminder to complete."
                    }
                },
                "required": ["title"]
            })),
        ),
        handler_complete_reminder(),
    );

    registry.register(
        "reminders_update_reminder",
        Tool::new(
            "reminders_update_reminder",
            "Update a reminder's properties. Finds the reminder by its current title.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "title": {
                        "type": "string",
                        "description": "Current title of the reminder to update."
                    },
                    "new_title": {
                        "type": "string",
                        "description": "New title for the reminder."
                    },
                    "notes": {
                        "type": "string",
                        "description": "New notes for the reminder."
                    },
                    "priority": {
                        "type": "string",
                        "enum": ["none", "low", "medium", "high"],
                        "description": "New priority level."
                    }
                },
                "required": ["title"]
            })),
        ),
        handler_update_reminder(),
    );

    registry.register(
        "reminders_delete_reminder",
        Tool::new(
            "reminders_delete_reminder",
            "Permanently delete a reminder by title.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "title": {
                        "type": "string",
                        "description": "Title of the reminder to delete."
                    }
                },
                "required": ["title"]
            })),
        ),
        handler_delete_reminder(),
    );

    registry.register(
        "reminders_open_reminder",
        Tool::new(
            "reminders_open_reminder",
            "Open the Reminders app.",
            schema_from_json(json!({
                "type": "object",
                "properties": {},
            })),
        ),
        handler_open_reminder(),
    );
}

fn handler_list_lists() -> ToolHandler {
    Arc::new(|_args| {
        Box::pin(async move {
            let script = r#"
                tell application "Reminders"
                    set output to ""
                    repeat with l in lists
                        set output to output & name of l & linefeed
                    end repeat
                    return output
                end tell
            "#;

            match crate::macos::applescript::run_applescript(script) {
                Ok(output) => {
                    let lists: Vec<&str> = output.lines().filter(|l| !l.trim().is_empty()).collect();
                    let result: Vec<serde_json::Value> = lists
                        .iter()
                        .map(|name| serde_json::json!({"title": name.trim()}))
                        .collect();
                    let json = serde_json::to_string_pretty(&result)?;
                    Ok(text_result(format!("Found {} reminder list(s):\n\n{json}", result.len())))
                }
                Err(e) => Ok(error_result(format!("Failed to list reminder lists: {e}"))),
            }
        })
    })
}

fn handler_search_reminders() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(50);
            let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
            let list_filter = args.get("list_name").and_then(|v| v.as_str()).unwrap_or("");

            let list_clause = if list_filter.is_empty() {
                "repeat with l in lists".to_string()
            } else {
                format!(r#"repeat with l in {{list "{list_filter}"}}"#)
            };

            let script = format!(
                r#"
                tell application "Reminders"
                    set output to ""
                    set counter to 0
                    {list_clause}
                        set rems to (every reminder of l whose completed is false)
                        repeat with r in rems
                            if counter >= {limit} then exit repeat
                            set rName to name of r
                            set rBody to ""
                            try
                                set rBody to body of r
                            end try
                            set rPriority to priority of r
                            set output to output & rName & "||" & rBody & "||" & rPriority & "||" & (name of l) & linefeed
                            set counter to counter + 1
                        end repeat
                    end repeat
                    return output
                end tell
                "#
            );

            match crate::macos::applescript::run_applescript(&script) {
                Ok(output) => {
                    let mut results: Vec<serde_json::Value> = Vec::new();
                    for line in output.lines() {
                        let line = line.trim();
                        if line.is_empty() {
                            continue;
                        }
                        let parts: Vec<&str> = line.split("||").collect();
                        let name = parts.first().unwrap_or(&"").to_string();

                        if !query.is_empty() && !name.to_lowercase().contains(&query.to_lowercase())
                        {
                            continue;
                        }

                        results.push(serde_json::json!({
                            "title": name,
                            "notes": parts.get(1).unwrap_or(&""),
                            "priority": parts.get(2).unwrap_or(&"0"),
                            "list": parts.get(3).unwrap_or(&""),
                        }));
                    }
                    let json = serde_json::to_string_pretty(&results)?;
                    Ok(text_result(format!("Found {} reminder(s):\n\n{json}", results.len())))
                }
                Err(e) => Ok(error_result(format!("Failed to search reminders: {e}"))),
            }
        })
    })
}

fn handler_create_reminder() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let title = args
                .get("title")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("title is required"))?;

            let notes = args.get("notes").and_then(|v| v.as_str()).unwrap_or("");
            let list_name = args.get("list_name").and_then(|v| v.as_str());
            let priority = args
                .get("priority")
                .and_then(|v| v.as_str())
                .unwrap_or("none");

            let priority_num = match priority {
                "high" => 1,
                "medium" => 5,
                "low" => 9,
                _ => 0,
            };

            let list_clause = if let Some(list) = list_name {
                format!(r#"tell list "{list}""#)
            } else {
                "tell default list".to_string()
            };

            let script = format!(
                r#"
                tell application "Reminders"
                    {list_clause}
                        set newReminder to make new reminder with properties {{name:"{title}", body:"{notes}"}}
                        set priority of newReminder to {priority_num}
                    end tell
                end tell
                return "Reminder created: {title}"
                "#
            );

            match crate::macos::applescript::run_applescript(&script) {
                Ok(result) => Ok(text_result(result)),
                Err(e) => Ok(error_result(format!("Failed to create reminder: {e}"))),
            }
        })
    })
}

fn handler_complete_reminder() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let title = args
                .get("title")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("title is required"))?;

            let script = format!(
                r#"
                tell application "Reminders"
                    set matchingReminders to (every reminder whose name is "{title}" and completed is false)
                    if (count of matchingReminders) > 0 then
                        set completed of item 1 of matchingReminders to true
                        return "Marked as complete: {title}"
                    else
                        return "No incomplete reminder found with title: {title}"
                    end if
                end tell
                "#
            );

            match crate::macos::applescript::run_applescript(&script) {
                Ok(result) => Ok(text_result(result)),
                Err(e) => Ok(error_result(format!("Failed to complete reminder: {e}"))),
            }
        })
    })
}

fn handler_update_reminder() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let title = args
                .get("title")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("title is required"))?;

            let new_title = args.get("new_title").and_then(|v| v.as_str());
            let notes = args.get("notes").and_then(|v| v.as_str());
            let priority = args.get("priority").and_then(|v| v.as_str());

            let mut set_clauses = Vec::new();
            if let Some(nt) = new_title {
                set_clauses.push(format!(r#"set name of r to "{nt}""#));
            }
            if let Some(n) = notes {
                set_clauses.push(format!(r#"set body of r to "{n}""#));
            }
            if let Some(p) = priority {
                let priority_num = match p {
                    "high" => 1,
                    "medium" => 5,
                    "low" => 9,
                    _ => 0,
                };
                set_clauses.push(format!("set priority of r to {priority_num}"));
            }

            if set_clauses.is_empty() {
                return Ok(error_result(
                    "No update fields provided. Specify at least one of: new_title, notes, priority.",
                ));
            }

            let updates = set_clauses.join("\n                            ");

            let script = format!(
                r#"
                tell application "Reminders"
                    set matchingReminders to (every reminder whose name is "{title}" and completed is false)
                    if (count of matchingReminders) > 0 then
                        set r to item 1 of matchingReminders
                        {updates}
                        return "Updated reminder: {title}"
                    else
                        return "No incomplete reminder found with title: {title}"
                    end if
                end tell
                "#
            );

            match crate::macos::applescript::run_applescript(&script) {
                Ok(result) => Ok(text_result(result)),
                Err(e) => Ok(error_result(format!("Failed to update reminder: {e}"))),
            }
        })
    })
}

fn handler_delete_reminder() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let title = args
                .get("title")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("title is required"))?;

            let script = format!(
                r#"
                tell application "Reminders"
                    set matchingReminders to (every reminder whose name is "{title}")
                    if (count of matchingReminders) > 0 then
                        delete item 1 of matchingReminders
                        return "Deleted reminder: {title}"
                    else
                        return "No reminder found with title: {title}"
                    end if
                end tell
                "#
            );

            match crate::macos::applescript::run_applescript(&script) {
                Ok(result) => Ok(text_result(result)),
                Err(e) => Ok(error_result(format!("Failed to delete reminder: {e}"))),
            }
        })
    })
}

fn handler_open_reminder() -> ToolHandler {
    Arc::new(|_args| {
        Box::pin(async move {
            let script = r#"
                tell application "Reminders"
                    activate
                end tell
                return "Reminders app opened"
            "#;

            match crate::macos::applescript::run_applescript(script) {
                Ok(result) => Ok(text_result(result)),
                Err(e) => Ok(error_result(format!("Failed to open Reminders: {e}"))),
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
        assert!(tools.len() >= 7, "Expected at least 7 reminder tools");

        let names: Vec<_> = tools.iter().map(|t| t.name.as_ref()).collect();
        assert!(names.contains(&"reminders_list_lists"));
        assert!(names.contains(&"reminders_search_reminders"));
        assert!(names.contains(&"reminders_create_reminder"));
        assert!(names.contains(&"reminders_complete_reminder"));
        assert!(names.contains(&"reminders_update_reminder"));
        assert!(names.contains(&"reminders_delete_reminder"));
        assert!(names.contains(&"reminders_open_reminder"));
    }
}
