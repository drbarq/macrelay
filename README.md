# MacRelay

Open-source MCP server that relays your AI's commands to native macOS apps.

A local, privacy-first replacement for [MacUse](https://macuse.app) ($39) that works with Claude Desktop, Cursor, Claude Code, and any MCP-compatible client. No cloud, no subscriptions, no telemetry.

## What It Does

MacRelay gives AI assistants direct access to your Mac's native apps:

- **Calendar** - Create events, search schedules, find available times
- **Reminders** - Create, complete, and manage reminders
- **Contacts** - Search and browse your contacts
- **Messages** - Search conversations, send iMessages
- **Mail** - Search, read, compose, reply, forward emails
- **Notes** - Search, read, create, and edit notes
- **Maps** - Search places, get directions, find nearby POIs
- **Location** - Get your current location
- **UI Automation** - Click buttons, fill forms, navigate menus in any app
- **Stickies** - List, read, create sticky notes
- **Shortcuts** - List and run Siri Shortcuts

Everything runs 100% locally on your Mac. No data leaves your machine.

## Quick Start

**Requirements:** macOS 14+, Rust toolchain (1.85+), `jq`

```bash
# 1. Clone the repo
git clone https://github.com/drbarq/macrelay.git
cd macrelay

# 2. Build, install, and configure Claude Desktop + Claude Code
bash scripts/setup-claude.sh

# 3. Restart Claude Desktop or Claude Code

# 4. Try it out:
#    "What's on my calendar this week?"
#    "Create a reminder to buy groceries"
#    "Search my emails from Amazon"
#    "What apps are running?"
#    "List my shortcuts"
```

The setup script:
1. Builds the release binary (~4MB)
2. Installs it to `~/.local/bin/macrelay`
3. Auto-configures Claude Desktop (`claude_desktop_config.json`)
4. Auto-configures Claude Code (`~/.claude/mcp.json`)

## Current Status

**Feature complete.** 71 tools across 13 services. Full parity with MacUse.

| Service | # | Tools | Status |
|---|---|---|---|
| **Calendar** | 8 | list_calendars, search_events, create_event, reschedule_event, cancel_event, update_event, open_event, find_available_times | Done |
| **Reminders** | 7 | list_lists, search_reminders, create_reminder, update_reminder, delete_reminder, complete_reminder, open_reminder | Done |
| **Contacts** | 2 | search, get_all | Done |
| **Permissions** | 1 | permissions_status | Done |
| **Notes** | 8 | list_accounts, list_folders, search_notes, read_note, write_note, delete_note, restore_note, open_note | Done |
| **Mail** | 13 | list_accounts, list_mailboxes, search_messages, get_messages, get_thread, compose_message, reply_message, forward_message, update_read_state, move_message, delete_message, open_message, get_attachment | Done |
| **Messages** | 4 | search_chats, get_chat, search_messages, send_message | Done |
| **Location** | 1 | get_current | Done |
| **Maps** | 4 | search_places, get_directions, explore_places, calculate_eta | Done |
| **UI Viewer** | 6 | list_apps, get_frontmost, get_ui_tree, get_visible_text, find_elements, capture_snapshot | Done |
| **UI Controller** | 10 | click, type_text, press_key, scroll, drag, select_menu, manage_window, manage_app, file_dialog, dock | Done |
| **Stickies** | 4 | list, read, create, open | Done |
| **Shortcuts** | 3 | list, get, run | Done |

### MacRelay vs MacUse

| | MacUse | MacRelay |
|---|---|---|
| Tools | 55 | **71** |
| Price | $39 | Free |
| Source | Closed | [MIT](https://github.com/drbarq/macrelay) |
| Telemetry | PostHog + Sentry | None |
| Cloud dependency | License server | None |
| MCP clients | Claude Desktop, Cursor, etc. | Same |

## How It Was Built

This entire project was built in a single Claude Code session:

| Metric | Value |
|---|---|
| Model | Claude Opus 4.6 (1M context) |
| Context used | 318k tokens |
| Total tokens | 113M (845k in, 283k out, 112.7M cached) |
| Throughput | 103 tokens/sec (4.7 in, 98.6 out) |
| Tools implemented | 71 |
| Tests written | 27 |
| Lines of Rust | ~8,000 |

The process:
1. Dissected the installed MacUse binary to extract its complete architecture
2. Identified all 55+ tools, source module structure, and technical approach
3. Built the MCP server from scratch using the same `rmcp` crate
4. Implemented all services in 4 phases using parallel agent workflows

## Architecture

### How We Got Here

We dissected the installed MacUse binary (v1.7.3) to understand exactly how it works:

- **MacUse is Rust/Tauri** (not Swift) using the `rmcp` crate for MCP
- **Transports:** Streamable HTTP (background daemon) + stdio
- **macOS integration:** AppleScript/JXA for Calendar, Reminders, Mail, Notes, Stickies; SQLite for Messages reads; Accessibility API + CGEvent for UI automation

We built the same thing, open-source, using the same proven approach.

### Tech Stack

| Component | Technology | Why |
|---|---|---|
| Language | Rust (edition 2024) | Best macOS FFI, high performance, same as MacUse |
| MCP Server | rmcp 1.4 | Same library MacUse uses |
| macOS APIs | AppleScript/JXA + SQLite | Reliable cross-app automation |
| Database | rusqlite (bundled) | Read Messages chat.db directly |
| UI Automation | System Events + JXA | Accessibility tree inspection + input simulation |

### Project Structure

```
macrelay/
  Cargo.toml                        # Workspace root
  crates/
    macrelay-server/                # MCP server binary (macrelay)
      src/main.rs                   # Entry point, ServerHandler impl
    macrelay-core/                  # Core library
      src/
        registry.rs                 # Service registry, tool routing
        permissions.rs              # Permission checking
        services/
          calendar/                 # 8 tools - AppleScript
          reminders/                # 7 tools - AppleScript
          contacts/                 # 2 tools - AppleScript
          notes/                    # 8 tools - AppleScript
          mail/                     # 13 tools - AppleScript
          messages/                 # 4 tools - SQLite + AppleScript
          location/                 # 1 tool - CoreLocation via Swift
          maps/                     # 4 tools - Maps URL scheme
          ui_viewer/                # 6 tools - System Events + JXA
          ui_controller/            # 10 tools - System Events
          stickies/                 # 4 tools - RTFD files + JXA
          shortcuts/                # 3 tools - /usr/bin/shortcuts
          permissions_status.rs     # 1 tool
        macos/
          applescript.rs            # osascript/JXA runner
          eventkit.rs               # EventKit helpers
  scripts/
    setup-claude.sh                 # Build + install + configure
  docs/
    PRD.md                          # Full product requirements
```

## Permissions

MacRelay uses AppleScript to interact with native apps. macOS will prompt for Automation permission per-app on first use.

| Permission | Required For | How It's Granted |
|---|---|---|
| Automation (per app) | Calendar, Reminders, Contacts, Mail, Notes, Messages, Stickies | Prompted automatically |
| Accessibility | UI Viewer + UI Controller tools | System Settings > Privacy & Security > Accessibility |
| Full Disk Access | Messages search (SQLite) | System Settings > Privacy & Security > Full Disk Access |
| Location Services | Location tool | Prompted automatically |

Use the `permissions_status` tool to check all states at once.

## Testing

```bash
cargo test              # 27 unit tests (no permissions needed)
cargo test -- --ignored # Integration tests (needs macOS permissions)
```

Every service includes schema validation tests. 27 tests currently passing.

## Roadmap

### Completed
- [x] Phase 1: Calendar + Reminders + Contacts (18 tools)
- [x] Phase 2: Notes + Mail + Messages + Location + Maps (30 tools)
- [x] Phase 3: UI Viewer + UI Controller (16 tools)
- [x] Phase 4: Stickies + Shortcuts (7 tools)

### Future: Beyond MacUse
Potential additions that go beyond what MacUse offers:

- [ ] **Safari/Browser** - Bookmarks, history, reading list, open tabs
- [ ] **Music** - Playback control, search library, queue management
- [ ] **Photos** - Search photos, browse albums, get metadata
- [ ] **System** - Volume, brightness, Wi-Fi, Bluetooth, Do Not Disturb
- [ ] **Clipboard** - Read/write clipboard contents
- [ ] **Notifications** - Send macOS notifications
- [ ] **Finder** - Advanced file operations, tags, Spotlight search
- [ ] **Terminal** - Execute shell commands (sandboxed)

### Future: Distribution
- [ ] Homebrew formula (`brew install macrelay`)
- [ ] System tray app (Tauri) with status indicator
- [ ] GitHub Actions CI + universal binary releases
- [ ] DMG installer + code signing for non-technical users

## Contributing

Adding a new service is self-contained:
1. Create a module in `crates/macrelay-core/src/services/`
2. Implement tools following the pattern in `calendar/mod.rs`
3. Register in `services/mod.rs` and `macrelay-server/src/main.rs`
4. Add tests

See [docs/PRD.md](docs/PRD.md) for the full product requirements and technical details.

## License

MIT
