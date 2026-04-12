# Product Requirements Document: MacRelay

## Current Status (Updated 2026-04-10)

**All phases complete.**

71 tools across 13 services. Test suite refined: **166 tests (137 CI-safe, 29 local-only Tier 3 round-trips)**. Every tool has meaningful validation of script generation, response parsing, error paths, required-param validation, and injection-safe escaping. GitHub Actions CI (`fmt` + `clippy -D warnings` + `test --lib`) runs on every push/PR on `macos-latest`.

| What | Status |
|---|---|
| Calendar (8 tools) | Done |
| Reminders (7 tools) | Done |
| Contacts (2 tools) | Done |
| Permissions (1 tool) | Done |
| Notes (8 tools) | Done |
| Mail (13 tools) | Done |
| Messages (4 tools) | Done |
| Location (1 tool) | Done |
| Maps (4 tools) | Done |
| UI Viewer (6 tools) | Done |
| UI Controller (10 tools) | Done |
| Stickies (4 tools) | Done |
| Shortcuts (3 tools) | Done |
| Install script | Done |
| Unit tests (Tier 1 & 2) | 137 passing in CI (GitHub Actions, `macos-latest`) |
| Integration tests (Tier 3) | 29 local-only tests (TCC-gated, 6 services with round-trip files) |
| GitHub Actions CI | `fmt` + `clippy -D warnings` + `test --lib`, all green |

**Remaining work:** Distribution polish (system tray, Homebrew formula, DMG).

### Test Coverage

The mocking foundation lives in `crates/macrelay-core/src/macos/applescript.rs`:
- `ScriptRunner` trait abstracts AppleScript/JXA execution.
- `MOCK_RUNNER` task-local override (via `tokio::task_local!`) lets tests inject scripted responses.
- `current_runner()` falls back to the real `OsascriptRunner` outside test scopes.

All 71 tools are covered by Tier 2 tests that **inspect the generated script content** to ensure correct command construction and argument escaping. Every tool also has error-path coverage (graceful handling when `osascript` fails) and required-parameter validation (missing args return `Ok(error_result(...))` instead of panicking). Injection-safe escape helpers (`escape_applescript_string`, `escape_jxa_string`, `escape_shell_single_quoted`) live in `crates/macrelay-core/src/macos/escape.rs` and are directly tested at Tier 1.

Tier 1 (pure unit) and Tier 2 (script-inspecting mocks) are CI-safe and run on `macos-latest`. Tier 3 integration tests (`cargo test -- --ignored`) hit real Calendar/Notes/Mail/etc. on the maintainer's Mac and never run in CI. See [docs/TESTING.md](TESTING.md) for the full strategy.

## 1. Overview

MacRelay is an open-source MCP (Model Context Protocol) server for macOS that gives AI assistants access to native Mac apps and universal UI control. Fully local, privacy-first, no cloud, no subscriptions, no telemetry.

## 2. Goals

1. **71 tools across 13 services** - Full native macOS app coverage (DONE)
2. **Non-technical user experience** - Setup script today, DMG + system tray planned for Phase 4
3. **100% local** - No cloud, no telemetry, no subscriptions
4. **Tested and reliable** - Three-tier strategy (pure unit → script-inspecting mocks → real-app integration). All tools covered by meaningful tests.
5. **Easy to contribute to** - Self-contained service modules, standard Rust toolchain

## 3. Technical Approach

MacRelay uses three main patterns to interact with native macOS apps:

- **AppleScript/JXA** — Calendar, Reminders, Contacts, Mail (compose/send), Notes (write), Messages (send), Stickies (create). Executed via `osascript` with injection-safe escaping.
- **SQLite (read-only)** — Messages (`chat.db`), Notes (`NoteStore.sqlite`), Mail (`Envelope Index`). Direct reads are faster than AppleScript and avoid automation prompts.
- **Accessibility API** — UI Viewer and UI Controller use `AXUIElement` for tree inspection and `CGEvent` for mouse/keyboard input. Elements get short ref IDs (B1, T1, S1).

Additional integrations: CoreLocation (via Swift subprocess) for location, `/usr/bin/shortcuts` for Siri Shortcuts, RTFD file reader for Stickies.

## 4. Complete Tool Specifications

### 4.1 Calendar Service (8 tools)

