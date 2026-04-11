use std::sync::Arc;

use rmcp::model::Tool;
use serde_json::json;

use crate::macos::escape::{escape_jxa_string, escape_shell_single_quoted};
use crate::registry::{ServiceRegistry, ToolHandler, error_result, schema_from_json, text_result};

/// Register all stickies tools with the service registry.
pub fn register(registry: &mut ServiceRegistry) {
    registry.register(
        "stickies_list",
        Tool::new(
            "stickies_list",
            "List all sticky notes. Reads the Stickies data directory to find available notes.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Optional text to filter sticky note filenames."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of stickies to return."
                    }
                }
            })),
        ),
        handler_stickies_list(),
    );

    registry.register(
        "stickies_read",
        Tool::new(
            "stickies_read",
            "Read the content of a sticky note by its ID (directory name).",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "sticky_id": {
                        "type": "string",
                        "description": "The sticky note ID (directory name) to read."
                    }
                },
                "required": ["sticky_id"]
            })),
        ),
        handler_stickies_read(),
    );

    registry.register(
        "stickies_create",
        Tool::new(
            "stickies_create",
            "Create a new sticky note by activating Stickies and typing content via System Events.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "content": {
                        "type": "string",
                        "description": "The text content for the new sticky note."
                    },
                    "color": {
                        "type": "string",
                        "description": "Optional color for the sticky note (e.g. 'yellow', 'blue', 'green', 'pink', 'purple', 'gray')."
                    }
                },
                "required": ["content"]
            })),
        ),
        handler_stickies_create(),
    );

    registry.register(
        "stickies_open",
        Tool::new(
            "stickies_open",
            "Open the Stickies application.",
            schema_from_json(json!({
                "type": "object",
                "properties": {}
            })),
        ),
        handler_stickies_open(),
    );
}

fn handler_stickies_list() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let query = args.get("query").and_then(|v| v.as_str());
            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(100) as usize;

            let script = r#"do shell script "ls -1 ~/Library/Containers/com.apple.Stickies/Data/Library/Stickies/ 2>/dev/null || echo 'No stickies directory found'""#;

            match crate::macos::applescript::run_applescript(script) {
                Ok(output) => {
                    let mut lines: Vec<&str> = output.lines().collect();

                    if let Some(q) = query {
                        let q_lower = q.to_lowercase();
                        lines.retain(|line| line.to_lowercase().contains(&q_lower));
                    }

                    lines.truncate(limit);

                    if lines.is_empty() || (lines.len() == 1 && lines[0].contains("No stickies")) {
                        Ok(text_result("No sticky notes found."))
                    } else {
                        Ok(text_result(format!(
                            "Found {} sticky note(s):\n{}",
                            lines.len(),
                            lines.join("\n")
                        )))
                    }
                }
                Err(e) => Ok(error_result(format!("Failed to list stickies: {e}"))),
            }
        })
    })
}

fn handler_stickies_read() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let sticky_id = match args.get("sticky_id").and_then(|v| v.as_str()) {
                Some(id) => id,
                None => return Ok(error_result("sticky_id is required")),
            };

            let escaped_id = escape_shell_single_quoted(sticky_id);

            let script = format!(
                r#"do shell script "cat ~/Library/Containers/com.apple.Stickies/Data/Library/Stickies/'{escaped_id}'/TXT.rtf 2>/dev/null || echo 'Sticky not found'""#
            );

            match crate::macos::applescript::run_applescript(&script) {
                Ok(result) => Ok(text_result(result)),
                Err(e) => Ok(error_result(format!("Failed to read sticky: {e}"))),
            }
        })
    })
}

fn handler_stickies_create() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let content = match args.get("content").and_then(|v| v.as_str()) {
                Some(c) => c,
                None => return Ok(error_result("content is required")),
            };

            let _color = args
                .get("color")
                .and_then(|v| v.as_str())
                .unwrap_or("yellow");

            // Escape content for JXA single-quoted string embedding
            let escaped_content = escape_jxa_string(content);

            let script = format!(
                r#"
                var stickies = Application('Stickies');
                stickies.activate();
                var se = Application('System Events');
                delay(0.5);
                se.keystroke('n', {{using: 'command down'}});
                delay(0.3);
                se.keystroke('{escaped_content}');
                'Created new sticky note';
                "#
            );

            match crate::macos::applescript::run_jxa(&script) {
                Ok(result) => Ok(text_result(result)),
                Err(e) => Ok(error_result(format!("Failed to create sticky: {e}"))),
            }
        })
    })
}

