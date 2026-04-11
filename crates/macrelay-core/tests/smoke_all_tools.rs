// Tier 1: Universal smoke test.
//
// Calls every registered tool with minimal args and asserts that:
//   1. Every registered tool is catalogued here (no silent coverage gaps).
//   2. Non-destructive tools either succeed or fail gracefully.
//   3. No tool response contains an "AppleScript error" / "execution error"
//      substring — those indicate a handler-level bug (like the stale-
//      reference -1728 we shipped fix for in notes_restore_note).
//
// Mutating / destructive / visibly-side-effecting tools are explicitly
// marked Skip. They should be covered by bespoke Tier 2 integration tests.
//
// Run with:
//     cargo test -p macrelay-core --test smoke_all_tools -- --ignored --test-threads=1

mod common;
use common::*;

use std::collections::HashSet;

use serde_json::{Value, json};

#[derive(Debug, Clone, Copy, PartialEq)]
enum Expect {
    /// Must return is_error != Some(true).
    Success,
    /// Either success or graceful error is acceptable. AppleScript crashes
    /// still fail the test (checked globally).
    Tolerate,
    /// Skipped entirely. Typically mutation, send, or visible side effect.
    Skip,
}

struct ToolCase {
    name: &'static str,
    args: fn() -> Value,
    expect: Expect,
    #[allow(dead_code)]
    note: &'static str, // inline documentation; not read at runtime
}

fn no_args() -> Value {
    json!({})
}

fn dummy_query() -> Value {
    // An unlikely-to-match query so read paths return empty/no-results.
    json!({ "query": "macrelay-smoke-zzzz-nonexistent-xyzzy-9999" })
}

