// Real Contacts.app integration tests.
//
// Run with:
//     cargo test -p macrelay-core --test contacts_integration -- --ignored --test-threads=1
//
// Requirements:
//   - macOS, Contacts.app set up
//   - Automation permission granted for osascript -> Contacts

mod common;
use common::*;

use macrelay_core::services::contacts;
use serde_json::json;

fn reg() -> macrelay_core::registry::ServiceRegistry {
    registry_with(contacts::register)
}

#[tokio::test]
#[ignore]
async fn contacts_get_all_returns_results() {
    let r = reg();
    let result = call_ok(&r, "pim_contacts_get_all", json!({})).await;
    let text = result_text(&result);

    // We expect to find at least one contact (usually the user's "Me" card)
    assert!(
        text.contains("| Emails:") || text.contains("No contacts found"),
        "pim_contacts_get_all returned unexpected output: {}",
        text
    );
}

#[tokio::test]
#[ignore]
async fn contacts_search_runs_without_panic() {
    let r = reg();
    // Search for a common letter.
    let result = r
        .call_tool("pim_contacts_search", args(json!({ "query": "a" })))
        .await;

    match result {
        Ok(res) => {
            let text = result_text(&res);
            assert!(
                text.contains("| Emails:") || text.contains("No contacts found"),
                "pim_contacts_search returned unexpected output: {}",
                text
            );
        }
        Err(e) => panic!("Tool call failed: {}", e),
    }
}
