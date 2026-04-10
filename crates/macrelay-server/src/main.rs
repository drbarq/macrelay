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

use macrelay_core::registry::ServiceRegistry;

/// The MCP server handler that routes tool calls to our service registry.
struct MacRelayServer {
    registry: Arc<ServiceRegistry>,
}

impl ServerHandler for MacRelayServer {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::default();
        info.instructions = Some(
            "MacRelay: Open-source MCP server for macOS. \
             Use these tools to interact with Calendar, Reminders, Contacts, and more on this Mac."
                .into(),
        );
        info.server_info.name = "macrelay".into();
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
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    tracing::info!("MacRelay v{} starting...", env!("CARGO_PKG_VERSION"));

    let mut registry = ServiceRegistry::new();

    // Phase 1
    macrelay_core::services::calendar::register(&mut registry);
    macrelay_core::services::reminders::register(&mut registry);
    macrelay_core::services::contacts::register(&mut registry);
    macrelay_core::services::permissions_status::register(&mut registry);

    // Phase 2
    macrelay_core::services::notes::register(&mut registry);
    macrelay_core::services::mail::register(&mut registry);
    macrelay_core::services::messages::register(&mut registry);
    macrelay_core::services::location::register(&mut registry);
    macrelay_core::services::maps::register(&mut registry);

    // Phase 3
    macrelay_core::services::ui_viewer::register(&mut registry);
    macrelay_core::services::ui_controller::register(&mut registry);

    let tool_count = registry.list_tools().len();
    tracing::info!("Registered {tool_count} tools");

    let server = MacRelayServer {
        registry: Arc::new(registry),
    };

    let transport = stdio();
    let service = server.serve(transport).await?;
    service.waiting().await?;

    Ok(())
}
