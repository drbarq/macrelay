use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use rmcp::model::{CallToolResult, Content, Tool};
use serde_json::Value;

/// A handler function that processes a tool call and returns a result.
pub type ToolHandler = Arc<
    dyn Fn(
            HashMap<String, Value>,
        ) -> Pin<Box<dyn Future<Output = Result<CallToolResult, anyhow::Error>> + Send>>
        + Send
        + Sync,
>;

/// A registered tool with its MCP schema and handler.
pub struct RegisteredTool {
    pub tool: Tool,
    pub handler: ToolHandler,
}

/// Registry holding all services and their tools.
pub struct ServiceRegistry {
    tools: HashMap<String, RegisteredTool>,
}

impl Default for ServiceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ServiceRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Register a tool with its handler.
    pub fn register(&mut self, name: impl Into<String>, tool: Tool, handler: ToolHandler) {
        let name = name.into();
        self.tools.insert(name, RegisteredTool { tool, handler });
    }

    /// Get all registered tool schemas for ListTools, sorted alphabetically.
    pub fn list_tools(&self) -> Vec<Tool> {
        let mut tools: Vec<Tool> = self.tools.values().map(|rt| rt.tool.clone()).collect();
        tools.sort_by(|a, b| a.name.cmp(&b.name));
        tools
    }

    /// Call a tool by name with the given arguments.
    pub async fn call_tool(
        &self,
        name: &str,
        arguments: HashMap<String, Value>,
    ) -> Result<CallToolResult, anyhow::Error> {
        let registered = self
            .tools
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("Unknown tool: {name}"))?;
        (registered.handler)(arguments).await
    }
}

/// Helper to build a Tool input_schema from a serde_json::Value.
/// The Value must be a JSON object (e.g., from json!({"type": "object", ...})).
pub fn schema_from_json(value: Value) -> Arc<serde_json::Map<String, Value>> {
    match value {
        Value::Object(map) => Arc::new(map),
        _ => Arc::new(serde_json::Map::new()),
    }
}

/// Helper to create a text-only CallToolResult.
pub fn text_result(text: impl Into<String>) -> CallToolResult {
    CallToolResult::success(vec![Content::text(text)])
}

/// Helper to create an error CallToolResult.
pub fn error_result(text: impl Into<String>) -> CallToolResult {
    CallToolResult::error(vec![Content::text(text)])
}
