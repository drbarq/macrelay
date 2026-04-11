// Shared helpers for integration tests.
//
// Usage from a test file:
//
//     mod common;
//     use common::*;
//
// Integration tests are gated behind #[ignore] and run with:
//     cargo test -p macrelay-core --test <name> -- --ignored --test-threads=1

#![allow(dead_code)] // Each test file only uses a subset of these helpers.

use std::collections::HashMap;

use macrelay_core::registry::ServiceRegistry;
use rmcp::model::CallToolResult;
use serde_json::{Value, json};

/// Build a fresh registry populated with only the services a test needs.
pub fn registry_with<F>(register_fn: F) -> ServiceRegistry
where
    F: FnOnce(&mut ServiceRegistry),
{
    let mut r = ServiceRegistry::new();
    register_fn(&mut r);
    r
}

/// Build a registry with every service registered. Use for smoke tests
/// that need to enumerate the full tool surface.
pub fn registry_all() -> ServiceRegistry {
    let mut r = ServiceRegistry::new();
    macrelay_core::services::calendar::register(&mut r);
    macrelay_core::services::reminders::register(&mut r);
    macrelay_core::services::contacts::register(&mut r);
    macrelay_core::services::permissions_status::register(&mut r);
    macrelay_core::services::notes::register(&mut r);
    macrelay_core::services::mail::register(&mut r);
    macrelay_core::services::messages::register(&mut r);
    macrelay_core::services::location::register(&mut r);
    macrelay_core::services::maps::register(&mut r);
    macrelay_core::services::ui_viewer::register(&mut r);
    macrelay_core::services::ui_controller::register(&mut r);
    macrelay_core::services::stickies::register(&mut r);
    macrelay_core::services::shortcuts::register(&mut r);
    r
}

/// Coerce a serde_json object into the HashMap shape the registry expects.
pub fn args(v: Value) -> HashMap<String, Value> {
    match v {
        Value::Object(m) => m.into_iter().collect(),
        _ => HashMap::new(),
    }
}

/// Concatenate all text content blocks from a CallToolResult. Sufficient
/// for substring assertions; no need to wrestle with rmcp's Annotated types.
pub fn result_text(r: &CallToolResult) -> String {
    r.content
        .iter()
        .filter_map(|c| c.as_text().map(|t| t.text.clone()))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Returns true if the result is flagged as an error.
pub fn is_err(r: &CallToolResult) -> bool {
    r.is_error == Some(true)
}

/// Returns true if the result is a successful (non-error) response.
pub fn is_ok(r: &CallToolResult) -> bool {
    r.is_error != Some(true)
}

/// Generate a unique identifier with a human-readable tag prefix.
/// Collisions across runs are prevented by the timestamp.
pub fn unique_tag(tag: &str) -> String {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    format!("macrelay-it-{tag}-{ts}")
}

/// Call a tool by name, returning a CallToolResult. Panics on dispatch
/// error (distinct from a tool-reported error).
pub async fn call(reg: &ServiceRegistry, name: &str, arguments: Value) -> CallToolResult {
    reg.call_tool(name, args(arguments))
        .await
        .unwrap_or_else(|e| panic!("call_tool({name}) dispatch error: {e}"))
}

/// Convenience: call a tool and assert the result is non-error.
pub async fn call_ok(reg: &ServiceRegistry, name: &str, arguments: Value) -> CallToolResult {
    let r = call(reg, name, arguments).await;
    assert!(
        is_ok(&r),
        "expected {name} to succeed but got error: {}",
        result_text(&r)
    );
    r
}

/// Convenience: call a tool and assert the result IS an error.
pub async fn call_err(reg: &ServiceRegistry, name: &str, arguments: Value) -> CallToolResult {
    let r = call(reg, name, arguments).await;
    assert!(
        is_err(&r),
        "expected {name} to error but it succeeded: {}",
        result_text(&r)
    );
    r
}

/// Best-effort cleanup helper — ignores errors. Useful in test teardown
/// where the cleanup itself might fail (e.g. item already gone).
pub async fn best_effort(reg: &ServiceRegistry, name: &str, arguments: Value) {
    let _ = reg.call_tool(name, args(arguments)).await;
}

/// Build a json!({}) argument helper for readability.
pub fn empty() -> Value {
    json!({})
}
