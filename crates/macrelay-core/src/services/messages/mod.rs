use std::sync::Arc;

use rmcp::model::Tool;
use serde_json::json;

use crate::macos::escape::escape_applescript_string;
use crate::registry::{ServiceRegistry, ToolHandler, error_result, schema_from_json, text_result};

/// Register all messages tools with the service registry.
pub fn register(registry: &mut ServiceRegistry) {
    registry.register(
        "communication_messages_search_chats",
        Tool::new(
            "communication_messages_search_chats",
            "[READ] Search iMessage/SMS conversations by participant name, phone number, or email address. Returns matching chats with their identifiers and participants.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Name, phone number, or email to search for in chat participants."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of chats to return. Default 20."
                    }
                },
                "required": ["query"]
            })),
        ),
        handler_search_chats(),
    );

    registry.register(
        "communication_messages_get_chat",
        Tool::new(
            "communication_messages_get_chat",
            "[READ] Get messages from a specific chat by chat ID. Returns message text, sender, timestamps, and attachment info.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "chat_id": {
                        "type": "integer",
                        "description": "The ROWID of the chat to retrieve messages from."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of messages to return. Default 50."
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Number of messages to skip (for pagination). Default 0."
                    }
                },
                "required": ["chat_id"]
            })),
        ),
        handler_get_chat(),
    );

    registry.register(
        "communication_messages_search_messages",
        Tool::new(
            "communication_messages_search_messages",
            "[READ] Search message text across all chats. Returns matching messages with their chat context, sender, and timestamps.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Text to search for within message bodies."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of messages to return. Default 50."
                    }
                },
                "required": ["query"]
            })),
        ),
        handler_search_messages(),
    );

    registry.register(
        "communication_messages_send_message",
        Tool::new(
            "communication_messages_send_message",
            "[CREATE] Send an iMessage to a recipient. The recipient can be a phone number or email address.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "recipient": {
                        "type": "string",
                        "description": "Phone number or email address of the recipient."
                    },
                    "message": {
                        "type": "string",
                        "description": "The message text to send."
                    }
                },
                "required": ["recipient", "message"]
            })),
        ),
        handler_send_message(),
    );
}

/// Open the Messages chat.db in read-only mode.
/// Returns a helpful permission error if the database cannot be accessed.
fn open_chat_db() -> Result<rusqlite::Connection, String> {
    let home = std::env::var("HOME").unwrap_or_default();
    let db_path = format!("{home}/Library/Messages/chat.db");
    rusqlite::Connection::open_with_flags(&db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|_| {
            crate::permissions::PermissionManager::permission_error(
                crate::permissions::PermissionType::FullDiskAccess,
            )
        })
}

/// Convert an Apple CoreData timestamp (nanoseconds since 2001-01-01) to a
/// human-readable UTC date string.
fn apple_timestamp_to_string(nanos: i64) -> String {
    // CoreData timestamp: nanoseconds since 2001-01-01 00:00:00 UTC
    // Convert to Unix timestamp: (nanos / 1_000_000_000) + 978307200
    let unix_ts = (nanos / 1_000_000_000) + 978_307_200;
    // Format as ISO 8601-ish for readability
    let secs = unix_ts;
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    // Simple date calculation from Unix days
    // Epoch: 1970-01-01
    let (year, month, day) = unix_days_to_date(days);
    format!("{year:04}-{month:02}-{day:02} {hours:02}:{minutes:02}:{seconds:02} UTC")
}

