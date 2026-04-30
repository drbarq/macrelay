use clap::Parser;
use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use rmcp::handler::server::ServerHandler;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, Content, ListToolsResult, PaginatedRequestParams,
    ServerCapabilities, ServerInfo,
};
use rmcp::service::RequestContext;
use rmcp::service::ServiceExt;
use rmcp::transport::io::stdio;
use rmcp::{ErrorData as McpError, RoleServer};
use serde_json::Value;
use tracing_subscriber::EnvFilter;

use macrelay_core::registry::ServiceRegistry;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Only enable tools for specific services (e.g. 'calendar', 'mail', 'ui').
    /// Can be specified multiple times. If omitted, all services are enabled.
    #[arg(short, long)]
    service: Vec<String>,
}

/// The MCP server handler that routes tool calls to our service registry.
struct MacRelayServer {
    registry: Arc<ServiceRegistry>,
    active_services: Vec<String>,
}

impl ServerHandler for MacRelayServer {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::default();
        let display_name = if self.active_services.contains(&"all".to_string()) {
            "macrelay".to_string()
        } else {
            format!("macrelay-{}", self.active_services.join("-"))
        };

        info.instructions = Some(format!(
            "MacRelay ({}): Open-source MCP server for macOS. \
             Use these tools to interact with your Mac.",
            self.active_services.join(", ")
        ));
        info.server_info.name = display_name;
        info.server_info.version = env!("CARGO_PKG_VERSION").into();
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _cx: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        let tools = self.registry.list_tools();
        Ok(ListToolsResult {
            tools,
            ..Default::default()
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _cx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
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

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    let mut active_services = cli.service;
    if active_services.is_empty() {
        active_services.push("all".to_string());
    }

    tracing::info!(
        "MacRelay v{} starting (services: {})...",
        env!("CARGO_PKG_VERSION"),
        active_services.join(", ")
    );

    let mut registry = ServiceRegistry::new();

    let is_active = |s: &str| {
        active_services.contains(&"all".to_string()) || active_services.contains(&s.to_string())
    };

    // PIM
    if is_active("calendar") {
        macrelay_core::services::calendar::register(&mut registry);
    }
    if is_active("reminders") {
        macrelay_core::services::reminders::register(&mut registry);
    }
    if is_active("contacts") {
        macrelay_core::services::contacts::register(&mut registry);
    }

    // Communication
    if is_active("mail") {
        macrelay_core::services::mail::register(&mut registry);
    }
    if is_active("messages") {
        macrelay_core::services::messages::register(&mut registry);
    }

    // Productivity
    if is_active("notes") {
        macrelay_core::services::notes::register(&mut registry);
    }
    if is_active("stickies") {
        macrelay_core::services::stickies::register(&mut registry);
    }
    if is_active("shortcuts") {
        macrelay_core::services::shortcuts::register(&mut registry);
    }

    // Navigation & Context
    if is_active("location") {
        macrelay_core::services::location::register(&mut registry);
    }
    if is_active("maps") {
        macrelay_core::services::maps::register(&mut registry);
    }

    // UI Automation
    if is_active("ui") {
        macrelay_core::services::ui_viewer::register(&mut registry);
        macrelay_core::services::ui_controller::register(&mut registry);
    }

    // System
    if is_active("system") || is_active("permissions") {
        macrelay_core::services::permissions_status::register(&mut registry);
    }

    let tool_count = registry.list_tools().len();
    if tool_count == 0 {
        tracing::error!(
            "No tools registered for services: {}",
            active_services.join(", ")
        );
        anyhow::bail!("Invalid service names: {}", active_services.join(", "));
    }
    tracing::info!("Registered {tool_count} tools");

    let server = MacRelayServer {
        registry: Arc::new(registry),
        active_services,
    };

    let transport = stdio();
    let service = server.serve(transport).await?;
    service.waiting().await?;

    Ok(())
}