fn cases() -> Vec<ToolCase> {
    use Expect::*;
    vec![
        // ---------------- permissions ----------------
        ToolCase {
            name: "permissions_status",
            args: no_args,
            expect: Success,
            note: "pure status check, no permissions required",
        },

        // ---------------- location ----------------
        ToolCase {
            name: "location_get_current",
            args: no_args,
            expect: Tolerate,
            note: "requires Location Services permission",
        },

        // ---------------- contacts ----------------
        ToolCase {
            name: "contacts_get_all",
            args: no_args,
            expect: Tolerate,
            note: "requires Contacts permission",
        },
        ToolCase {
            name: "contacts_search",
            args: dummy_query,
            expect: Tolerate,
            note: "requires Contacts permission",
        },

        // ---------------- notes (read-only surfaces) ----------------
        ToolCase {
            name: "notes_list_accounts",
            args: no_args,
            expect: Success,
            note: "read-only; covered deeply by notes_integration",
        },
        ToolCase {
            name: "notes_list_folders",
            args: no_args,
            expect: Success,
            note: "read-only",
        },
        ToolCase {
            name: "notes_search_notes",
            args: dummy_query,
            expect: Success,
            note: "read-only, empty result acceptable",
        },
        ToolCase {
            name: "notes_read_note",
            args: || json!({ "name": "macrelay-smoke-nonexistent-note" }),
            expect: Tolerate,
            note: "graceful not-found expected",
        },
        ToolCase {
            name: "notes_write_note",
            args: no_args,
            expect: Skip,
            note: "mutation - covered by notes_integration",
        },
        ToolCase {
            name: "notes_delete_note",
            args: no_args,
            expect: Skip,
            note: "mutation - covered by notes_integration",
        },
        ToolCase {
            name: "notes_restore_note",
            args: no_args,
            expect: Skip,
            note: "mutation - covered by notes_integration",
        },
        ToolCase {
            name: "notes_open_note",
            args: no_args,
            expect: Skip,
            note: "opens Notes.app - visible side effect",
        },

        // ---------------- mail ----------------
        ToolCase {
            name: "mail_list_accounts",
            args: no_args,
            expect: Success,
            note: "read-only",
        },
        ToolCase {
            name: "mail_list_mailboxes",
            args: || json!({ "account": "macrelay-smoke-nonexistent-account" }),
            expect: Tolerate,
            note: "requires account arg; dummy is fine for graceful error",
        },
        ToolCase {
            name: "mail_search_messages",
            args: dummy_query,
            expect: Tolerate,
            note: "read-only but may prompt first-run Mail permission",
        },
        ToolCase {
            name: "mail_get_messages",
            args: || json!({ "subject": "macrelay-smoke-nonexistent-subject" }),
            expect: Tolerate,
            note: "read-only, no match expected",
        },
        ToolCase {
            name: "mail_get_thread",
            args: || json!({ "subject": "macrelay-smoke-nonexistent-subject" }),
            expect: Tolerate,
            note: "read-only, no match expected",
        },
        ToolCase {
            name: "mail_compose_message",
            args: no_args,
            expect: Skip,
            note: "mutation - Tier 2 via save_as_draft",
        },
        ToolCase {
            name: "mail_reply_message",
            args: no_args,
            expect: Skip,
            note: "mutation - Tier 2",
        },
        ToolCase {
            name: "mail_forward_message",
            args: no_args,
            expect: Skip,
            note: "mutation - Tier 2",
        },
        ToolCase {
            name: "mail_update_read_state",
            args: no_args,
            expect: Skip,
            note: "mutation - Tier 2",
        },
        ToolCase {
            name: "mail_move_message",
            args: no_args,
            expect: Skip,
            note: "mutation - Tier 2",
        },
        ToolCase {
            name: "mail_delete_message",
            args: no_args,
            expect: Skip,
            note: "destructive - Tier 2",
        },
        ToolCase {
            name: "mail_open_message",
            args: no_args,
            expect: Skip,
            note: "opens Mail.app - visible side effect",
        },
        ToolCase {
            name: "mail_get_attachment",
            args: no_args,
            expect: Skip,
            note: "filesystem side effect",
        },

        // ---------------- calendar ----------------
        ToolCase {
            name: "calendar_list_calendars",
            args: no_args,
            expect: Success,
            note: "read-only",
        },
        ToolCase {
            name: "calendar_search_events",
            args: dummy_query,
            expect: Tolerate,
            note: "read-only; range defaults may apply",
        },
        ToolCase {
            name: "calendar_find_available_times",
            args: || json!({
                "start_date": "1900000000",
                "end_date": "1900003600"
            }),
            expect: Tolerate,
            note: "read-only, far-future range",
        },
        ToolCase {
            name: "calendar_create_event",
            args: no_args,
            expect: Skip,
            note: "mutation - Tier 2",
        },
        ToolCase {
            name: "calendar_reschedule_event",
            args: no_args,
            expect: Skip,
            note: "mutation - Tier 2",
        },
        ToolCase {
            name: "calendar_cancel_event",
            args: no_args,
            expect: Skip,
            note: "destructive - Tier 2",
        },
        ToolCase {
            name: "calendar_update_event",
            args: no_args,
            expect: Skip,
            note: "mutation - Tier 2",
        },
        ToolCase {
            name: "calendar_open_event",
            args: no_args,
            expect: Skip,
            note: "opens Calendar.app",
        },

        // ---------------- reminders ----------------
        ToolCase {
            name: "reminders_list_lists",
            args: no_args,
            expect: Success,
            note: "read-only",
        },
        ToolCase {
            name: "reminders_search_reminders",
            args: dummy_query,
            expect: Tolerate,
            note: "read-only",
        },
        ToolCase {
            name: "reminders_create_reminder",
            args: no_args,
            expect: Skip,
            note: "mutation - Tier 2",
        },
        ToolCase {
            name: "reminders_complete_reminder",
            args: no_args,
            expect: Skip,
            note: "mutation - Tier 2",
        },
        ToolCase {
            name: "reminders_update_reminder",
            args: no_args,
            expect: Skip,
            note: "mutation - Tier 2",
        },
        ToolCase {
            name: "reminders_delete_reminder",
            args: no_args,
            expect: Skip,
            note: "destructive - Tier 2",
        },
        ToolCase {
            name: "reminders_open_reminder",
            args: no_args,
            expect: Skip,
            note: "opens Reminders.app",
        },

        // ---------------- messages ----------------
        ToolCase {
            name: "messages_search_chats",
            args: dummy_query,
            expect: Tolerate,
            note: "read-only SQLite path",
        },
        ToolCase {
            name: "messages_get_chat",
            args: || json!({ "chat_id": -999_999_999i64 }),
            expect: Tolerate,
            note: "read-only SQLite path, non-existent ROWID",
        },
        ToolCase {
            name: "messages_search_messages",
            args: dummy_query,
            expect: Tolerate,
            note: "read-only SQLite path",
        },
        ToolCase {
            name: "messages_send_message",
            args: no_args,
            expect: Skip,
            note: "sends real iMessage - cannot safely automate",
        },

        // ---------------- stickies ----------------
        ToolCase {
            name: "stickies_list",
            args: no_args,
            expect: Success,
            note: "read-only filesystem scan",
        },
        ToolCase {
            name: "stickies_read",
            args: || json!({ "sticky_id": "macrelay-smoke-nonexistent-sticky" }),
            expect: Tolerate,
            note: "read-only, graceful not-found expected",
        },
        ToolCase {
            name: "stickies_create",
            args: no_args,
            expect: Skip,
            note: "mutation - Tier 2",
        },
        ToolCase {
            name: "stickies_open",
            args: no_args,
            expect: Skip,
            note: "opens Stickies.app",
        },

        // ---------------- shortcuts ----------------
        ToolCase {
            name: "shortcuts_list",
            args: no_args,
            expect: Success,
            note: "read-only CLI call",
        },
        ToolCase {
            name: "shortcuts_get",
            args: || json!({ "name": "macrelay-smoke-nonexistent-shortcut" }),
            expect: Tolerate,
            note: "graceful not-found expected",
        },
        ToolCase {
            name: "shortcuts_run",
            args: no_args,
            expect: Skip,
            note: "runs user shortcut - arbitrary side effects",
        },

        // ---------------- maps ----------------
        // All maps tools open Maps.app via URL scheme. Skip from smoke;
        // manual verification is appropriate.
        ToolCase {
            name: "map_search_places",
            args: no_args,
            expect: Skip,
            note: "opens Maps.app - visible side effect",
        },
        ToolCase {
            name: "map_get_directions",
            args: no_args,
            expect: Skip,
            note: "opens Maps.app",
        },
        ToolCase {
            name: "map_explore_places",
            args: no_args,
            expect: Skip,
            note: "opens Maps.app",
        },
        ToolCase {
            name: "map_calculate_eta",
            args: no_args,
            expect: Skip,
            note: "opens Maps.app",
        },

        // ---------------- ui_viewer ----------------
        ToolCase {
            name: "ui_viewer_list_apps",
            args: no_args,
            expect: Success,
            note: "read-only process list",
        },
        ToolCase {
            name: "ui_viewer_get_frontmost",
            args: no_args,
            expect: Success,
            note: "read-only",
        },
        ToolCase {
            name: "ui_viewer_get_ui_tree",
            args: || json!({ "app_name": "Finder" }),
            expect: Tolerate,
            note: "Finder is always running",
        },
        ToolCase {
            name: "ui_viewer_get_visible_text",
            args: || json!({ "app_name": "Finder" }),
            expect: Tolerate,
            note: "Finder is always running",
        },
        ToolCase {
            name: "ui_viewer_find_elements",
            args: || json!({ "app_name": "Finder", "role": "AXButton" }),
            expect: Tolerate,
            note: "Finder is always running",
        },
        ToolCase {
            name: "ui_viewer_capture_snapshot",
            args: || json!({ "app_name": "Finder" }),
            expect: Tolerate,
            note: "Finder is always running",
        },

        // ---------------- ui_controller ----------------
        // All ui_controller tools drive mouse/keyboard/windows. Smoke-testing
        // them safely is hard (need a target app with known state). All Skip.
        ToolCase {
            name: "ui_controller_click",
            args: no_args,
            expect: Skip,
            note: "mouse input - needs target state",
        },
        ToolCase {
            name: "ui_controller_type_text",
            args: no_args,
            expect: Skip,
            note: "keyboard input - needs target state",
        },
        ToolCase {
            name: "ui_controller_press_key",
            args: no_args,
            expect: Skip,
            note: "keyboard input",
        },
        ToolCase {
            name: "ui_controller_scroll",
            args: no_args,
            expect: Skip,
            note: "mouse input",
        },
        ToolCase {
            name: "ui_controller_drag",
            args: no_args,
            expect: Skip,
            note: "mouse input",
        },
        ToolCase {
            name: "ui_controller_select_menu",
            args: no_args,
            expect: Skip,
            note: "menu activation",
        },
        ToolCase {
            name: "ui_controller_manage_window",
            args: no_args,
            expect: Skip,
            note: "window state mutation",
        },
        ToolCase {
            name: "ui_controller_manage_app",
            args: no_args,
            expect: Skip,
            note: "app lifecycle mutation",
        },
        ToolCase {
            name: "ui_controller_file_dialog",
            args: no_args,
            expect: Skip,
            note: "interacts with modal dialogs",
        },
        ToolCase {
            name: "ui_controller_dock",
            args: no_args,
            expect: Skip,
            note: "dock interaction",
        },
    ]
}

