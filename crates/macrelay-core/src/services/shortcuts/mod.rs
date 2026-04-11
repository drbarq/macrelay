use std::sync::Arc;

use rmcp::model::Tool;
use serde_json::json;

use crate::macos::escape::{escape_applescript_string, escape_shell_single_quoted};
use crate::registry::{ServiceRegistry, ToolHandler, error_result, schema_from_json, text_result};

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
                // User input is embedded inside `do shell script "..."`, so it must be
                // escaped for BOTH layers: shell single-quoting first, then AppleScript
                // string-literal escaping on the result.
                let escaped = escape_applescript_string(&escape_shell_single_quoted(f));
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
            let name = match args.get("name").and_then(|v| v.as_str()) {
                Some(n) => n,
                None => return Ok(error_result("name is required")),
            };

            let escaped_name = escape_applescript_string(&escape_shell_single_quoted(name));

            // `|| true` keeps exit 0 when grep finds no matches, so a missing
            // shortcut returns empty output instead of bubbling up as an error.
            let script = format!(
                r#"do shell script "/usr/bin/shortcuts list 2>&1 | grep -i '{escaped_name}' || true""#
            );

            match crate::macos::applescript::run_applescript(&script) {
                Ok(output) => {
                    if output.trim().is_empty() {
                        Ok(text_result(format!("No shortcut found matching: {name}")))
                    } else {
                        Ok(text_result(format!("Shortcut found: {}", output.trim())))
                    }
                }
                Err(e) => Ok(error_result(format!("Failed to get shortcut: {e}"))),
            }
        })
    })
}

fn handler_shortcuts_run() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let name = match args.get("name").and_then(|v| v.as_str()) {
                Some(n) => n,
                None => return Ok(error_result("name is required")),
            };

            let input = args.get("input").and_then(|v| v.as_str());
            let timeout_secs = args
                .get("timeout_secs")
                .and_then(|v| v.as_u64())
                .unwrap_or(30);

            let escaped_name = escape_applescript_string(&escape_shell_single_quoted(name));

            let script = if let Some(input_text) = input {
                let escaped_input =
                    escape_applescript_string(&escape_shell_single_quoted(input_text));
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

    #[tokio::test]
    async fn test_mock_shortcuts_list() {
        use crate::macos::applescript::{MOCK_RUNNER, ScriptRunner};
        use std::sync::Arc;
        use std::time::Duration;

        struct MockShortcuts;
        impl ScriptRunner for MockShortcuts {
            fn run_applescript(&self, script: &str) -> anyhow::Result<String> {
                assert!(script.contains("shortcuts list"));
                Ok("My Shortcut 1\nMy Shortcut 2".to_string())
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

        let mock = Arc::new(MockShortcuts);
        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_shortcuts_list();
                let args = std::collections::HashMap::new();

                let result = handler(args).await.unwrap();
                assert_eq!(result.is_error, Some(false));

                let content = result.content[0].as_text().unwrap().text.as_str();
                assert!(content.contains("Found 2 shortcut(s):"));
                assert!(content.contains("My Shortcut 1"));
                assert!(content.contains("My Shortcut 2"));
            })
            .await;
    }

    #[tokio::test]
    async fn test_mock_shortcuts_get() {
        use crate::macos::applescript::{MOCK_RUNNER, ScriptRunner};
        use std::sync::Arc;
        use std::time::Duration;

        struct MockShortcuts;
        impl ScriptRunner for MockShortcuts {
            fn run_applescript(&self, script: &str) -> anyhow::Result<String> {
                assert!(script.contains("grep -i 'My Shortcut'"));
                Ok("My Shortcut".to_string())
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

        let mock = Arc::new(MockShortcuts);
        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_shortcuts_get();
                let mut args = std::collections::HashMap::new();
                args.insert(
                    "name".to_string(),
                    serde_json::Value::String("My Shortcut".to_string()),
                );

                let result = handler(args).await.unwrap();
                assert_eq!(result.is_error, Some(false));

                let content = result.content[0].as_text().unwrap().text.as_str();
                assert!(content.contains("Shortcut found: My Shortcut"));
            })
            .await;
    }

    #[tokio::test]
    async fn test_mock_shortcuts_run() {
        use crate::macos::applescript::{MOCK_RUNNER, ScriptRunner};
        use std::sync::Arc;
        use std::time::Duration;

        struct MockShortcuts;
        impl ScriptRunner for MockShortcuts {
            fn run_applescript(&self, _script: &str) -> anyhow::Result<String> {
                unimplemented!()
            }
            fn run_applescript_with_timeout(
                &self,
                script: &str,
                _timeout: Duration,
            ) -> anyhow::Result<String> {
                assert!(script.contains("shortcuts run 'My Shortcut'"));
                Ok("Shortcut output data".to_string())
            }
            fn run_jxa(&self, _script: &str) -> anyhow::Result<String> {
                unimplemented!()
            }
        }

        let mock = Arc::new(MockShortcuts);
        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_shortcuts_run();
                let mut args = std::collections::HashMap::new();
                args.insert(
                    "name".to_string(),
                    serde_json::Value::String("My Shortcut".to_string()),
                );

                let result = handler(args).await.unwrap();
                assert_eq!(result.is_error, Some(false));

                let content = result.content[0].as_text().unwrap().text.as_str();
                assert!(content.contains("Shortcut 'My Shortcut' output:"));
                assert!(content.contains("Shortcut output data"));
            })
            .await;
    }

    #[tokio::test]
    async fn test_mock_shortcuts_run_error() {
        use crate::macos::applescript::{MOCK_RUNNER, ScriptRunner};
        use std::sync::Arc;
        use std::time::Duration;

        struct ErrorMock;
        impl ScriptRunner for ErrorMock {
            fn run_applescript(&self, _script: &str) -> anyhow::Result<String> {
                unimplemented!()
            }
            fn run_applescript_with_timeout(
                &self,
                _script: &str,
                _timeout: Duration,
            ) -> anyhow::Result<String> {
                Err(anyhow::anyhow!(
                    "osascript error: The shortcut was not found"
                ))
            }
            fn run_jxa(&self, _script: &str) -> anyhow::Result<String> {
                unimplemented!()
            }
        }

        let mock = Arc::new(ErrorMock);
        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_shortcuts_run();
                let mut args = std::collections::HashMap::new();
                args.insert("name".to_string(), json!("Invalid"));

                let result = handler(args).await.unwrap();
                assert_eq!(result.is_error, Some(true));
                let content = result.content[0].as_text().unwrap().text.as_str();
                assert!(content.contains("Failed to run shortcut"));
            })
            .await;
    }
}
