# mac-app-oss

Open-source MCP server for macOS native app integration and universal UI control.

A local, privacy-first replacement for [MacUse](https://macuse.app) that works with Claude Desktop, Cursor, Claude Code, and any MCP-compatible client. No cloud, no subscriptions, no telemetry.

## What It Does

mac-app-oss gives AI assistants direct access to your Mac's native apps and universal control of any application's UI:

- **Calendar** - Create events, search schedules, find available times
- **Reminders** - Create, complete, and manage reminders
- **Contacts** - Search and browse your contacts
- **Messages** - Search conversations, send iMessages *(Phase 2)*
- **Mail** - Search, read, compose, reply, forward emails *(Phase 2)*
- **Notes** - Search, read, create, and edit notes *(Phase 2)*
- **Maps** - Search places, get directions, find nearby POIs *(Phase 2)*
- **Location** - Get your current location *(Phase 2)*
- **UI Automation** - Click buttons, fill forms, navigate menus in any app *(Phase 3)*
- **Stickies** - List, read, create sticky notes *(Phase 4)*
- **Shortcuts** - List and run Siri Shortcuts *(Phase 4)*

Everything runs 100% locally on your Mac. No data leaves your machine.

## Quick Start

**Requirements:** macOS 14+, Rust toolchain (1.85+), `jq`

```bash
# 1. Clone the repo
git clone https://github.com/drbarq/mac-app-oss.git
cd mac-app-oss

# 2. Build, install, and configure Claude Desktop + Claude Code
bash scripts/setup-claude.sh

# 3. Restart Claude Desktop or Claude Code

# 4. Try it out:
#    "What's on my calendar this week?"
#    "Create a reminder to buy groceries"
#    "Find John's phone number"
```

The setup script:
1. Builds the release binary
2. Installs it to `~/.local/bin/macapp-server`
3. Auto-configures Claude Desktop (`claude_desktop_config.json`)
4. Auto-configures Claude Code (`~/.claude/mcp.json`)

## Current Status

**Phase 1 complete.** 18 tools working across Calendar, Reminders, Contacts, and Permissions.

| Service | Tools | Status |
|---|---|---|
| **Calendar** (8) | list_calendars, search_events, create_event, reschedule_event, cancel_event, update_event, open_event, find_available_times | Done |
| **Reminders** (7) | list_lists, search_reminders, create_reminder, update_reminder, delete_reminder, complete_reminder, open_reminder | Done |
| **Contacts** (2) | search, get_all | Done |
| **Permissions** (1) | permissions_status | Done |
| **Messages** (4) | search_chats, get_chat, search_messages, send_messages | Phase 2 |
| **Mail** (13) | list_accounts, list_mailboxes, search/get/compose/reply/forward/move/delete messages | Phase 2 |
| **Notes** (8) | list/search/read/write/delete/restore notes | Phase 2 |
| **Location** (1) | get_current | Phase 2 |
| **Maps** (4) | search_places, get_directions, explore_places, calculate_eta | Phase 2 |
| **UI Viewer** (6) | list_apps, get_frontmost, get_ui_tree, find_elements, capture_snapshot | Phase 3 |
| **UI Controller** (10) | click, type_text, press_key, scroll, drag, select_menu, manage_window/app | Phase 3 |
| **Stickies** (4) | list, read, create, open | Phase 4 |
| **Shortcuts** (3) | list, get, run | Phase 4 |

**Total: 18 / 71 tools implemented**

## Architecture

### How We Got Here

We dissected the installed MacUse binary (v1.7.3) to understand exactly how it works:

- **MacUse is Rust/Tauri** (not Swift) using the `rmcp` crate for MCP
- **Transports:** Streamable HTTP (background daemon) + stdio
- **macOS integration:** AppleScript/JXA for Calendar, Reminders, Mail, Notes, Stickies; SQLite for Messages/Notes/Mail reads; Accessibility API + CGEvent for UI automation

We're building the same thing, open-source, using the same proven approach.

### Tech Stack

| Component | Technology | Why |
|---|---|---|
| Language | Rust (edition 2024) | Best macOS FFI via objc2, high performance |
| MCP Server | rmcp 1.4 | Same library MacUse uses |
| macOS APIs | AppleScript (Phase 1), objc2 (later) | AppleScript for fast iteration, objc2 for performance |
| Database Access | rusqlite | Read Messages, Notes, Mail databases directly |
| UI Automation | Accessibility API + CGEvent | XPath queries on AX tree, input simulation |
| Scripting | AppleScript/JXA via osascript | Reliable cross-app automation |

### Project Structure

```
mac-app-oss/
  Cargo.toml                        # Workspace root
  crates/
    macapp-server/                  # MCP server binary
      src/main.rs                   # Entry point, stdio transport, tool routing
    macapp-core/                    # Core library (all services)
      src/
        registry.rs                 # Service registry, tool routing
        permissions.rs              # Permission checking with human-readable errors
        services/
          calendar/                 # 8 tools - AppleScript
          reminders/                # 7 tools - AppleScript
          contacts/                 # 2 tools - AppleScript
          permissions_status.rs     # 1 tool - permission checker
        macos/
          applescript.rs            # osascript runner
          eventkit.rs               # EventKit helpers
  scripts/
    setup-claude.sh                 # Build + install + configure Claude
```

## Permissions

mac-app-oss uses AppleScript to interact with native apps. macOS will prompt you to grant Automation permission for each app the first time it's accessed.

| Permission | Required For | How It's Granted |
|---|---|---|
| Automation (Calendar) | Calendar tools | Prompted automatically on first use |
| Automation (Reminders) | Reminder tools | Prompted automatically on first use |
| Automation (Contacts) | Contact tools | Prompted automatically on first use |
| Accessibility | UI automation (Phase 3) | System Settings > Privacy & Security > Accessibility |
| Full Disk Access | Messages, Notes, Mail DBs (Phase 2) | System Settings > Privacy & Security > Full Disk Access |

If a permission is missing, the tool returns a helpful error explaining exactly what to enable and where.

Use the `permissions_status` tool to check all permission states at once.

## Testing

```bash
cargo test              # 12 unit tests (no permissions needed)
cargo test -- --ignored # Integration tests (needs macOS permissions)
```

Every service ships with unit tests that validate tool schemas register correctly. 12 tests currently passing.

## Roadmap

### Phase 1: Calendar + Reminders + Contacts (Done)
- [x] Cargo workspace (macapp-core + macapp-server)
- [x] MCP server with rmcp 1.4 + stdio transport
- [x] Service registry with dynamic tool routing
- [x] Permission manager with human-readable errors
- [x] CalendarService (8 tools) + tests
- [x] RemindersService (7 tools) + tests
- [x] ContactsService (2 tools) + tests
- [x] permissions_status tool
- [x] Install script (build + configure Claude Desktop/Code)

### Phase 2: Messages + Mail + Notes + Location + Maps
- [ ] SQLite helper (Messages chat.db, Notes NoteStore.sqlite, Mail Envelope Index)
- [ ] MessagesService (4 tools) + tests
- [ ] MailService (13 tools) + tests
- [ ] NotesService (8 tools) + tests
- [ ] LocationService (1 tool) + tests
- [ ] MapsService (4 tools) + tests
- [ ] E2E test harness

### Phase 3: UI Automation
- [ ] Accessibility API wrapper + XPath query engine
- [ ] Ref ID system (B1=button, T1=textfield)
- [ ] UI Viewer (6 tools): list_apps, get_frontmost, get_ui_tree, get_visible_text, find_elements, capture_snapshot
- [ ] UI Controller (10 tools): click, type_text, press_key, scroll, drag, select_menu, manage_window/app, file_dialog, dock
- [ ] UI diff engine (show what changed after actions)

### Phase 4: Stickies + Shortcuts + Distribution
- [ ] StickiesService (4 tools) + tests
- [ ] ShortcutsService (3 tools) + tests
- [ ] System tray app (Tauri) with status indicator
- [ ] HTTP+SSE transport
- [ ] Homebrew formula
- [ ] GitHub Actions CI
- [ ] DMG installer + code signing

## Contributing

The project is structured so adding a new service is self-contained:
1. Create a module in `crates/macapp-core/src/services/`
2. Implement tools following the pattern in `calendar/mod.rs`
3. Register in `services/mod.rs` and `macapp-server/src/main.rs`
4. Add tests
5. No changes needed to server, transport, or protocol code

## License

MIT
