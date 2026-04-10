use std::sync::Arc;

use rmcp::model::Tool;
use serde_json::json;

use crate::registry::{error_result, schema_from_json, text_result, ServiceRegistry, ToolHandler};

/// Register all shortcuts tools with the service registry.
pub fn register(registry: &mut ServiceRegistry) {
    registry.register(
        "shortcuts_list",
        Tool::new(
            "shortcuts_list",
            "List all installed Shortcuts on this Mac.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "folder": {
                        "type": "string",
                        "description": "Optional folder name to filter shortcuts."
                    }
                }
            })),
        ),
        handler_shortcuts_list(),
    );

    registry.register(
        "shortcuts_get",
        Tool::new(
            "shortcuts_get",
            "Get details about a specific shortcut by name. Verifies the shortcut exists and returns its name.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "The name of the shortcut to look up."
                    }
                },
                "required": ["name"]
            })),
        ),
        handler_shortcuts_get(),
    );

    registry.register(
        "shortcuts_run",
        Tool::new(
            "shortcuts_run",
            "Run a shortcut by name. WARNING: This executes the shortcut with real effects on the system.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "The name of the shortcut to run."
                    },
                    "input": {
                        "type": "string",
                        "description": "Optional text input to pass to the shortcut via stdin."
                    },
                    "timeout_secs": {
                        "type": "integer",
                        "description": "Timeout in seconds for shortcut execution. Default 30."
                    }
                },
                "required": ["name"]
            })),
        ),
        handler_shortcuts_run(),
    );
}

fn handler_shortcuts_list() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let folder = args.get("folder").and_then(|v| v.as_str());

            let script = if let Some(f) = folder {
                let escaped = f.replace('\'', "'\\''");
                format!(
                    r#"do shell script "/usr/bin/shortcuts list --folder-name '{}' 2>&1""#,
                    escaped
                )
            } else {
                r#"do shell script "/usr/bin/shortcuts list 2>&1""#.to_string()
            };

            match crate::macos::applescript::run_applescript(&script) {
                Ok(output) => {
                    if output.trim().is_empty() {
                        Ok(text_result("No shortcuts found."))
                    } else {
                        let lines: Vec<&str> = output.lines().collect();
                        Ok(text_result(format!(
                            "Found {} shortcut(s):\n{}",
                            lines.len(),
                            output
                        )))
                    }
                }
                Err(e) => Ok(error_result(format!("Failed to list shortcuts: {e}"))),
            }
        })
    })
}

fn handler_shortcuts_get() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("name is required"))?;

            let escaped_name = name.replace('\'', "'\\''");

            let script = format!(
                r#"do shell script "/usr/bin/shortcuts list | grep -i '{}' 2>&1""#,
                escaped_name
            );

            match crate::macos::applescript::run_applescript(&script) {
                Ok(output) => {
                    if output.trim().is_empty() {
                        Ok(text_result(format!(
                            "No shortcut found matching: {name}"
                        )))
                    } else {
                        Ok(text_result(format!(
                            "Shortcut found: {}",
                            output.trim()
                        )))
                    }
                }
                Err(e) => {
                    // grep returns exit code 1 when no match is found,
                    // which AppleScript treats as an error
                    let err_str = format!("{e}");
                    if err_str.contains("exit code") || err_str.contains("status 1") {
                        Ok(text_result(format!(
                            "No shortcut found matching: {name}"
                        )))
                    } else {
                        Ok(error_result(format!("Failed to get shortcut: {e}")))
                    }
                }
            }
        })
    })
}

fn handler_shortcuts_run() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("name is required"))?;

            let input = args.get("input").and_then(|v| v.as_str());
            let timeout_secs = args
                .get("timeout_secs")
                .and_then(|v| v.as_u64())
                .unwrap_or(30);

            let escaped_name = name.replace('\'', "'\\''");

            let script = if let Some(input_text) = input {
                let escaped_input = input_text.replace('\'', "'\\''");
                format!(
                    r#"do shell script "echo '{}' | /usr/bin/shortcuts run '{}' 2>&1""#,
                    escaped_input, escaped_name
                )
            } else {
                format!(
                    r#"do shell script "/usr/bin/shortcuts run '{}' 2>&1""#,
                    escaped_name
                )
            };

            let timeout = std::time::Duration::from_secs(timeout_secs);

            match crate::macos::applescript::run_applescript_with_timeout(&script, timeout) {
                Ok(output) => {
                    if output.trim().is_empty() {
                        Ok(text_result(format!(
                            "Shortcut '{}' executed successfully (no output).",
                            name
                        )))
                    } else {
                        Ok(text_result(format!(
                            "Shortcut '{}' output:\n{}",
                            name, output
                        )))
                    }
                }
                Err(e) => Ok(error_result(format!(
                    "Failed to run shortcut '{}': {e}",
                    name
                ))),
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
        assert_eq!(tools.len(), 3, "Expected exactly 3 shortcuts tools");

        let names: Vec<_> = tools.iter().map(|t| t.name.as_ref()).collect();
        assert!(names.contains(&"shortcuts_list"));
        assert!(names.contains(&"shortcuts_get"));
        assert!(names.contains(&"shortcuts_run"));
    }
}
