// Real Calendar.app integration tests.
//
// Run with:
//     cargo test -p macrelay-core --test calendar_integration -- --ignored --test-threads=1
//
// Requirements:
//   - macOS, Calendar.app set up
//   - Automation permission granted for osascript -> Calendar
//     (System Settings > Privacy & Security > Automation). First run will
//     prompt; click Allow.
//
// Every created event uses a unique timestamped title under the "macrelay-it-"
// prefix, scheduled ~1 year in the future to avoid polluting near-term views.
// Cleanup is best-effort.

mod common;
use common::*;

use macrelay_core::services::calendar;
use serde_json::json;

fn reg() -> macrelay_core::registry::ServiceRegistry {
    registry_with(calendar::register)
}

/// Returns (start, end) Unix-epoch second strings for a 1-hour slot roughly
/// one year from now. Far enough out that these test events don't clutter
/// the user's working calendar view.
fn far_future_slot() -> (String, String) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let one_year = 365u64 * 24 * 60 * 60;
    let start = now + one_year;
    let end = start + 3600;
    (start.to_string(), end.to_string())
}

#[tokio::test]
#[ignore]
async fn list_calendars_returns_at_least_one() {
    let r = reg();
    let result = call_ok(&r, "pim_calendar_list_calendars", json!({})).await;
    let text = result_text(&result);
    assert!(
        !text.trim().is_empty(),
        "list_calendars returned empty output: {text}"
    );
}

#[tokio::test]
#[ignore]
async fn search_events_does_not_leak_applescript_crash() {
    // This exercises the same `repeat with c in calendars` pattern that caused
    // the notes_restore -1728 bug. Using a harmless query that should return
    // nothing — we care about the call not crashing, not about the results.
    let r = reg();
    let result = call(
        &r,
        "pim_calendar_search_events",
        json!({ "query": "macrelay-cal-smoke-zzzz-nonexistent-xyzzy" }),
    )
    .await;
    let text = result_text(&result);
    assert!(
        !text.contains("AppleScript error") && !text.contains("execution error"),
        "search_events leaked an AppleScript crash: {text}"
    );
}

#[tokio::test]
#[ignore]
async fn create_search_cancel_round_trip() {
    let r = reg();
    let title = unique_tag("cal-create-search-cancel");
    let (start, end) = far_future_slot();

    let created = call_ok(
        &r,
        "pim_calendar_create_event",
        json!({
            "title": &title,
            "start_date": &start,
            "end_date": &end,
            "notes": "macrelay integration test"
        }),
    )
    .await;
    let created_text = result_text(&created);
    assert!(
        created_text.contains(&title),
        "create response should name the event: {created_text}"
    );

    let search = call_ok(
        &r,
        "pim_calendar_search_events",
        json!({
            "query": &title,
            "start_date": &start,
            "end_date": &end
        }),
    )
    .await;
    assert!(
        result_text(&search).contains(&title),
        "search did not surface created event: {}",
        result_text(&search)
    );

    best_effort(&r, "pim_calendar_cancel_event", json!({ "title": &title })).await;
}

#[tokio::test]
#[ignore]
async fn create_reschedule_cancel_round_trip() {
    let r = reg();
    let title = unique_tag("cal-reschedule");
    let (start, end) = far_future_slot();

    call_ok(
        &r,
        "pim_calendar_create_event",
        json!({
            "title": &title,
            "start_date": &start,
            "end_date": &end
        }),
    )
    .await;

    // Move it 2 hours forward
    let new_start: u64 = start.parse::<u64>().unwrap() + 7200;
    let new_end: u64 = new_start + 3600;

    let rescheduled = call_ok(
        &r,
        "pim_calendar_reschedule_event",
        json!({
            "title": &title,
            "new_start_date": new_start.to_string(),
            "new_end_date": new_end.to_string()
        }),
    )
    .await;
    let text = result_text(&rescheduled);
    assert!(
        text.contains(&title) || text.to_lowercase().contains("reschedule"),
        "reschedule should acknowledge the event: {text}"
    );

    best_effort(&r, "pim_calendar_cancel_event", json!({ "title": &title })).await;
}

#[tokio::test]
#[ignore]
async fn create_update_cancel_round_trip() {
    let r = reg();
    let title = unique_tag("cal-update");
    let (start, end) = far_future_slot();

    call_ok(
        &r,
        "pim_calendar_create_event",
        json!({
            "title": &title,
            "start_date": &start,
            "end_date": &end,
            "notes": "original notes"
        }),
    )
    .await;

    let updated = call_ok(
        &r,
        "pim_calendar_update_event",
        json!({
            "title": &title,
            "notes": "updated notes via integration test"
        }),
    )
    .await;
    let text = result_text(&updated);
    assert!(
        text.contains(&title) || text.to_lowercase().contains("updat"),
        "update should acknowledge the event: {text}"
    );

    best_effort(&r, "pim_calendar_cancel_event", json!({ "title": &title })).await;
}