#[tokio::test]
#[ignore]
async fn smoke_every_registered_tool_is_catalogued_and_non_crashing() {
    let reg = registry_all();
    let cases = cases();

    // --- Coverage: every registered tool must appear in the catalogue ---
    let registered: HashSet<String> = reg
        .list_tools()
        .into_iter()
        .map(|t| t.name.to_string())
        .collect();
    let catalog: HashSet<String> = cases.iter().map(|c| c.name.to_string()).collect();

    let uncatalogued: Vec<&String> = registered.difference(&catalog).collect();
    assert!(
        uncatalogued.is_empty(),
        "{} tool(s) registered but missing from smoke catalogue: {:?}. \
         Add them to cases() with an explicit Expect variant.",
        uncatalogued.len(),
        uncatalogued
    );

    let stale: Vec<&String> = catalog.difference(&registered).collect();
    assert!(
        stale.is_empty(),
        "{} tool(s) in smoke catalogue no longer registered: {:?}",
        stale.len(),
        stale
    );

    // --- Execute non-skip cases ---
    let mut failures: Vec<String> = Vec::new();
    let mut permission_gaps: Vec<String> = Vec::new();
    let mut ran = 0usize;
    let mut skipped = 0usize;

    for case in &cases {
        if case.expect == Expect::Skip {
            skipped += 1;
            continue;
        }
        ran += 1;

        let result = match reg.call_tool(case.name, args((case.args)())).await {
            Ok(r) => r,
            Err(e) => {
                failures.push(format!("{}: dispatch error: {e}", case.name));
                continue;
            }
        };
        let text = result_text(&result);

        // Permission errors are environment state, not code bugs. Track them
        // separately so they don't drown out real failures.
        //   -1743  : Not authorized to send Apple events (app-specific)
        //   -25211 : osascript is not allowed assistive access
        //   -1719  : Can't get ... (often accessibility-gated)
        //   -1744  : User denied the Apple event
        let is_permission_gap = text.contains("(-1743)")
            || text.contains("(-25211)")
            || text.contains("(-1744)")
            || text.contains("Not authorized to send Apple events")
            || text.contains("not allowed assistive access");

        if is_permission_gap {
            permission_gaps.push(format!("{}: {}", case.name, first_line(&text)));
            continue;
        }

        // Any other AppleScript-level crash is a hard failure — this is the
        // class that caught the notes_restore -1728 reference-chain bug.
        if text.contains("AppleScript error")
            || text.contains("execution error")
            || text.contains("syntax error")
        {
            failures.push(format!(
                "{}: handler leaked an AppleScript/shell crash -> {text}",
                case.name
            ));
            continue;
        }

        match case.expect {
            Expect::Success => {
                if is_err(&result) {
                    failures.push(format!(
                        "{}: expected Success but got graceful error -> {text}",
                        case.name
                    ));
                }
            }
            Expect::Tolerate => {
                // Any non-crashing response is acceptable.
            }
            Expect::Skip => unreachable!(),
        }
    }

    println!(
        "\nSMOKE SUMMARY: catalogued={} ran={} skipped={} permission_gaps={} failures={}",
        cases.len(),
        ran,
        skipped,
        permission_gaps.len(),
        failures.len()
    );
    if !permission_gaps.is_empty() {
        println!("\nPermission gaps (grant in System Settings > Privacy & Security):");
        for p in &permission_gaps {
            println!("  - {p}");
        }
    }
    if !failures.is_empty() {
        panic!(
            "\nsmoke test failures ({} total):\n  - {}",
            failures.len(),
            failures.join("\n  - ")
        );
    }
}

fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or("").trim().to_string()
}
