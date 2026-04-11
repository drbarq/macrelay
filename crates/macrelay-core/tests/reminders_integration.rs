// Real Reminders.app integration tests.
//
// Run with:
//     cargo test -p macrelay-core --test reminders_integration -- --ignored --test-threads=1
//
// Requirements:
//   - macOS, Reminders.app set up
//   - Automation permission granted for osascript -> Reminders
//
// Cleanup is best-effort.

mod common;
use common::*;

use macrelay_core::services::reminders;
use serde_json::json;

fn reg() -> macrelay_core::registry::ServiceRegistry {
    registry_with(reminders::register)
}

#[tokio::test]
#[ignore]
async fn list_reminders_lists_returns_at_least_one() {
    let r = reg();
    let result = call_ok(&r, "reminders_list_lists", json!({})).await;
    let text = result_text(&result);
    assert!(
        !text.trim().is_empty() && text.contains("Found"),
        "list_lists returned unexpected output: {text}"
    );
}

#[tokio::test]
#[ignore]
async fn create_search_complete_delete_round_trip() {
    let r = reg();
    let title = unique_tag("rem-it");

    // 1. Create
    let created = call_ok(
        &r,
        "reminders_create_reminder",
        json!({
            "title": &title,
            "notes": "macrelay integration test"
        }),
    )
    .await;
    assert!(result_text(&created).contains(&title));

    // 2. Search
    let search = call_ok(&r, "reminders_search_reminders", json!({ "query": &title })).await;
    assert!(result_text(&search).contains(&title));

    // 3. Complete
    let completed = call_ok(
        &r,
        "reminders_complete_reminder",
        json!({ "title": &title }),
    )
    .await;
    assert!(
        result_text(&completed).to_lowercase().contains("complete")
            || result_text(&completed).contains(&title)
    );

    // 4. Delete
    let deleted = call_ok(&r, "reminders_delete_reminder", json!({ "title": &title })).await;
    assert!(
        result_text(&deleted).to_lowercase().contains("deleted")
            || result_text(&deleted).contains(&title)
    );
}

#[tokio::test]
#[ignore]
async fn create_update_delete_round_trip() {
    let r = reg();
    let title = unique_tag("rem-update");
    let new_title = format!("{}-updated", title);

    call_ok(&r, "reminders_create_reminder", json!({ "title": &title })).await;

    let updated = call_ok(
        &r,
        "reminders_update_reminder",
        json!({
            "title": &title,
            "new_title": &new_title,
            "priority": "high"
        }),
    )
    .await;
    assert!(result_text(&updated).contains(&title) || result_text(&updated).contains("Updated"));

    best_effort(
        &r,
        "reminders_delete_reminder",
        json!({ "title": &new_title }),
    )
    .await;
    best_effort(&r, "reminders_delete_reminder", json!({ "title": &title })).await;
}