/// Convert days since Unix epoch to (year, month, day).
fn unix_days_to_date(days: i64) -> (i64, i64, i64) {
    // Algorithm from https://howardhinnant.github.io/date_algorithms.html
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097; // day of era [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // year of era [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // day of year [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // day [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // month [1, 12]
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

fn handler_search_chats() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let query = match args.get("query").and_then(|v| v.as_str()) {
                Some(q) => q,
                None => return Ok(error_result("query is required")),
            };

            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;

            let conn = match open_chat_db() {
                Ok(c) => c,
                Err(e) => return Ok(error_result(e)),
            };

            let search_pattern = format!("%{query}%");

            // Search chats by matching handle identifiers (phone/email) or chat display_name
            let sql = r#"
                SELECT DISTINCT
                    c.ROWID,
                    c.guid,
                    c.chat_identifier,
                    c.display_name,
                    c.service_name,
                    GROUP_CONCAT(h.id, ', ') AS participants
                FROM chat c
                LEFT JOIN chat_handle_join chj ON chj.chat_id = c.ROWID
                LEFT JOIN handle h ON h.ROWID = chj.handle_id
                WHERE h.id LIKE ?1
                   OR c.display_name LIKE ?1
                   OR c.chat_identifier LIKE ?1
                GROUP BY c.ROWID
                ORDER BY c.ROWID DESC
                LIMIT ?2
            "#;

            let mut stmt = conn.prepare(sql).map_err(|e| anyhow::anyhow!("{e}"))?;
            let rows = stmt
                .query_map(rusqlite::params![search_pattern, limit as i64], |row| {
                    Ok(json!({
                        "chat_id": row.get::<_, i64>(0)?,
                        "guid": row.get::<_, String>(1).unwrap_or_default(),
                        "chat_identifier": row.get::<_, String>(2).unwrap_or_default(),
                        "display_name": row.get::<_, String>(3).unwrap_or_default(),
                        "service": row.get::<_, String>(4).unwrap_or_default(),
                        "participants": row.get::<_, String>(5).unwrap_or_default(),
                    }))
                })
                .map_err(|e| anyhow::anyhow!("{e}"))?;

            let mut results = Vec::new();
            for val in rows.flatten() {
                results.push(val);
            }

            if results.is_empty() {
                Ok(text_result(format!("No chats found matching: {query}")))
            } else {
                let json = serde_json::to_string_pretty(&results)?;
                Ok(text_result(format!(
                    "Found {} chat(s):\n\n{json}",
                    results.len()
                )))
            }
        })
    })
}

fn handler_get_chat() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let chat_id = match args.get("chat_id").and_then(|v| v.as_i64()) {
                Some(id) => id,
                None => return Ok(error_result("chat_id is required")),
            };

            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as i64;

            let offset = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as i64;

            let conn = match open_chat_db() {
                Ok(c) => c,
                Err(e) => return Ok(error_result(e)),
            };

            let sql = r#"
                SELECT
                    m.ROWID,
                    m.guid,
                    m.text,
                    m.date,
                    m.is_from_me,
                    m.cache_has_attachments,
                    h.id AS sender
                FROM message m
                JOIN chat_message_join cmj ON cmj.message_id = m.ROWID
                LEFT JOIN handle h ON h.ROWID = m.handle_id
                WHERE cmj.chat_id = ?1
                ORDER BY m.date DESC
                LIMIT ?2 OFFSET ?3
            "#;

            let mut stmt = conn.prepare(sql).map_err(|e| anyhow::anyhow!("{e}"))?;
            let rows = stmt
                .query_map(rusqlite::params![chat_id, limit, offset], |row| {
                    let date_val: i64 = row.get(3).unwrap_or(0);
                    let is_from_me: i32 = row.get(4).unwrap_or(0);
                    let has_attachments: i32 = row.get(5).unwrap_or(0);
                    Ok(json!({
                        "message_id": row.get::<_, i64>(0)?,
                        "guid": row.get::<_, String>(1).unwrap_or_default(),
                        "text": row.get::<_, String>(2).unwrap_or_default(),
                        "date": apple_timestamp_to_string(date_val),
                        "is_from_me": is_from_me == 1,
                        "has_attachments": has_attachments == 1,
                        "sender": row.get::<_, String>(6).unwrap_or_else(|_| "me".to_string()),
                    }))
                })
                .map_err(|e| anyhow::anyhow!("{e}"))?;

            let mut results = Vec::new();
            for val in rows.flatten() {
                results.push(val);
            }

            if results.is_empty() {
                Ok(text_result(format!(
                    "No messages found for chat_id: {chat_id}"
                )))
            } else {
                let json = serde_json::to_string_pretty(&results)?;
                Ok(text_result(format!(
                    "Retrieved {} message(s) from chat {chat_id}:\n\n{json}",
                    results.len()
                )))
            }
        })
    })
}

