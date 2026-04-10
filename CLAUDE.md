# CLAUDE.md

## Project Overview

MacRelay is an open-source MCP server that relays AI commands to native macOS apps. It's a local, privacy-first replacement for MacUse (macuse.app, $39).

**Current state:** Feature complete. 71 tools across 13 services. Binary at `~/.local/bin/macrelay`, configured for Claude Desktop and Claude Code.

## Tech Stack

- **Language:** Rust (edition 2024, requires Rust 1.85+)
- **MCP:** rmcp 1.4 crate with stdio transport
- **macOS APIs:** AppleScript/JXA via osascript for most services
- **Database:** rusqlite (bundled) for Messages SQLite reads
- **UI Automation:** System Events + JXA for accessibility tree + input simulation
- **Location:** CoreLocation via Swift subprocess
- **Maps:** Apple Maps URL scheme

## Build Commands

```bash
cargo build                    # Debug build
cargo build --release          # Release build (~4MB binary)
cargo test                     # 27 unit tests (no permissions needed)
cargo test -- --ignored        # Integration tests (needs macOS permissions)
bash scripts/setup-claude.sh   # Build + install + configure Claude Desktop/Code
```

## Services (13 services, 71 tools)

| Service | Tools | Implementation |
|---|---|---|
| calendar (8) | list/search/create/reschedule/cancel/update/open events, find_available_times | AppleScript |
| reminders (7) | list/search/create/update/delete/complete/open reminders | AppleScript |
| contacts (2) | search, get_all | AppleScript |
| notes (8) | list_accounts/folders, search/read/write/delete/restore/open notes | AppleScript |
| mail (13) | list_accounts/mailboxes, search/get/thread/compose/reply/forward/read_state/move/delete/open messages, get_attachment | AppleScript |
| messages (4) | search_chats/messages, get_chat, send_message | SQLite + AppleScript |
| location (1) | get_current | CoreLocation via Swift |
| maps (4) | search_places, get_directions, explore_places, calculate_eta | Maps URL scheme |
| ui_viewer (6) | list_apps, get_frontmost, get_ui_tree, get_visible_text, find_elements, capture_snapshot | System Events + JXA |
| ui_controller (10) | click, type_text, press_key, scroll, drag, select_menu, manage_window/app, file_dialog, dock | System Events |
| stickies (4) | list, read, create, open | RTFD files + JXA |
| shortcuts (3) | list, get, run | /usr/bin/shortcuts CLI |
| permissions (1) | permissions_status | Native checks |

## Key Directories

- `crates/macrelay-core/src/services/` - 13 service modules
- `crates/macrelay-core/src/macos/` - applescript.rs, eventkit.rs
- `crates/macrelay-core/src/registry.rs` - ServiceRegistry with dynamic tool routing
- `crates/macrelay-core/src/permissions.rs` - Permission checking
- `crates/macrelay-server/src/main.rs` - MCP ServerHandler impl
- `scripts/setup-claude.sh` - Install + configure

## Architecture Notes

- Server implements `rmcp::handler::server::ServerHandler` trait
- Tools registered dynamically via `ServiceRegistry` (not rmcp macros)
- Each service: `pub fn register(registry: &mut ServiceRegistry)`
- Schemas: `schema_from_json(json!({...}))` -> `Arc<JsonObject>`
- Results: `text_result()` / `error_result()` helpers
- AppleScript field delimiter: `||`
- Server logs to stderr, MCP JSON-RPC to stdout
- Binary name: `macrelay`

## Adding New Services

1. Create `crates/macrelay-core/src/services/myservice/mod.rs`
2. Implement `pub fn register(registry: &mut ServiceRegistry)` with tools
3. Add `pub mod myservice;` to `services/mod.rs`
4. Add `macrelay_core::services::myservice::register(&mut registry);` to `main.rs`
5. Add tests in `#[cfg(test)] mod tests`
