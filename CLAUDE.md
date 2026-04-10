# CLAUDE.md

## Project Overview

mac-app-oss is an open-source MCP server that gives AI assistants access to macOS native apps and universal UI control. It's a local, privacy-first replacement for MacUse (macuse.app).

**Current state:** Phase 1 complete. 18 tools working (Calendar 8, Reminders 7, Contacts 2, Permissions 1). Binary installed at `~/.local/bin/macapp-server`, configured for Claude Desktop and Claude Code.

## Target Audience

Non-technical people who use Claude Desktop/Claude Code. Installation must be simple (`bash scripts/setup-claude.sh`). System tray icon planned for Phase 4.

## Tech Stack

- **Language:** Rust (edition 2024, requires Rust 1.85+)
- **MCP:** rmcp 1.4 crate with stdio transport
- **macOS APIs:** AppleScript via osascript (Phase 1), objc2 crate family for later phases
- **Database:** rusqlite (bundled) for Messages/Notes/Mail SQLite reads in Phase 2
- **UI Automation:** Accessibility API + CGEvent (Phase 3)

## Development Rules

- **Tests are mandatory.** Every service ships with unit tests. Never skip tests.
- **Human-readable errors.** When a permission is missing, return a message explaining what to enable and where in System Settings.
- **No telemetry.** No analytics, no crash reporting, no phone-home. Everything stays local.
- **Phase-based development.** See README.md roadmap. Complete each phase with tests before moving on.
- **AppleScript first.** Use AppleScript for Phase 1-2 (avoids objc2 Send/Sync complexity). Migrate to direct framework calls for performance later.

## Build Commands

```bash
cargo build                    # Debug build
cargo build --release          # Release build
cargo test                     # 12 unit tests (no permissions needed)
cargo test -- --ignored        # Integration tests (needs macOS permissions)
bash scripts/setup-claude.sh   # Build + install + configure Claude Desktop/Code
```

## Key Directories

- `crates/macapp-core/src/services/` - One module per service (calendar, reminders, contacts, permissions_status)
- `crates/macapp-core/src/macos/` - macOS API wrappers (applescript.rs, eventkit.rs)
- `crates/macapp-core/src/registry.rs` - Service registry with dynamic tool routing
- `crates/macapp-core/src/permissions.rs` - Permission checking with human-readable errors
- `crates/macapp-server/src/main.rs` - MCP server entry point (ServerHandler impl)
- `scripts/setup-claude.sh` - Install and configure for Claude Desktop/Code

## Architecture Notes

- Server implements `rmcp::handler::server::ServerHandler` trait
- Tools registered dynamically via `ServiceRegistry` (not via rmcp macros)
- Each service has a `pub fn register(registry: &mut ServiceRegistry)` entry point
- Tool handlers are `Arc<dyn Fn(HashMap<String, Value>) -> Pin<Box<dyn Future<Output = Result<CallToolResult>>>>>`
- `schema_from_json(json!({...}))` converts serde_json::Value to `Arc<JsonObject>` for tool schemas
- `text_result()` / `error_result()` helpers for creating CallToolResult
- AppleScript uses `||` as field delimiter for multi-value outputs (pipes can appear in data)
- Server logs to stderr, MCP JSON-RPC goes to stdout

## What's Next (Phase 2)

Messages (4 tools), Mail (13 tools), Notes (8 tools), Location (1 tool), Maps (4 tools).
These need SQLite direct access (rusqlite) for reading chat.db, NoteStore.sqlite, and Mail Envelope Index.
Mail/Notes writes stay on AppleScript. Full Disk Access permission required.