fn handler_search_messages() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let query = match args.get("query").and_then(|v| v.as_str()) {
                Some(q) => q,
                None => return Ok(error_result("query is required")),
            };

            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as i64;

            let conn = match open_chat_db() {
                Ok(c) => c,
                Err(e) => return Ok(error_result(e)),
            };

            let search_pattern = format!("%{query}%");

            let sql = r#"
                SELECT
                    m.ROWID,
                    m.text,
                    m.date,
                    m.is_from_me,
                    m.cache_has_attachments,
                    h.id AS sender,
                    c.chat_identifier,
                    c.display_name,
                    c.ROWID AS chat_id
                FROM message m
                LEFT JOIN handle h ON h.ROWID = m.handle_id
                LEFT JOIN chat_message_join cmj ON cmj.message_id = m.ROWID
                LEFT JOIN chat c ON c.ROWID = cmj.chat_id
                WHERE m.text LIKE ?1
                ORDER BY m.date DESC
                LIMIT ?2
            "#;

            let mut stmt = conn.prepare(sql).map_err(|e| anyhow::anyhow!("{e}"))?;
            let rows = stmt
                .query_map(rusqlite::params![search_pattern, limit], |row| {
                    let date_val: i64 = row.get(2).unwrap_or(0);
                    let is_from_me: i32 = row.get(3).unwrap_or(0);
                    let has_attachments: i32 = row.get(4).unwrap_or(0);
                    Ok(json!({
                        "message_id": row.get::<_, i64>(0)?,
                        "text": row.get::<_, String>(1).unwrap_or_default(),
                        "date": apple_timestamp_to_string(date_val),
                        "is_from_me": is_from_me == 1,
                        "has_attachments": has_attachments == 1,
                        "sender": row.get::<_, String>(5).unwrap_or_else(|_| "me".to_string()),
                        "chat_identifier": row.get::<_, String>(6).unwrap_or_default(),
                        "chat_display_name": row.get::<_, String>(7).unwrap_or_default(),
                        "chat_id": row.get::<_, i64>(8).unwrap_or(0),
                    }))
                })
                .map_err(|e| anyhow::anyhow!("{e}"))?;

            let mut results = Vec::new();
            for val in rows.flatten() {
                results.push(val);
            }

            if results.is_empty() {
                Ok(text_result(format!("No messages found matching: {query}")))
            } else {
                let json = serde_json::to_string_pretty(&results)?;
                Ok(text_result(format!(
                    "Found {} message(s) matching \"{query}\":\n\n{json}",
                    results.len()
                )))
            }
        })
    })
}

