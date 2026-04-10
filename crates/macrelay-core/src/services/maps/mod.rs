use std::sync::Arc;

use rmcp::model::Tool;
use serde_json::json;

use crate::registry::{error_result, schema_from_json, text_result, ServiceRegistry, ToolHandler};

/// Register all maps tools with the service registry.
pub fn register(registry: &mut ServiceRegistry) {
    registry.register(
        "map_search_places",
        Tool::new(
            "map_search_places",
            "Search for places or businesses using Apple Maps. Opens Maps.app with the search query and returns results.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query (e.g. 'coffee shops near me', 'gas stations in Austin TX')."
                    }
                },
                "required": ["query"]
            })),
        ),
        handler_search_places(),
    );

    registry.register(
        "map_get_directions",
        Tool::new(
            "map_get_directions",
            "Get directions between two locations using Apple Maps. Opens Maps.app with turn-by-turn directions displayed.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "origin": {
                        "type": "string",
                        "description": "Starting address or place name. Use 'Current Location' for the user's current position."
                    },
                    "destination": {
                        "type": "string",
                        "description": "Destination address or place name."
                    },
                    "transport_type": {
                        "type": "string",
                        "description": "Mode of transport: 'driving', 'walking', or 'transit'. Default is 'driving'.",
                        "enum": ["driving", "walking", "transit"]
                    }
                },
                "required": ["origin", "destination"]
            })),
        ),
        handler_get_directions(),
    );

    registry.register(
        "map_explore_places",
        Tool::new(
            "map_explore_places",
            "Explore nearby points of interest by category using Apple Maps. Opens Maps.app filtered to the specified category.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "category": {
                        "type": "string",
                        "description": "Category of places to explore (e.g. 'restaurant', 'cafe', 'gas station', 'hotel', 'pharmacy', 'grocery', 'parking')."
                    },
                    "query": {
                        "type": "string",
                        "description": "Optional additional search terms to refine results (e.g. 'Italian' for restaurants, 'EV charging' for gas stations)."
                    }
                },
                "required": ["category"]
            })),
        ),
        handler_explore_places(),
    );

    registry.register(
        "map_calculate_eta",
        Tool::new(
            "map_calculate_eta",
            "Calculate estimated travel time between two locations. Opens Apple Maps with directions so the ETA is displayed in the app.",
            schema_from_json(json!({
                "type": "object",
                "properties": {
                    "origin": {
                        "type": "string",
                        "description": "Starting address or place name. Use 'Current Location' for the user's current position."
                    },
                    "destination": {
                        "type": "string",
                        "description": "Destination address or place name."
                    }
                },
                "required": ["origin", "destination"]
            })),
        ),
        handler_calculate_eta(),
    );
}

/// URL-encode a string for use in maps:// URLs.
/// Encodes spaces and common special characters that would break the URL.
fn url_encode(s: &str) -> String {
    let mut result = String::with_capacity(s.len() * 2);
    for ch in s.chars() {
        match ch {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => result.push(ch),
            ' ' => result.push_str("%20"),
            '&' => result.push_str("%26"),
            '=' => result.push_str("%3D"),
            '+' => result.push_str("%2B"),
            '#' => result.push_str("%23"),
            '\'' => result.push_str("%27"),
            '"' => result.push_str("%22"),
            _ => {
                // Percent-encode other characters as UTF-8 bytes
                for byte in ch.to_string().as_bytes() {
                    result.push_str(&format!("%{:02X}", byte));
                }
            }
        }
    }
    result
}

fn handler_search_places() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let query = args
                .get("query")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("query is required"))?;

            let encoded_query = url_encode(query);
            let script = format!(
                r#"do shell script "open 'maps://?q={encoded_query}'"
return "Opened Apple Maps with search: {}"#,
                query.replace('"', "\\\"")
            );

            match crate::macos::applescript::run_applescript(&script) {
                Ok(result) => Ok(text_result(result)),
                Err(e) => Ok(error_result(format!("Failed to search places: {e}"))),
            }
        })
    })
}

