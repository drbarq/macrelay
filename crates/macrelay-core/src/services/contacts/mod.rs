use std::sync::Arc;

use rmcp::model::Tool;
use serde_json::json;

use crate::registry::{ServiceRegistry, ToolHandler, error_result, schema_from_json, text_result};

/// Register all contacts tools with the service registry.
pub fn register(registry: &mut ServiceRegistry) {
    registry.register(
        "pim_contacts_search",
        Tool::new(
            "pim_contacts_search",
            "[READ] Search contacts by name, phone number, or email address.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Name, phone number, or email to search for."
                    }
                },
                "required": ["query"]
            })),
        ),
        handler_search(),
    );

    registry.register(
        "pim_contacts_get_all",
        Tool::new(
            "pim_contacts_get_all",
            "[READ] Get all contacts. Returns names, phone numbers, and email addresses.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "limit": {
                        "type": "integer",
                        "description": "Maximum contacts to return. Default 100."
                    }
                }
            })),
        ),
        handler_get_all(),
    );
}

fn handler_search() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let query = match args.get("query").and_then(|v| v.as_str()) {
                Some(q) => q,
                None => return Ok(error_result("query is required")),
            };

            // Use AppleScript for reliable contact search
            let script = format!(
                r#"tell application "Contacts"
                    set matchingPeople to (every person whose name contains "{query}")
                    set output to ""
                    repeat with p in matchingPeople
                        set pName to name of p
                        set pEmails to ""
                        repeat with e in emails of p
                            set pEmails to pEmails & value of e & ", "
                        end repeat
                        set pPhones to ""
                        repeat with ph in phones of p
                            set pPhones to pPhones & value of ph & ", "
                        end repeat
                        set output to output & pName & " | Emails: " & pEmails & " | Phones: " & pPhones & "
"
                    end repeat
                    if output is "" then
                        return "No contacts found matching: {query}"
                    end if
                    return output
                end tell"#
            );

            match crate::macos::applescript::run_applescript(&script) {
                Ok(result) => Ok(text_result(result)),
                Err(e) => Ok(error_result(format!("Failed to search contacts: {e}"))),
            }
        })
    })
}

fn handler_get_all() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(100);

            let script = format!(
                r#"tell application "Contacts"
                    set allPeople to every person
                    set output to ""
                    set counter to 0
                    repeat with p in allPeople
                        if counter >= {limit} then exit repeat
                        set pName to name of p
                        set pEmails to ""
                        repeat with e in emails of p
                            set pEmails to pEmails & value of e & ", "
                        end repeat
                        set pPhones to ""
                        repeat with ph in phones of p
                            set pPhones to pPhones & value of ph & ", "
                        end repeat
                        set output to output & pName & " | Emails: " & pEmails & " | Phones: " & pPhones & "
"
                        set counter to counter + 1
                    end repeat
                    return output
                end tell"#
            );

            match crate::macos::applescript::run_applescript(&script) {
                Ok(result) => {
                    if result.is_empty() {
                        Ok(text_result("No contacts found."))
                    } else {
                        Ok(text_result(result))
                    }
                }
                Err(e) => Ok(error_result(format!("Failed to get contacts: {e}"))),
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
        assert_eq!(tools.len(), 2);

        let names: Vec<_> = tools.iter().map(|t| t.name.as_ref()).collect();
        assert!(names.contains(&"pim_contacts_search"));
        assert!(names.contains(&"pim_contacts_get_all"));
    }

    #[tokio::test]
    async fn test_mock_contacts_search() {
        use crate::macos::applescript::{MOCK_RUNNER, ScriptRunner};
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
                    "Script missing fragment: {}\nScript: {}",
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

        let mock = Arc::new(AssertingMock {
            expected_fragment: "whose name contains \"John\"".to_string(),
            response: "John Doe | Emails: john@example.com,  | Phones: 555-1234, ".to_string(),
        });

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_search();
                let mut args = std::collections::HashMap::new();
                args.insert(
                    "query".to_string(),
                    serde_json::Value::String("John".to_string()),
                );

                let result = handler(args).await.unwrap();
                assert_eq!(result.is_error, Some(false));

                let content = result.content[0].as_text().unwrap().text.as_str();
                assert!(content.contains("John Doe"));
                assert!(content.contains("john@example.com"));
            })
            .await;
    }

    #[tokio::test]
    async fn test_mock_contacts_get_all() {
        use crate::macos::applescript::{MOCK_RUNNER, ScriptRunner};
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
                    "Script missing fragment: {}\nScript: {}",
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

        let mock = Arc::new(AssertingMock {
            expected_fragment: "if counter >= 50 then exit repeat".to_string(),
            response: "Jane Smith | Emails: jane@example.com,  | Phones: 555-5678, ".to_string(),
        });

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_get_all();
                let mut args = std::collections::HashMap::new();
                args.insert("limit".to_string(), serde_json::Value::Number(50.into()));

                let result = handler(args).await.unwrap();
                assert_eq!(result.is_error, Some(false));

                let content = result.content[0].as_text().unwrap().text.as_str();
                assert!(content.contains("Jane Smith"));
                assert!(content.contains("jane@example.com"));
            })
            .await;
    }

    #[tokio::test]
    async fn test_mock_contacts_get_all_default_limit() {
        use crate::macos::applescript::{MOCK_RUNNER, ScriptRunner};
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
                    "Script missing fragment: {}\nScript: {}",
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

        let mock = Arc::new(AssertingMock {
            expected_fragment: "if counter >= 100 then exit repeat".to_string(),
            response: "Jane Smith | Emails: jane@example.com,  | Phones: 555-5678, ".to_string(),
        });

        MOCK_RUNNER
            .scope(mock, async {
                let handler = handler_get_all();
                let args = std::collections::HashMap::new();

                let result = handler(args).await.unwrap();
                assert_eq!(result.is_error, Some(false));

                let content = result.content[0].as_text().unwrap().text.as_str();
                assert!(content.contains("Jane Smith"));
            })
            .await;
    }

    #[tokio::test]
    async fn test_mock_contacts_search_error() {
        use crate::macos::applescript::{MOCK_RUNNER, ScriptRunner};
        use std::sync::Arc;
        use std::time::Duration;

        struct ErrorMock;
        impl ScriptRunner for ErrorMock {
            fn run_applescript(&self, _script: &str) -> anyhow::Result<String> {
                Err(anyhow::anyhow!(
                    "osascript: Contacts got an error: Not authorized"
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
                let handler = handler_search();
                let mut args = std::collections::HashMap::new();
                args.insert("query".to_string(), json!("John"));

                let result = handler(args).await.unwrap();
                assert_eq!(result.is_error, Some(true));
                let content = result.content[0].as_text().unwrap().text.as_str();
                assert!(content.contains("Not authorized"));
            })
            .await;
    }
}
