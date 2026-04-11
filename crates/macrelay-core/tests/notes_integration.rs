// Real macOS integration tests for the notes service.
//
// These tests hit the actual Notes app via AppleScript. They are gated with
// #[ignore] and only run under:
//
//     cargo test -p macrelay-core --test notes_integration -- --ignored
//
// Requirements:
//   - macOS host
//   - Notes.app accessible
//   - "Automation" permission for the test binary to control Notes
//     (first run will prompt; System Settings > Privacy & Security > Automation)
//
// Each test uses a UUID-like timestamp suffix in the note title so reruns
// don't collide. Tests clean up after themselves; on failure, stray notes may
// be left in Recently Deleted and will auto-purge.

use std::collections::HashMap;

use macrelay_core::registry::ServiceRegistry;
use macrelay_core::services::notes;
use rmcp::model::CallToolResult;
use serde_json::{Value, json};

fn registry() -> ServiceRegistry {
    let mut r = ServiceRegistry::new();
    notes::register(&mut r);
    r
}

fn args(v: Value) -> HashMap<String, Value> {
    match v {
        Value::Object(m) => m.into_iter().collect(),
        _ => HashMap::new(),
    }
}

fn result_text(r: &CallToolResult) -> String {
    r.content
        .iter()
        .filter_map(|c| c.as_text().map(|t| t.text.clone()))
        .collect::<Vec<_>>()
        .join("\n")
}

fn unique_title(tag: &str) -> String {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    format!("macrelay-it-{tag}-{ts}")
}

async fn cleanup(reg: &ServiceRegistry, title: &str) {
    let _ = reg
        .call_tool("notes_delete_note", args(json!({ "name": title })))
        .await;
}

#[tokio::test]
#[ignore]
async fn write_note_default_account_creates_and_is_searchable() {
    let reg = registry();
    let title = unique_title("write-default");

    let result = reg
        .call_tool(
            "notes_write_note",
            args(json!({ "title": &title, "body": "integration body" })),
        )
        .await
        .expect("write_note handler returned Err");

    let text = result_text(&result);
    assert_ne!(result.is_error, Some(true), "write failed: {text}");
    assert!(text.contains(&title), "expected title in result: {text}");
    assert!(
        text.contains("account:"),
        "expected account attribution in default-account result: {text}"
    );

    // Verify the note is actually searchable.
    let search = reg
        .call_tool("notes_search_notes", args(json!({ "query": &title })))
        .await
        .expect("search_notes handler returned Err");
    let search_text = result_text(&search);
    assert!(
        search_text.contains(&title),
        "search did not surface the created note: {search_text}"
    );

    cleanup(&reg, &title).await;
}

#[tokio::test]
#[ignore]
async fn write_note_with_bogus_account_returns_clear_guided_error() {
    let reg = registry();
    let title = unique_title("write-bogus-acct");

    let result = reg
        .call_tool(
            "notes_write_note",
            args(json!({
                "title": &title,
                "body": "body",
                "account": "ThisAccountDoesNotExist-xyzzy"
            })),
        )
        .await
        .expect("handler returned Err");

    assert_eq!(
        result.is_error,
        Some(true),
        "bogus account should produce is_error=true"
    );
    let text = result_text(&result);
    assert!(
        text.contains("Account not found") || text.contains("not found"),
        "error should name the missing account: {text}"
    );
    assert!(
        text.contains("notes_list_accounts"),
        "error should point user at notes_list_accounts for discovery: {text}"
    );
}

#[tokio::test]
#[ignore]
async fn write_note_with_explicit_icloud_account_attributes_correctly() {
    // On a Mac without iCloud Notes this test expects a clear error instead.
    let reg = registry();
    let title = unique_title("write-icloud");

    let result = reg
        .call_tool(
            "notes_write_note",
            args(json!({
                "title": &title,
                "body": "body",
                "account": "iCloud"
            })),
        )
        .await
        .expect("handler returned Err");

    let text = result_text(&result);

    if result.is_error == Some(true) {
        assert!(
            text.contains("Account not found") || text.contains("Folder"),
            "if iCloud is unavailable, error should explain: {text}"
        );
    } else {
        assert!(
            text.contains("iCloud"),
            "success result should attribute to iCloud: {text}"
        );
        assert!(text.contains(&title));
        cleanup(&reg, &title).await;
    }
}

#[tokio::test]
#[ignore]
async fn write_delete_restore_round_trip_end_to_end() {
    let reg = registry();
    let title = unique_title("round-trip");

    let created = reg
        .call_tool(
            "notes_write_note",
            args(json!({ "title": &title, "body": "round trip" })),
        )
        .await
        .expect("write err");
    assert_ne!(
        created.is_error,
        Some(true),
        "write should succeed: {}",
        result_text(&created)
    );

    let deleted = reg
        .call_tool("notes_delete_note", args(json!({ "name": &title })))
        .await
        .expect("delete err");
    assert_ne!(
        deleted.is_error,
        Some(true),
        "delete should succeed: {}",
        result_text(&deleted)
    );

    let restored = reg
        .call_tool("notes_restore_note", args(json!({ "name": &title })))
        .await
        .expect("restore err");
    let text = result_text(&restored);
    assert_ne!(
        restored.is_error,
        Some(true),
        "restore should succeed: {text}"
    );
    assert!(
        text.contains(&title),
        "restore result should name the note: {text}"
    );
    assert!(
        text.contains("account:"),
        "restore result should include account attribution: {text}"
    );

    cleanup(&reg, &title).await;
}