fn handler_send_message() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let recipient = match args.get("recipient").and_then(|v| v.as_str()) {
                Some(r) => r,
                None => return Ok(error_result("recipient is required")),
            };

            let message = match args.get("message").and_then(|v| v.as_str()) {
                Some(m) => m,
                None => return Ok(error_result("message is required")),
            };

            // Escape special characters for AppleScript strings
            let escaped_recipient = escape_applescript_string(recipient);
            let escaped_message = escape_applescript_string(message);

            let script = format!(
                r#"tell application "Messages"
    set targetService to 1st account whose service type = iMessage
    set targetBuddy to participant "{escaped_recipient}" of targetService
    send "{escaped_message}" to targetBuddy
end tell
return "Message sent to {escaped_recipient}""#
            );

            match crate::macos::applescript::run_applescript(&script) {
                Ok(result) => Ok(text_result(result)),
                Err(e) => Ok(error_result(format!("Failed to send message: {e}"))),
            }
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_tool_schemas_valid() {
        let mut registry = ServiceRegistry::new();
        register(&mut registry);
        let tools = registry.list_tools();
        assert_eq!(tools.len(), 4, "Expected exactly 4 messages tools");

        let names: Vec<_> = tools.iter().map(|t| t.name.as_ref()).collect();
        assert!(names.contains(&"communication_messages_search_chats"));
        assert!(names.contains(&"communication_messages_get_chat"));
        assert!(names.contains(&"communication_messages_search_messages"));
        assert!(names.contains(&"communication_messages_send_message"));
    }

    #[test]
    fn test_apple_timestamp_conversion() {
        // 2023-01-01 00:00:00 UTC
        // Unix timestamp: 1672531200
        // Apple CoreData: (1672531200 - 978307200) * 1_000_000_000 = 694224000_000_000_000
        let nanos: i64 = 694_224_000_000_000_000;
        let result = apple_timestamp_to_string(nanos);
        assert_eq!(result, "2023-01-01 00:00:00 UTC");
    }

    #[test]
    fn test_unix_days_to_date() {
        // 1970-01-01 is day 0
        assert_eq!(unix_days_to_date(0), (1970, 1, 1));
        // 2000-01-01 is day 10957
        assert_eq!(unix_days_to_date(10957), (2000, 1, 1));
    }

    #[tokio::test]
    #[ignore]
    async fn test_search_chats_tool() {
        let handler = handler_search_chats();
        let mut args = HashMap::new();
        args.insert("query".to_string(), json!("John Doe"));

        let result = handler(args).await.unwrap();
        // Since database availability isn't guaranteed in CI, we just verify it doesn't panic
        let _is_err = result.is_error.unwrap_or(false);
    }

    #[tokio::test]
    #[ignore]
    async fn test_get_chat_tool() {
        let handler = handler_get_chat();
        let mut args = HashMap::new();
        args.insert("chat_id".to_string(), json!(1));

        let result = handler(args).await.unwrap();
        let _is_err = result.is_error.unwrap_or(false);
    }

    #[tokio::test]
    #[ignore]
    async fn test_search_messages_tool() {
        let handler = handler_search_messages();
        let mut args = HashMap::new();
        args.insert("query".to_string(), json!("hello"));

        let result = handler(args).await.unwrap();
        let _is_err = result.is_error.unwrap_or(false);
    }

    #[tokio::test]
    async fn test_send_message_tool() {
        use crate::macos::applescript::{MOCK_RUNNER, ScriptRunner};
        use std::sync::Arc;
        use std::time::Duration;

        struct MockAppleScript;
        impl ScriptRunner for MockAppleScript {
            fn run_applescript(&self, script: &str) -> anyhow::Result<String> {
                assert!(script.contains("1st account whose service type = iMessage"));
                assert!(script.contains("participant \"test@example.com\""));
                assert!(script.contains("send \"hello world\""));
                Ok("Message sent to test@example.com".to_string())
            }
            fn run_applescript_with_timeout(
                &self,
                _script: &str,
                _timeout: Duration,
            ) -> anyhow::Result<String> {
                Ok("".to_string())
            }
            fn run_jxa(&self, _script: &str) -> anyhow::Result<String> {
                Ok("".to_string())
            }
        }

        let mock = Arc::new(MockAppleScript);
        let handler = handler_send_message();

        MOCK_RUNNER
            .scope(mock, async {
                let mut args = HashMap::new();
                args.insert("recipient".to_string(), json!("test@example.com"));
                args.insert("message".to_string(), json!("hello world"));

                let result = handler(args).await.unwrap();

                assert!(!result.is_error.unwrap_or(false));

                let content_str = format!("{:?}", result.content);
                assert!(
                    content_str.contains("Message sent to test@example.com"),
                    "Content didn't match: {}",
                    content_str
                );
            })
            .await;
    }

    #[tokio::test]
    async fn test_send_message_escaping() {
        use crate::macos::applescript::{MOCK_RUNNER, ScriptRunner};
        use std::sync::Arc;
        use std::time::Duration;

        struct MockAppleScript;
        impl ScriptRunner for MockAppleScript {
            fn run_applescript(&self, script: &str) -> anyhow::Result<String> {
                assert!(script.contains("participant \"test@example.com\""));
                assert!(script.contains("send \"Hello \\\"World\\\" \\\\backslashed\""));
                Ok("Message sent to test@example.com".to_string())
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

        let mock = Arc::new(MockAppleScript);
        let handler = handler_send_message();

        MOCK_RUNNER
            .scope(mock, async {
                let mut args = HashMap::new();
                args.insert("recipient".to_string(), json!("test@example.com"));
                args.insert(
                    "message".to_string(),
                    json!("Hello \"World\" \\backslashed"),
                );

                let result = handler(args).await.unwrap();
                assert!(!result.is_error.unwrap_or(false));
            })
            .await;
    }

    /// When osascript fails, send_message must return a graceful error
    /// result instead of panicking or propagating a raw anyhow error.
    #[tokio::test]
    async fn test_send_message_returns_error_result_on_osascript_failure() {
        use crate::macos::applescript::{MOCK_RUNNER, ScriptRunner};
        use std::sync::Arc;
        use std::time::Duration;

        struct ErrorMock;
        impl ScriptRunner for ErrorMock {
            fn run_applescript(&self, _script: &str) -> anyhow::Result<String> {
                Err(anyhow::anyhow!(
                    "osascript: Messages got an error: Not authorized to send Apple events"
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
                let handler = handler_send_message();
                let mut args = HashMap::new();
                args.insert("recipient".to_string(), json!("test@example.com"));
                args.insert("message".to_string(), json!("hello"));

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
                    content.contains("Not authorized"),
                    "Expected underlying error to be surfaced, got: {}",
                    content
                );
            })
            .await;
    }

    #[tokio::test]
    async fn test_validation_send_message_requires_recipient() {
        let handler = handler_send_message();
        let mut args = HashMap::new();
        args.insert("message".to_string(), json!("hello"));

        let result = handler(args).await.expect("Handler should not panic");
        assert_eq!(result.is_error, Some(true));
        assert!(
            result.content[0]
                .as_text()
                .unwrap()
                .text
                .contains("recipient is required")
        );
    }

    #[tokio::test]
    async fn test_validation_get_chat_requires_chat_id() {
        let handler = handler_get_chat();
        let args = HashMap::new();

        let result = handler(args).await.expect("Handler should not panic");
        assert_eq!(result.is_error, Some(true));
        assert!(
            result.content[0]
                .as_text()
                .unwrap()
                .text
                .contains("chat_id is required")
        );
    }
}
