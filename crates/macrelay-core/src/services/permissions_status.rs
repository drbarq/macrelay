use std::sync::Arc;

use rmcp::model::Tool;
use serde_json::json;

use crate::permissions::PermissionManager;
use crate::registry::{ServiceRegistry, schema_from_json, text_result};

/// Register the permissions_status tool with the service registry.
pub fn register(registry: &mut ServiceRegistry) {
    registry.register(
        "system_permissions_status",
        Tool::new(
            "system_permissions_status",
            "[SYSTEM] Check and return the status of all macOS permissions required by MacRelay.",
            schema_from_json(json!({
                "type": "object",
                "properties": {},
                "required": []
            })),
        ),
        handler(),
    );
}

fn handler() -> crate::registry::ToolHandler {
    Arc::new(|_args| {
        Box::pin(async move {
            let statuses = PermissionManager::check_all();

            // Build a sorted JSON object from the permission statuses.
            let mut map = serde_json::Map::new();
            for (perm_type, status) in &statuses {
                let key = serde_json::to_value(perm_type)
                    .unwrap_or_default()
                    .as_str()
                    .unwrap_or("unknown")
                    .to_string();
                let val = serde_json::to_value(status).unwrap_or_default();
                map.insert(key, val);
            }

            // Sort keys for deterministic output.
            let mut entries: Vec<_> = map.into_iter().collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            let sorted_map: serde_json::Map<String, serde_json::Value> =
                entries.into_iter().collect();

            let result_json = serde_json::to_string_pretty(&sorted_map)
                .unwrap_or_else(|e| format!("Error serializing permissions: {e}"));

            Ok(text_result(result_json))
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_tool_schema_valid() {
        let mut registry = ServiceRegistry::new();
        register(&mut registry);
        let tools = registry.list_tools();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name.as_ref(), "system_permissions_status");
    }

    #[tokio::test]
    #[ignore]
    async fn test_permissions_status_handler_returns_valid_json() {
        let h = handler();
        let args = HashMap::new();
        let result = h(args).await.expect("Handler should succeed");

        assert_eq!(result.is_error, Some(false));
        assert_eq!(result.content.len(), 1);

        let content_text = result.content[0]
            .as_text()
            .map(|t| t.text.as_str())
            .expect("Expected text content");

        let json: serde_json::Value =
            serde_json::from_str(content_text).expect("Should be valid JSON");

        assert!(json.is_object());
        let map = json.as_object().unwrap();

        // Ensure all expected keys are present
        let expected_keys = [
            "accessibility",
            "calendar",
            "contacts",
            "full_disk_access",
            "location",
            "reminders",
            "screen_recording",
        ];

        for key in expected_keys {
            assert!(
                map.contains_key(key),
                "Missing key: {}. Content: {}",
                key,
                content_text
            );
        }

        // Verify keys are sorted alphabetically
        let keys: Vec<_> = map.keys().cloned().collect();
        let mut sorted_keys = keys.clone();
        sorted_keys.sort();
        assert_eq!(keys, sorted_keys, "Keys should be sorted alphabetically");

        // Verify status values are valid
        for (key, val) in map {
            let status = val.as_str().expect("Status should be a string");
            match status {
                "granted" | "denied" | "not_determined" | "unknown" => {}
                _ => panic!("Unexpected status '{}' for key '{}'", status, key),
            }
        }
    }
}
