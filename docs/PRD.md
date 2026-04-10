# Product Requirements Document: MacRelay

## Current Status (Updated 2026-04-10)

**All phases complete. Feature parity with MacUse achieved.**

71 tools across 13 services, 27 tests passing.

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
| Unit tests | 27 passing |

**Remaining work:** Distribution polish (system tray, Homebrew, CI, DMG)

## 1. Overview

MacRelay is an open-source MCP (Model Context Protocol) server for macOS that gives AI assistants access to native Mac apps and universal UI control. It replaces the closed-source MacUse app ($39, macuse.app) with a fully local, privacy-first, open-source alternative.

## 2. Goals

1. **Feature parity with MacUse** - 71 tools across 13 services (DONE)
2. **Non-technical user experience** - Setup script today, DMG + system tray planned for Phase 4
3. **100% local** - No cloud, no telemetry, no subscriptions
4. **Tested and reliable** - Unit tests per service, E2E harness planned for Phase 2
5. **Easy to contribute to** - Self-contained service modules, standard Rust toolchain

## 3. What We Learned from Dissecting MacUse

We extracted the complete architecture from the installed MacUse binary (v1.7.3):

### 3.1 MacUse Tech Stack
- **Framework:** Rust + Tauri v2 (not Swift as the website implies)
- **MCP Library:** `rmcp` crate v0.17.0
- **MCP Protocol:** Versions 2025-06-18 and 2025-11-25
- **Transport:** Streamable HTTP (primary, background daemon) + stdio
- **Licensing:** Keygen.rs (tauri_plugin_keygen_rs2)
- **Analytics:** PostHog + Sentry (we will NOT include these)
- **Auto-update:** Sparkle framework
- **Internal DB:** SQLite (data.db with WAL mode) for OAuth, settings, usage tracking
- **Bundle ID:** studio.techflow.macuse

### 3.2 MacUse Source Structure (from module paths in binary)
```
src/
  cli/mcp/
    server.rs, bridge.rs, oauth.rs
  services/
    analytics.rs
    api_key/service.rs
    event_bus.rs
    license.rs
    mcp_client/claude.rs
    mcp_server/
      config.rs, service.rs, permissions.rs, registry.rs
      compact_mode/types.rs
      http_service.rs
      legacy_sse/handlers.rs
      oauth/{registration, token, authorize, revoke}.rs
      servers/
        common/app_reference.rs
        calendar/{service, types}.rs
        contacts/
        location/
        map/{service, types}.rs
        mail/
        messages/{types}.rs
        notes/
        reminders/{types}.rs
        shortcuts/
        stickies/{error, responses, service, types}.rs
        ui_controller/{types, validation, diff}.rs
    oauth/{metadata_client, service, store}.rs
    onboarding.rs, panel.rs, updater.rs
  macos/
    accessibility/input/activation.rs
    contacts/store.rs
    eventkit/executor.rs
    messages/auth.rs
    stickies/{auth, jxa/executor, reader, rtf, types}.rs
  store/migrations/{v2..v9}.rs
  utils/icon.rs
  api/handlers/{mcp_client, oauth, api_key}.rs
  api/events.rs
  ability/{plan, rule, action, limits, ability, subject}.rs
  plugins/posthog.rs
  di/initialization.rs
```

### 3.3 MacUse Data Access Patterns

**Calendar & Reminders:** EventKit framework via EKEventStore with TTL caching. Deep links to open in native apps (ical://, x-apple-reminderkit://).

**Contacts:** CNContactStore with TTL caching. Listens for CNContactStoreDidChangeNotification to invalidate cache.

**Messages:** Direct SQLite access to ~/Library/Messages/chat.db. Queries join message, handle, and chat_message_join tables. Sends via AppleScript automation of Messages.app.

**Mail:** SQLite access to ~/Library/Mail/V10/MailData/Envelope Index for search/read. AppleScript for compose, reply, forward, move, delete. Requires Full Disk Access.

**Notes:** SQLite access to ~/Library/Group Containers/group.com.apple.notes/NoteStore.sqlite for reads (ZPLAINTEXT column for search). AppleScript for creates/updates (writing SQLite would corrupt sync).

**Location:** CoreLocation via CLLocationManager.

**Maps:** MapKit via MKLocalSearch and MKDirections.

**Stickies:** RTFD file reader for listing/reading. JXA scripts (create_sticky.js, delete_sticky.js, open_stickies.js) for automation.

**UI Automation:** macOS Accessibility API for tree inspection. XPath for element querying. CGEvent for mouse/keyboard input. Returns UI diffs after actions. Elements get short ref IDs (B1, T1, S1).

### 3.4 MacUse MCP Client Auto-Configuration

MacUse detects and auto-configures these MCP clients:
- Claude Desktop (com.anthropic.claudefordesktop) - Downloads and installs .mcpb bundle
- Cursor (com.todesktop.230313mzl4w4u92) - Opens via deeplink
- VS Code (com.microsoft.VSCode) - Configures user MCP settings
- Raycast (com.raycast.macos) - Opens via deeplink
- Goose - Configures ~/.config/goose/config.yaml
- ChatWise (app.chatwise) - Opens via deeplink
- LM Studio (ai.elementlabs.lmstudio) - Opens via deeplink
- Msty Studio - Opens setup guide
- Perplexity (ai.perplexity.mac) - Opens setup guide
- AnythingLLM (com.anythingllm) - Configures MCP servers

### 3.5 MacUse Permission Model

Permissions checked at tool-call time with human-readable error messages:
- Calendar: EKEventStore.requestFullAccessToEvents()
- Reminders: EKEventStore.requestFullAccessToReminders()
- Contacts: CNContactStore.requestAccess()
- Location: CLLocationManager.requestAlwaysAuthorization()
- Accessibility: AXIsProcessTrustedWithOptions (manual grant)
- Screen Recording: CGRequestScreenCaptureAccess() (manual grant)
- Full Disk Access: Detected via file access attempt (manual grant)
- Automation (per-app): NSAppleEventDescriptor (prompted per target app)

### 3.6 MacUse Licensing & Usage Tracking

Free tier: 100 tool calls/day, 1 connected client. Lifetime: $39, unlimited.

Internal SQLite tracks: daily tool calls, connected clients, OAuth tokens, API keys. Has migration system (v2 through v9).

We will NOT implement usage limits, licensing, or usage tracking.

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
- OAuth server (MacUse has this for remote MCP; we only need local)
- Analytics or crash reporting
- Auto-update (users update via Homebrew or DMG download)