| Tool | Description | Parameters | Notes |
|---|---|---|---|
| `calendar_list_calendars` | List all calendars | none | Returns calendar names, colors, types |
| `calendar_search_events` | Search events | query, start_date, end_date, calendars, limit, offset | Flexible date parsing, ISO 8601 |
| `calendar_create_event` | Create an event | title, start_date, end_date, is_all_day, location, notes, url, calendar | Returns deep link |
| `calendar_reschedule_event` | Reschedule an event | event reference, start_date, end_date, strategy (strict/flexible) | ShiftStrategy enum |
| `calendar_cancel_event` | Cancel/delete an event | event reference or deep_link | Destructive |
| `calendar_update_event` | Update event properties | event reference, any event fields | Partial update |
| `calendar_open_event` | Open event in Calendar.app | event reference or deep_link | Uses ical:// deep link |
| `calendar_find_available_times` | Find free time slots | time_range_start, time_range_end, duration_minutes, buffer_minutes, max_results, calendars, time_range_mode | TimeRangeMode enum |

**Event Reference:** Can identify events by ID, title, or deep link.

### 4.2 Reminders Service (7 tools)

| Tool | Description | Parameters | Notes |
|---|---|---|---|
| `reminders_list_lists` | List reminder lists | none | Returns list names and IDs |
| `reminders_search_reminders` | Search/filter reminders | query, lists, completed, start_date, end_date, limit, offset | Applied filters in response |
| `reminders_create_reminder` | Create a reminder | title, list_id, due_date, notes, priority (none/low/medium/high), duration_minutes | Returns deep link |
| `reminders_update_reminder` | Update reminder properties | reminder reference, any reminder fields | Partial update |
| `reminders_delete_reminder` | Permanently delete | reminder reference | Destructive, cannot be undone |
| `reminders_complete_reminder` | Mark as complete | reminder reference | Soft action |
| `reminders_open_reminder` | Open in Reminders.app | reminder reference or deep_link | Uses x-apple-reminderkit:// |

### 4.3 Notes Service (8 tools)

| Tool | Description | Parameters | Notes |
|---|---|---|---|
| `notes_list_accounts` | List Notes accounts | none | iCloud, On My Mac, etc. |
| `notes_list_folders` | List folders | none | Includes Recently Deleted |
| `notes_search_notes` | Search notes | query, folder, limit, offset | SQLite ZPLAINTEXT search |
| `notes_read_note` | Read full note content | note_id | SQLite direct read |
| `notes_write_note` | Create or update a note | title, body, folder | AppleScript (HTML body) |
| `notes_delete_note` | Move to Recently Deleted | note_id | Recoverable for 30 days |
| `notes_restore_note` | Restore from Recently Deleted | note_id | |
| `notes_open_note` | Open in Notes.app | note_id | |

### 4.4 Mail Service (13 tools)

| Tool | Description | Parameters | Notes |
|---|---|---|---|
| `mail_list_accounts` | List mail accounts | none | |
| `mail_list_mailboxes` | List mailboxes | none | |
| `mail_search_messages` | Search mail | query, mailbox, from, subject, start_date, end_date, limit, offset | SQLite Envelope Index |
| `mail_get_messages` | Get messages by reference | references (by_id array) | |
| `mail_get_thread` | Get full thread | reference | |
| `mail_compose_message` | Compose new email | to, cc, bcc, subject, body, attachments | AppleScript |
| `mail_reply_message` | Reply to message | reference, body, reply_all | AppleScript, saves as draft |
| `mail_forward_message` | Forward message | reference, to, body | AppleScript |
| `mail_update_read_state` | Mark read/unread | references, read | |
| `mail_move_message` | Move to mailbox | references, mailbox | Can move to Trash |
| `mail_delete_message` | Delete message | references | Moves to Trash |
| `mail_open_message` | Open in Mail.app | reference | |
| `mail_get_attachment` | Get attachment | reference, attachment_id | Returns file path or content |

### 4.5 Messages Service (4 tools)

| Tool | Description | Parameters | Notes |
|---|---|---|---|
| `messages_search_chats` | Search conversations | query, service, limit, offset | SQLite chat.db |
| `messages_get_chat` | Get chat messages | chat reference, limit, offset | |
| `messages_search_messages` | Search message text | query, chat_ids, sender, is_from_me, is_read, start_date, end_date, limit, offset | |
| `messages_send_messages` | Send iMessage/SMS | recipients (with message each) | AppleScript automation |

### 4.6 Contacts Service (2 tools)

| Tool | Description | Parameters | Notes |
|---|---|---|---|
| `contacts_search` | Search contacts | query (name, phone, email) | CNContactStore |
| `contacts_get_all` | Get all contacts | none | Full contact list |

### 4.7 Maps Service (4 tools)

| Tool | Description | Parameters | Notes |
|---|---|---|---|
| `map_search_places` | Search locations | query, latitude, longitude, radius | MKLocalSearch |
| `map_get_directions` | Get directions | origin, destination, transport_type (automobile/walking/transit) | MKDirections |
| `map_explore_places` | Nearby POIs | latitude, longitude, category, radius, limit | POICategory enum (airport, restaurant, gym, etc.) |
| `map_calculate_eta` | Travel time | origin coords, destination coords, transport_type | |