fn handler_get_directions() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let origin = args
                .get("origin")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("origin is required"))?;

            let destination = args
                .get("destination")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("destination is required"))?;

            let transport_type = args
                .get("transport_type")
                .and_then(|v| v.as_str())
                .unwrap_or("driving");

            // Maps URL direction flags: d = driving, w = walking, r = transit
            let dir_flag = match transport_type {
                "walking" => "w",
                "transit" => "r",
                _ => "d", // driving is default
            };

            let encoded_origin = url_encode(origin);
            let encoded_dest = url_encode(destination);

            let script = format!(
                r#"do shell script "open 'maps://?saddr={encoded_origin}&daddr={encoded_dest}&dirflg={dir_flag}'"
return "Opened Apple Maps with {} directions from {} to {}"#,
                transport_type,
                origin.replace('"', "\\\""),
                destination.replace('"', "\\\"")
            );

            match crate::macos::applescript::run_applescript(&script) {
                Ok(result) => Ok(text_result(result)),
                Err(e) => Ok(error_result(format!("Failed to get directions: {e}"))),
            }
        })
    })
}

fn handler_explore_places() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let category = args
                .get("category")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("category is required"))?;

            let additional_query = args
                .get("query")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            // Combine category with optional query for more refined search
            let search_term = if additional_query.is_empty() {
                category.to_string()
            } else {
                format!("{additional_query} {category}")
            };

            let encoded_search = url_encode(&search_term);

            let script = format!(
                r#"do shell script "open 'maps://?q={encoded_search}'"
return "Opened Apple Maps exploring nearby: {}"#,
                search_term.replace('"', "\\\"")
            );

            match crate::macos::applescript::run_applescript(&script) {
                Ok(result) => Ok(text_result(result)),
                Err(e) => Ok(error_result(format!("Failed to explore places: {e}"))),
            }
        })
    })
}

fn handler_calculate_eta() -> ToolHandler {
    Arc::new(|args| {
        Box::pin(async move {
            let origin = args
                .get("origin")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("origin is required"))?;

            let destination = args
                .get("destination")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("destination is required"))?;

            let encoded_origin = url_encode(origin);
            let encoded_dest = url_encode(destination);

            // Open directions in driving mode; Maps.app displays the ETA
            let script = format!(
                r#"do shell script "open 'maps://?saddr={encoded_origin}&daddr={encoded_dest}&dirflg=d'"
return "Opened Apple Maps with directions from {} to {}. The estimated travel time (ETA) is displayed in the Maps app."#,
                origin.replace('"', "\\\""),
                destination.replace('"', "\\\"")
            );

            match crate::macos::applescript::run_applescript(&script) {
                Ok(result) => Ok(text_result(result)),
                Err(e) => Ok(error_result(format!("Failed to calculate ETA: {e}"))),
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
        assert_eq!(tools.len(), 4, "Expected exactly 4 maps tools");

        let names: Vec<_> = tools.iter().map(|t| t.name.as_ref()).collect();
        assert!(names.contains(&"map_search_places"));
        assert!(names.contains(&"map_get_directions"));
        assert!(names.contains(&"map_explore_places"));
        assert!(names.contains(&"map_calculate_eta"));
    }

    #[test]
    fn test_url_encode_basic() {
        assert_eq!(url_encode("hello world"), "hello%20world");
        assert_eq!(url_encode("coffee&tea"), "coffee%26tea");
        assert_eq!(url_encode("simple"), "simple");
    }

    #[test]
    fn test_url_encode_special_chars() {
        assert_eq!(url_encode("a=b"), "a%3Db");
        assert_eq!(url_encode("foo+bar"), "foo%2Bbar");
        assert_eq!(url_encode("hash#tag"), "hash%23tag");
    }
}
