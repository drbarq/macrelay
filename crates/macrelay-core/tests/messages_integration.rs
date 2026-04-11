// Real Messages (iMessage/SMS) integration tests.
//
// These tests are READ-ONLY. They query the local chat.db but never send messages.
//
// Run with:
//     cargo test -p macrelay-core --test messages_integration -- --ignored --test-threads=1
//
// Requirements:
//   - macOS, Messages set up
//   - Full Disk Access granted for the terminal/binary (to read ~/Library/Messages/chat.db)
//
// These tests may return "No chats found" if the database is empty or inaccessible,
// which is considered a graceful pass if the permission check handles it.

mod common;
use common::*;

use macrelay_core::services::messages;
use serde_json::json;

fn reg() -> macrelay_core::registry::ServiceRegistry {
    registry_with(messages::register)
}

#[tokio::test]
#[ignore]
async fn search_chats_runs_without_panic() {
    let r = reg();
    // We search for a very common character or name.
    // Even if it returns "No chats found", we're testing the SQLite connection logic.
    let result = r
        .call_tool(
            "communication_messages_search_chats",
            args(json!({ "query": "a", "limit": 1 })),
        )
        .await;

    match result {
        Ok(res) => {
            let text = result_text(&res);
            assert!(
                text.contains("Found")
                    || text.contains("No chats found")
                    || text.contains("Permission required"),
                "Unexpected output from search_chats: {}",
                text
            );
        }
        Err(e) => panic!("Tool call failed at protocol level: {}", e),
    }
}

#[tokio::test]
#[ignore]
async fn search_messages_runs_without_panic() {
    let r = reg();
    let result = r
        .call_tool(
            "communication_messages_search_messages",
            args(json!({ "query": "hello", "limit": 1 })),
        )
        .await;

    match result {
        Ok(res) => {
            let text = result_text(&res);
            // We're checking that the SQL executed and the parser didn't crash.
            assert!(
                text.contains("Found")
                    || text.contains("No messages found")
                    || text.contains("Permission required"),
                "Unexpected output from search_messages: {}",
                text
            );
        }
        Err(e) => panic!("Tool call failed at protocol level: {}", e),
    }
}
