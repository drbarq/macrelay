// Real macOS integration tests for the notes service.
//
// Run with:
//     cargo test -p macrelay-core --test notes_integration -- --ignored --test-threads=1
//
// Requirements: macOS, Notes.app accessible, Automation permission for osascript.

mod common;
use common::*;

use macrelay_core::services::notes;
use serde_json::json;

fn reg() -> macrelay_core::registry::ServiceRegistry {
    registry_with(notes::register)
}

#[tokio::test]
#[ignore]
async fn write_note_default_account_creates_and_is_searchable() {
    let r = reg();
    let title = unique_tag("notes-write-default");

    let result = call_ok(
        &r,
        "notes_write_note",
        json!({ "title": &title, "body": "integration body" }),
    )
    .await;
    let text = result_text(&result);
    assert!(text.contains(&title), "expected title in result: {text}");
    assert!(
        text.contains("account:"),
        "expected account attribution: {text}"
    );

    let search = call_ok(&r, "notes_search_notes", json!({ "query": &title })).await;
    assert!(
        result_text(&search).contains(&title),
        "search did not surface the created note: {}",
        result_text(&search)
    );

    best_effort(&r, "notes_delete_note", json!({ "name": &title })).await;
}

#[tokio::test]
#[ignore]
async fn write_note_with_bogus_account_returns_clear_guided_error() {
    let r = reg();
    let title = unique_tag("notes-bogus");

    let result = call_err(
        &r,
        "notes_write_note",
        json!({
            "title": &title,
            "body": "body",
            "account": "ThisAccountDoesNotExist-xyzzy"
        }),
    )
    .await;

    let text = result_text(&result);
    assert!(
        text.contains("Account not found") || text.contains("not found"),
        "error should name the missing account: {text}"
    );
    assert!(
        text.contains("notes_list_accounts"),
        "error should point user at notes_list_accounts: {text}"
    );
}

#[tokio::test]
#[ignore]
async fn write_note_with_explicit_icloud_account_attributes_correctly() {
    // Tolerates Macs without iCloud Notes — in that case expects a guided error.
    let r = reg();
    let title = unique_tag("notes-icloud");

    let result = call(
        &r,
        "notes_write_note",
        json!({ "title": &title, "body": "body", "account": "iCloud" }),
    )
    .await;
    let text = result_text(&result);

    if is_err(&result) {
        assert!(
            text.contains("Account not found") || text.contains("Folder"),
            "if iCloud unavailable, error should explain: {text}"
        );
    } else {
        assert!(text.contains("iCloud"), "should attribute iCloud: {text}");
        assert!(text.contains(&title));
        best_effort(&r, "notes_delete_note", json!({ "name": &title })).await;
    }
}

#[tokio::test]
#[ignore]
async fn write_delete_restore_round_trip_end_to_end() {
    let r = reg();
    let title = unique_tag("notes-round-trip");

    call_ok(
        &r,
        "notes_write_note",
        json!({ "title": &title, "body": "round trip" }),
    )
    .await;

    call_ok(&r, "notes_delete_note", json!({ "name": &title })).await;

    let restored = call_ok(&r, "notes_restore_note", json!({ "name": &title })).await;
    let text = result_text(&restored);
    assert!(
        text.contains(&title),
        "restore result should name the note: {text}"
    );
    assert!(
        text.contains("account:"),
        "restore result should include account attribution: {text}"
    );

    best_effort(&r, "notes_delete_note", json!({ "name": &title })).await;
}