fn handler_stickies_open() -> ToolHandler {
    Arc::new(|_args| {
        Box::pin(async move {
            let script = r#"tell application "Stickies" to activate"#;

            match crate::macos::applescript::run_applescript(script) {
                Ok(_) => Ok(text_result("Stickies application opened.")),
                Err(e) => Ok(error_result(format!("Failed to open Stickies: {e}"))),
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
        assert_eq!(tools.len(), 4, "Expected exactly 4 stickies tools");

        let names: Vec<_> = tools.iter().map(|t| t.name.as_ref()).collect();
        assert!(names.contains(&"stickies_list"));
        assert!(names.contains(&"stickies_read"));
        assert!(names.contains(&"stickies_create"));
        assert!(names.contains(&"stickies_open"));
    }

    #[tokio::test]
    async fn test_mock_stickies_list() {
        use crate::macos::applescript::{MOCK_RUNNER, ScriptRunner};
        use std::sync::Arc;
        use std::time::Duration;

        struct MockStickies;
        impl ScriptRunner for MockStickies {
            fn run_applescript(&self, script: &str) -> anyhow::Result<String> {
                assert!(script.contains("ls -1"));
                Ok("Note1.rtfd\nNote2.rtfd".to_string())
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

        let mock = Arc::new(MockStickies);
        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_stickies_list();
                let args = std::collections::HashMap::new();

                let result = handler(args).await.unwrap();
                assert_eq!(result.is_error, Some(false));

                let content = result.content[0].as_text().unwrap().text.as_str();
                assert!(content.contains("Found 2 sticky note(s):"));
                assert!(content.contains("Note1.rtfd"));
                assert!(content.contains("Note2.rtfd"));
            })
            .await;
    }

    #[tokio::test]
    async fn test_mock_stickies_read() {
        use crate::macos::applescript::{MOCK_RUNNER, ScriptRunner};
        use std::sync::Arc;
        use std::time::Duration;

        struct MockStickies;
        impl ScriptRunner for MockStickies {
            fn run_applescript(&self, script: &str) -> anyhow::Result<String> {
                assert!(script.contains("cat"));
                assert!(script.contains("Note1.rtfd"));
                Ok("This is a sticky note.".to_string())
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

        let mock = Arc::new(MockStickies);
        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_stickies_read();
                let mut args = std::collections::HashMap::new();
                args.insert(
                    "sticky_id".to_string(),
                    serde_json::Value::String("Note1.rtfd".to_string()),
                );

                let result = handler(args).await.unwrap();
                assert_eq!(result.is_error, Some(false));

                let content = result.content[0].as_text().unwrap().text.as_str();
                assert_eq!(content, "This is a sticky note.");
            })
            .await;
    }

    #[tokio::test]
    async fn test_mock_stickies_create() {
        use crate::macos::applescript::{MOCK_RUNNER, ScriptRunner};
        use std::sync::Arc;
        use std::time::Duration;

        struct MockStickies;
        impl ScriptRunner for MockStickies {
            fn run_applescript(&self, _script: &str) -> anyhow::Result<String> {
                unimplemented!()
            }
            fn run_applescript_with_timeout(
                &self,
                _script: &str,
                _timeout: Duration,
            ) -> anyhow::Result<String> {
                unimplemented!()
            }
            fn run_jxa(&self, script: &str) -> anyhow::Result<String> {
                assert!(script.contains("se.keystroke('Hello World')"));
                Ok("Created new sticky note".to_string())
            }
        }

        let mock = Arc::new(MockStickies);
        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_stickies_create();
                let mut args = std::collections::HashMap::new();
                args.insert(
                    "content".to_string(),
                    serde_json::Value::String("Hello World".to_string()),
                );

                let result = handler(args).await.unwrap();
                assert_eq!(result.is_error, Some(false));

                let content = result.content[0].as_text().unwrap().text.as_str();
                assert_eq!(content, "Created new sticky note");
            })
            .await;
    }

    #[tokio::test]
    async fn test_mock_stickies_open() {
        use crate::macos::applescript::{MOCK_RUNNER, ScriptRunner};
        use std::sync::Arc;
        use std::time::Duration;

        struct MockStickies;
        impl ScriptRunner for MockStickies {
            fn run_applescript(&self, script: &str) -> anyhow::Result<String> {
                assert!(script.contains("tell application \"Stickies\" to activate"));
                Ok("".to_string())
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

        let mock = Arc::new(MockStickies);
        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_stickies_open();
                let args = std::collections::HashMap::new();

                let result = handler(args).await.unwrap();
                assert_eq!(result.is_error, Some(false));

                let content = result.content[0].as_text().unwrap().text.as_str();
                assert_eq!(content, "Stickies application opened.");
            })
            .await;
    }

    #[tokio::test]
    async fn test_mock_stickies_open_error() {
        use crate::macos::applescript::{MOCK_RUNNER, ScriptRunner};
        use std::sync::Arc;
        use std::time::Duration;

        struct ErrorMock;
        impl ScriptRunner for ErrorMock {
            fn run_applescript(&self, _script: &str) -> anyhow::Result<String> {
                Err(anyhow::anyhow!(
                    "osascript error: Can't get application \"Stickies\""
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
                let handler = handler_stickies_open();
                let args = std::collections::HashMap::new();

                let result = handler(args).await.unwrap();
                assert_eq!(result.is_error, Some(true));
                let content = result.content[0].as_text().unwrap().text.as_str();
                assert!(content.contains("Failed to open Stickies"));
            })
            .await;
    }
}
