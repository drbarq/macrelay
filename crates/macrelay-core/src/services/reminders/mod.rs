use std::sync::Arc;

use rmcp::model::Tool;
use serde_json::json;

use crate::macos::escape::escape_applescript_string;
use crate::registry::{ServiceRegistry, ToolHandler, error_result, schema_from_json, text_result};

/// Register all reminder tools with the service registry.
pub fn register(registry: &mut ServiceRegistry) {
    registry.register(
        "pim_reminders_list_lists",
        Tool::new(
            "pim_reminders_list_lists",
            "[READ] List all reminder lists.",
            schema_from_json(json!({
                "type": "object",
                "properties": {},
            })),
        ),
        handler_list_lists(),
    );

    registry.register(
        "pim_reminders_search_reminders",
        Tool::new(
            "pim_reminders_search_reminders",
            "[READ] Search and filter reminders. Returns incomplete reminders by default.",
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
        "pim_reminders_create_reminder",
        Tool::new(
            "pim_reminders_create_reminder",
            "[CREATE] Create a new reminder with title and optional details.",
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
        "pim_reminders_complete_reminder",
        Tool::new(
            "pim_reminders_complete_reminder",
            "[UPDATE] Mark a reminder as complete without deleting it.",
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
        "pim_reminders_update_reminder",
        Tool::new(
            "pim_reminders_update_reminder",
            "[UPDATE] Update a reminder's properties. Finds the reminder by its current title.",
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
        "pim_reminders_delete_reminder",
        Tool::new(
            "pim_reminders_delete_reminder",
            "[DELETE] Permanently delete a reminder by title.",
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
        "pim_reminders_open_reminder",
        Tool::new(
            "pim_reminders_open_reminder",
            "[UPDATE] Open the Reminders app.",
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
                    let lists: Vec<&str> =
                        output.lines().filter(|l| !l.trim().is_empty()).collect();
                    let result: Vec<serde_json::Value> = lists
                        .iter()
                        .map(|name| serde_json::json!({"title": name.trim()}))
                        .collect();
                    let json = serde_json::to_string_pretty(&result)?;
                    Ok(text_result(format!(
                        "Found {} reminder list(s):\n\n{json}",
                        result.len()
                    )))
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
                let escaped_list = escape_applescript_string(list_filter);
                format!(r#"repeat with l in {{list "{escaped_list}"}}"#)
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
                    Ok(text_result(format!(
                        "Found {} reminder(s):\n\n{json}",
                        results.len()
                    )))
                }
                Err(e) => Ok(error_result(format!("Failed to search reminders: {e}"))),
            }
        })
    })
}

fn handler_create_reminder() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let title = match args.get("title").and_then(|v| v.as_str()) {
                Some(t) => t,
                None => return Ok(error_result("title is required")),
            };

            let notes = args.get("notes").and_then(|v| v.as_str()).unwrap_or("");
            let list_name = args.get("list_name").and_then(|v| v.as_str());
            let priority = args
                .get("priority")
                .and_then(|v| v.as_str())
                .unwrap_or("none");

            let escaped_title = escape_applescript_string(title);
            let escaped_notes = escape_applescript_string(notes);

            let priority_num = match priority {
                "high" => 1,
                "medium" => 5,
                "low" => 9,
                _ => 0,
            };

            let list_clause = if let Some(list) = list_name {
                let escaped_list = escape_applescript_string(list);
                format!(r#"tell list "{escaped_list}""#)
            } else {
                "tell default list".to_string()
            };

            let script = format!(
                r#"
                tell application "Reminders"
                    {list_clause}
                        set newReminder to make new reminder with properties {{name:"{escaped_title}", body:"{escaped_notes}"}}
                        set priority of newReminder to {priority_num}
                    end tell
                end tell
                return "Reminder created: {escaped_title}"
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
            let title = match args.get("title").and_then(|v| v.as_str()) {
                Some(t) => t,
                None => return Ok(error_result("title is required")),
            };

            let escaped_title = escape_applescript_string(title);

            let script = format!(
                r#"
                tell application "Reminders"
                    set matchingReminders to (every reminder whose name is "{escaped_title}" and completed is false)
                    if (count of matchingReminders) > 0 then
                        set completed of item 1 of matchingReminders to true
                        return "Marked as complete: {escaped_title}"
                    else
                        return "No incomplete reminder found with title: {escaped_title}"
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
            let title = match args.get("title").and_then(|v| v.as_str()) {
                Some(t) => t,
                None => return Ok(error_result("title is required")),
            };

            let new_title = args.get("new_title").and_then(|v| v.as_str());
            let notes = args.get("notes").and_then(|v| v.as_str());
            let priority = args.get("priority").and_then(|v| v.as_str());

            let escaped_title = escape_applescript_string(title);

            let mut set_clauses = Vec::new();
            if let Some(nt) = new_title {
                let escaped_nt = escape_applescript_string(nt);
                set_clauses.push(format!(r#"set name of r to "{escaped_nt}""#));
            }
            if let Some(n) = notes {
                let escaped_n = escape_applescript_string(n);
                set_clauses.push(format!(r#"set body of r to "{escaped_n}""#));
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
                    set matchingReminders to (every reminder whose name is "{escaped_title}" and completed is false)
                    if (count of matchingReminders) > 0 then
                        set r to item 1 of matchingReminders
                        {updates}
                        return "Updated reminder: {escaped_title}"
                    else
                        return "No incomplete reminder found with title: {escaped_title}"
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
            let title = match args.get("title").and_then(|v| v.as_str()) {
                Some(t) => t,
                None => return Ok(error_result("title is required")),
            };

            let escaped_title = escape_applescript_string(title);

            let script = format!(
                r#"
                tell application "Reminders"
                    set matchingReminders to (every reminder whose name is "{escaped_title}")
                    if (count of matchingReminders) > 0 then
                        delete item 1 of matchingReminders
                        return "Deleted reminder: {escaped_title}"
                    else
                        return "No reminder found with title: {escaped_title}"
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
    use crate::macos::applescript::{MOCK_RUNNER, ScriptRunner};
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::time::Duration;

    struct AssertingMock {
        expected_fragment: String,
        response: String,
    }

    impl ScriptRunner for AssertingMock {
        fn run_applescript(&self, script: &str) -> anyhow::Result<String> {
            assert!(
                script.contains(&self.expected_fragment),
                "script missing fragment {:?}: {}",
                self.expected_fragment,
                script
            );
            Ok(self.response.clone())
        }
        fn run_applescript_with_timeout(
            &self,
            _script: &str,
            _timeout: Duration,
        ) -> anyhow::Result<String> {
            unimplemented!()
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
        assert!(tools.len() >= 7, "Expected at least 7 reminder tools");

        let names: Vec<_> = tools.iter().map(|t| t.name.as_ref()).collect();
        assert!(names.contains(&"pim_reminders_list_lists"));
        assert!(names.contains(&"pim_reminders_search_reminders"));
        assert!(names.contains(&"pim_reminders_create_reminder"));
        assert!(names.contains(&"pim_reminders_complete_reminder"));
        assert!(names.contains(&"pim_reminders_update_reminder"));
        assert!(names.contains(&"pim_reminders_delete_reminder"));
        assert!(names.contains(&"pim_reminders_open_reminder"));
    }

    #[tokio::test]
    async fn test_mock_list_lists() {
        let mock = Arc::new(AssertingMock {
            expected_fragment: "repeat with l in lists".to_string(),
            response: "Personal\nWork\nGroceries\n".to_string(),
        });

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_list_lists();
                let result = handler(HashMap::new()).await.unwrap();
                assert_eq!(result.is_error, Some(false));

                let content = result.content[0].as_text().unwrap().text.as_str();
                assert!(content.contains("Found 3 reminder list(s)"));
                assert!(content.contains("Personal"));
                assert!(content.contains("Work"));
                assert!(content.contains("Groceries"));
            })
            .await;
    }

    #[tokio::test]
    async fn test_mock_search_reminders() {
        let mock = Arc::new(AssertingMock {
            expected_fragment: "set rems to (every reminder of l whose completed is false)"
                .to_string(),
            response: "Buy Milk||2 gallons||1||Groceries\nCall Alice||About project||5||Work\n"
                .to_string(),
        });

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_search_reminders();
                let mut args = HashMap::new();
                args.insert("query".to_string(), json!("Milk"));

                let result = handler(args).await.unwrap();
                assert_eq!(result.is_error, Some(false));

                let content = result.content[0].as_text().unwrap().text.as_str();
                assert!(content.contains("Found 1 reminder(s)"));
                assert!(content.contains("\"title\": \"Buy Milk\""));
                assert!(content.contains("\"notes\": \"2 gallons\""));
            })
            .await;
    }

    #[tokio::test]
    async fn test_mock_create_reminder() {
        let mock = Arc::new(AssertingMock {
            expected_fragment: "make new reminder with properties {name:\"Buy Bread\", body:\"\"}"
                .to_string(),
            response: "Reminder created: Buy Bread".to_string(),
        });

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_create_reminder();
                let mut args = HashMap::new();
                args.insert("title".to_string(), json!("Buy Bread"));

                let result = handler(args).await.unwrap();
                assert_eq!(result.is_error, Some(false));

                let content = result.content[0].as_text().unwrap().text.as_str();
                assert!(content.contains("Reminder created: Buy Bread"));
            })
            .await;
    }

    #[tokio::test]
    async fn test_mock_create_reminder_escaping() {
        let mock = Arc::new(AssertingMock {
            expected_fragment: "name:\"Buy \\\"Wheat\\\" Bread \\\\backslashed\"".to_string(),
            response: "Reminder created: Buy \"Wheat\" Bread \\backslashed".to_string(),
        });

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_create_reminder();
                let mut args = HashMap::new();
                args.insert(
                    "title".to_string(),
                    json!("Buy \"Wheat\" Bread \\backslashed"),
                );

                let result = handler(args).await.unwrap();
                assert_eq!(result.is_error, Some(false));
            })
            .await;
    }

    #[tokio::test]
    async fn test_mock_complete_reminder() {
        let mock = Arc::new(AssertingMock {
            expected_fragment: "set completed of item 1 of matchingReminders to true".to_string(),
            response: "Marked as complete: Buy Bread".to_string(),
        });

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_complete_reminder();
                let mut args = HashMap::new();
                args.insert("title".to_string(), json!("Buy Bread"));

                let result = handler(args).await.unwrap();
                assert_eq!(result.is_error, Some(false));

                let content = result.content[0].as_text().unwrap().text.as_str();
                assert!(content.contains("Marked as complete: Buy Bread"));
            })
            .await;
    }

    #[tokio::test]
    async fn test_mock_update_reminder() {
        let mock = Arc::new(AssertingMock {
            expected_fragment: "set name of r to \"Buy Wheat Bread\"".to_string(),
            response: "Updated reminder: Buy Bread".to_string(),
        });

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_update_reminder();
                let mut args = HashMap::new();
                args.insert("title".to_string(), json!("Buy Bread"));
                args.insert("new_title".to_string(), json!("Buy Wheat Bread"));

                let result = handler(args).await.unwrap();
                assert_eq!(result.is_error, Some(false));

                let content = result.content[0].as_text().unwrap().text.as_str();
                assert!(content.contains("Updated reminder: Buy Bread"));
            })
            .await;
    }

    #[tokio::test]
    async fn test_mock_delete_reminder() {
        let mock = Arc::new(AssertingMock {
            expected_fragment: "delete item 1 of matchingReminders".to_string(),
            response: "Deleted reminder: Buy Bread".to_string(),
        });

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_delete_reminder();
                let mut args = HashMap::new();
                args.insert("title".to_string(), json!("Buy Bread"));

                let result = handler(args).await.unwrap();
                assert_eq!(result.is_error, Some(false));

                let content = result.content[0].as_text().unwrap().text.as_str();
                assert!(content.contains("Deleted reminder: Buy Bread"));
            })
            .await;
    }

    #[tokio::test]
    async fn test_mock_open_reminder() {
        let mock = Arc::new(AssertingMock {
            expected_fragment: "activate".to_string(),
            response: "Reminders app opened".to_string(),
        });

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_open_reminder();
                let result = handler(HashMap::new()).await.unwrap();
                assert_eq!(result.is_error, Some(false));

                let content = result.content[0].as_text().unwrap().text.as_str();
                assert!(content.contains("Reminders app opened"));
            })
            .await;
    }

    /// When osascript fails, the handler must return a graceful error result
    /// (is_error == Some(true) with a human-readable message) instead of
    /// panicking or propagating a raw anyhow error.
    #[tokio::test]
    async fn test_create_reminder_returns_error_result_on_osascript_failure() {
        struct ErrorMock;
        impl ScriptRunner for ErrorMock {
            fn run_applescript(&self, _script: &str) -> anyhow::Result<String> {
                Err(anyhow::anyhow!(
                    "osascript: Reminders got an error: Application isn't running"
                ))
            }
            fn run_applescript_with_timeout(
                &self,
                _script: &str,
                _timeout: Duration,
            ) -> anyhow::Result<String> {
                unimplemented!()
            }
            fn run_jxa(&self, _script: &str) -> anyhow::Result<String> {
                unimplemented!()
            }
        }

        let mock = Arc::new(ErrorMock);
        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_create_reminder();
                let mut args = HashMap::new();
                args.insert("title".to_string(), json!("Buy milk"));

                let result = handler(args)
                    .await
                    .expect("Handler should not panic on osascript error");
                assert_eq!(result.is_error, Some(true));

                let content = result.content[0].as_text().unwrap().text.as_str();
                assert!(
                    content.to_lowercase().contains("fail")
                        || content.to_lowercase().contains("error"),
                    "Expected a human-readable error, got: {}",
                    content
                );
                assert!(
                    content.contains("isn't running"),
                    "Expected underlying error to be surfaced, got: {}",
                    content
                );
            })
            .await;
    }

    #[tokio::test]
    async fn test_validation_create_reminder_requires_title() {
        let handler = handler_create_reminder();
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

    #[tokio::test]
    async fn test_validation_delete_reminder_requires_title() {
        let handler = handler_delete_reminder();
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
