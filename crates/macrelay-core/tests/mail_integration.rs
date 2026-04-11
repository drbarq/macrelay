// Real Mail.app integration tests.
//
// Run with:
//     cargo test -p macrelay-core --test mail_integration -- --ignored --test-threads=1
//
// Requirements:
//   - macOS, Mail.app set up
//   - Automation permission granted for osascript -> Mail
//   - Full Disk Access granted for macrelay (to read Mail SQLite index)
//
// Cleanup is best-effort.

mod common;
use common::*;

use macrelay_core::services::mail;
use serde_json::json;

fn reg() -> macrelay_core::registry::ServiceRegistry {
    registry_with(mail::register)
}

#[tokio::test]
#[ignore]
async fn list_mail_accounts_returns_at_least_one() {
    let r = reg();
    let result = call_ok(&r, "mail_list_accounts", json!({})).await;
    let text = result_text(&result);
    assert!(
        !text.trim().is_empty() && text.contains("Found"),
        "list_accounts returned unexpected output: {text}"
    );
}

#[tokio::test]
#[ignore]
async fn list_mailboxes_returns_inbox() {
    let r = reg();
    let result = call_ok(&r, "mail_list_mailboxes", json!({})).await;
    let text = result_text(&result);
    assert!(
        text.to_lowercase().contains("inbox"),
        "list_mailboxes should return INBOX: {text}"
    );
}

#[tokio::test]
#[ignore]
async fn compose_search_delete_round_trip() {
    let r = reg();
    let subject = unique_tag("mail-it");

    // 1. Compose (Draft)
    let created = call_ok(
        &r,
        "mail_compose_message",
        json!({
            "to": "test@example.com",
            "subject": &subject,
            "body": "macrelay integration test"
        }),
    )
    .await;
    assert!(result_text(&created).contains("composed"));

    // 2. Search
    // Since composing a message via AppleScript in Mail often puts it in 'Drafts',
    // we search Drafts or INBOX.
    let _search = call_ok(
        &r,
        "mail_search_messages",
        json!({ "subject": &subject, "mailbox": "Drafts" }),
    )
    .await;

    // 3. Delete (Best Effort)
    // We attempt deletion regardless of whether search found it, to ensure no leaks.
    best_effort(
        &r,
        "mail_delete_message",
        json!({ "subject": &subject, "mailbox": "Drafts" }),
    )
    .await;
}
