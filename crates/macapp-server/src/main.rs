use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use rmcp::handler::server::ServerHandler;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, Content, ListToolsResult, PaginatedRequestParams,
    ServerInfo,
};
use rmcp::service::RequestContext;
use rmcp::transport::io::stdio;
use rmcp::service::ServiceExt;
use rmcp::{ErrorData as McpError, RoleServer};
use serde_json::Value;
use tracing_subscriber::EnvFilter;

use macapp_core::registry::ServiceRegistry;

/// The MCP server handler that routes tool calls to our service registry.
struct MacAppServer {
    registry: Arc<ServiceRegistry>,
}

impl ServerHandler for MacAppServer {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::default();
        info.instructions = Some(
            "mac-app-oss: Open-source MCP server for macOS native app integration and universal UI control. \
             Use these tools to interact with Calendar, Reminders, Contacts, and more on this Mac."
                .into(),
        );
        info.server_info.name = "mac-app-oss".into();
        info.server_info.version = env!("CARGO_PKG_VERSION").into();
        info
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _cx: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, McpError>> + Send + '_ {
        async move {
            let tools = self.registry.list_tools();
            Ok(ListToolsResult {
                tools,
                ..Default::default()
            })
        }
    }

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        _cx: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<CallToolResult, McpError>> + Send + '_ {
        async move {
            let name = &request.name;
            let arguments: HashMap<String, Value> = request
                .arguments
                .map(|map| map.into_iter().collect())
                .unwrap_or_default();

            match self.registry.call_tool(name, arguments).await {
                Ok(result) => Ok(result),
                Err(e) => Ok(CallToolResult::error(vec![Content::text(format!(
                    "Error: {e}"
                ))])),
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Set up logging to stderr (stdout is for MCP JSON-RPC)
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    tracing::info!("mac-app-oss v{} starting...", env!("CARGO_PKG_VERSION"));

    // Build the service registry with all tools
    let mut registry = ServiceRegistry::new();

    macapp_core::services::calendar::register(&mut registry);
    macapp_core::services::reminders::register(&mut registry);
    macapp_core::services::contacts::register(&mut registry);
    macapp_core::services::permissions_status::register(&mut registry);

    let tool_count = registry.list_tools().len();
    tracing::info!("Registered {tool_count} tools");

    // Create the server
    let server = MacAppServer {
        registry: Arc::new(registry),
    };

    // Run with stdio transport
    let transport = stdio();
    let service = server.serve(transport).await?;
    service.waiting().await?;

    Ok(())
}
