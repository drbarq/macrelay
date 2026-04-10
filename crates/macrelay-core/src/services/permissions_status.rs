use std::sync::Arc;

use rmcp::model::Tool;
use serde_json::json;

use crate::permissions::PermissionManager;
use crate::registry::{schema_from_json, text_result, ServiceRegistry};

/// Register the permissions_status tool with the service registry.
pub fn register(registry: &mut ServiceRegistry) {
    registry.register(
        "permissions_status",
        Tool::new(
            "permissions_status",
            "Check and return the status of all macOS permissions required by mac-app-oss.",
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

            let result_json =
                serde_json::to_string_pretty(&sorted_map).unwrap_or_else(|e| format!("Error serializing permissions: {e}"));

            Ok(text_result(result_json))
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_schema_valid() {
        let mut registry = ServiceRegistry::new();
        register(&mut registry);
        let tools = registry.list_tools();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name.as_ref(), "permissions_status");
    }
}