### 4.8 Location Service (1 tool)

| Tool | Description | Parameters | Notes |
|---|---|---|---|
| `location_get_current` | Get current location | none | Returns lat/lng, accuracy, altitude |

### 4.9 UI Viewer Service (6 tools)

| Tool | Description | Parameters | Notes |
|---|---|---|---|
| `ui_viewer_list_apps` | List running apps | none | NSWorkspace.shared.runningApplications |
| `ui_viewer_get_frontmost` | Get frontmost app + window | none | NSWorkspace.shared.frontmostApplication |
| `ui_viewer_get_ui_tree` | Get accessibility tree | app reference, scope_xpath, max_depth | Returns XML with ref IDs |
| `ui_viewer_get_visible_text` | Extract visible text | app reference, from_end | Char count + truncation |
| `ui_viewer_find_elements` | XPath query on UI | app reference, xpath | Common: //AXButton, //AXTextField[@AXFocused='true'] |
| `ui_viewer_capture_snapshot` | Screenshot + element map | app reference, save | Returns image + role-based ref IDs |

### 4.10 UI Controller Service (10 tools)

All UI controller tools return a **UI diff** showing what changed after the action.

| Tool | Description | Parameters | Notes |
|---|---|---|---|
| `ui_controller_click` | Click element or coords | ref_id, xpath, text query, or [x,y]; button (left/right); click_count; wait_ms (default 300) | Supports single, double, right click |
| `ui_controller_type_text` | Type into text field | text, clear_before (default true), verify, press_enter, wait_ms, ensure_focused_element_is | |
| `ui_controller_press_key` | Press key combination | keys (e.g., ["command", "s"]), wait_ms (default 100) | Modifier mapping |
| `ui_controller_scroll` | Scroll in app | direction (up/down/left/right), amount (default 3), ref_id/xpath, wait_ms | Distributed scroll for large amounts |
| `ui_controller_drag` | Drag element | from, to (coords or ref_ids) | |
| `ui_controller_select_menu` | Select menu item | menu_path (e.g., ["File", "Save As..."]), wait_ms (default 300) | Titles are localized |
| `ui_controller_manage_window` | Window operations | action (list/close/minimize/restore/fullscreen/focus/move/resize), window index or title | |
| `ui_controller_manage_app` | App lifecycle | action (open/close), app reference, force | |
| `ui_controller_file_dialog` | Drive file dialogs | action (navigate/set_filename/confirm/cancel/select_file), path/filename | Requires open dialog |
| `ui_controller_dock` | Control Dock | action, app_name | |

### 4.11 Stickies Service (4 tools)

| Tool | Description | Parameters | Notes |
|---|---|---|---|
| `stickies_list` | List all stickies | query (optional filter), limit | RTFD file reader |
| `stickies_read` | Read full sticky content | sticky_id (RTFD dir name) | |
| `stickies_create` | Create new sticky | content, color (none/low/medium/high) | JXA + Accessibility |
| `stickies_open` | Open Stickies app | none | |

### 4.12 Shortcuts Service (3 tools)

| Tool | Description | Parameters | Notes |
|---|---|---|---|
| `shortcuts_list` | List installed shortcuts | name (filter), folder | /usr/bin/shortcuts list |
| `shortcuts_get` | Get shortcut details | name | Action count, accepts input, folder |
| `shortcuts_run` | Run a shortcut | name, input (text), timeout_secs (default 30, max 300) | WARNING: executes with real effects |

## 5. Non-Functional Requirements

### 5.1 Performance
- Tool calls should respond in <2 seconds for non-UI operations
- EventKit/Contacts use TTL caching to avoid repeated framework initialization
- SQLite queries use indexes where available

### 5.2 Security
- No data leaves the machine
- No telemetry, analytics, or crash reporting
- Sandboxed where possible; unsandboxed only where necessary (Accessibility, Full Disk Access)
- AppleScript commands are constructed safely (no string interpolation of user input into scripts)

### 5.3 Reliability
- Graceful handling of permission denials with actionable error messages
- Timeout handling for AppleScript execution (60s default)
- SQLite database access is read-only where possible (Messages, Notes reads, Mail reads)
- Service isolation: one service failing doesn't affect others

### 5.4 Compatibility
- macOS 14.0+ (Sonoma and later)
- Universal binary (arm64 + x86_64)
- Works with: Claude Desktop, Cursor, Claude Code, VS Code, Raycast, and any MCP client

## 6. What We Explicitly Won't Build

- Usage tracking or rate limiting
- Licensing or payment system
- Cloud sync or remote access
- OAuth server
- Analytics or crash reporting
- Auto-update (users update via Homebrew or DMG download)
