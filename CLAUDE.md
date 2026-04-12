# CLAUDE.md

## Project Overview

MacRelay is an open-source MCP server that gives AI assistants access to native macOS apps. Local, privacy-first, no cloud.

**Current state:** Feature complete. 71 tools across 13 services. All tools use full category prefixes (e.g., `communication_mail_...`) for perfect alphabetical grouping in Claude Desktop.

## Tech Stack

- **Language:** Rust (edition 2024, requires Rust 1.85+)
- **MCP:** rmcp 1.4 crate with stdio transport
- **macOS APIs:** AppleScript/JXA via osascript for most services
- **Database:** rusqlite (bundled) for Messages SQLite reads
- **UI Automation:** System Events + JXA for accessibility tree + input simulation
- **Location:** CoreLocation via Swift subprocess
- **Maps:** Apple Maps URL scheme

## Build & Test Commands

```bash
cargo build                    # Debug build
cargo build --release          # Release build (~4MB binary)
cargo test -p macrelay-core --lib                                  # 137 CI-safe tests
cargo test -p macrelay-core --all-targets -- --include-ignored     # All 166 (includes Tier 3)
cargo fmt -- --check && cargo clippy --all-targets -- -D warnings  # CI gates
bash scripts/setup-claude.sh   # Rebuild and refresh monolithic config
```

## Tool Naming (Alphabetical Grouping)

Tools are prefixed by category to ensure they appear grouped in the UI:
- `communication_` (Mail, Messages)
- `navigation_` (Maps, Location)
- `pim_` (Calendar, Reminders, Contacts)
- `productivity_` (Notes, Stickies, Shortcuts)
- `system_` (Permissions Status)
- `ui_` (Viewer, Controller)

## Testing Strategy

- **Tier 1 (Pure Unit):** Helper functions, schema validation.
- **Tier 2 (Mock Runner):** Intercepts AppleScript/JXA. **Must** assert script content.
- **Tier 3 (Integration):** Local-only, hits real apps. **Must** clean up all created data.
- **Stability:** Tools return `Ok(error_result)` for missing args, not protocol `Err`.

## Key Directories

- `crates/macrelay-core/src/services/` - 13 service modules
- `crates/macrelay-core/src/macos/` - applescript.rs, escape.rs, eventkit.rs
- `crates/macrelay-core/src/registry.rs` - ServiceRegistry with alphabetical sorting
- `crates/macrelay-core/src/permissions.rs` - Permission checking
- `crates/macrelay-server/src/main.rs` - MCP ServerHandler impl
- `scripts/setup-claude.sh` - Install + configure

## Architecture Notes

- Server implements `rmcp::handler::server::ServerHandler` trait
- Tools registered dynamically via `ServiceRegistry`
- `ServiceRegistry::list_tools()` sorts alphabetically by name
- Schemas: `schema_from_json(json!({...}))` -> `Arc<JsonObject>`
- Results: `text_result()` / `error_result()` helpers
- AppleScript field delimiter: `||`
- Server logs to stderr, MCP JSON-RPC to stdout
- Binary name: `macrelay`
