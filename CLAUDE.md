# CLAUDE.md

## Project Overview

mac-app-oss is an open-source MCP server that gives AI assistants access to macOS native apps and universal UI control. It's a local, privacy-first replacement for MacUse (macuse.app).

## Target Audience

Non-technical people who use Claude Desktop/Claude Code. Installation must be drag-and-drop simple. System tray icon is essential so users know it's running.

## Tech Stack

- **Language:** Rust
- **MCP:** rmcp crate (same as MacUse v1.7.3)
- **macOS APIs:** objc2 crate family (EventKit, Contacts, CoreLocation, MapKit, AppKit)
- **Database:** rusqlite for reading Messages/Notes/Mail SQLite databases
- **UI Automation:** macOS Accessibility API + CGEvent
- **App GUI:** Tauri v2 (system tray, settings UI, permission wizard)
- **Scripting:** AppleScript/JXA via osascript for Mail, Notes writes, Stickies

## Development Rules

- **Tests are mandatory.** Every service ships with unit tests + integration tests. Never skip tests.
- **Human-readable errors.** When a permission is missing, return a message explaining what to enable and where in System Settings.
- **No telemetry.** No analytics, no crash reporting, no phone-home. Everything stays local.
- **Phase-based development.** See README.md roadmap. Complete each phase with tests before moving on.

## Build Commands

```bash
cargo build                    # Debug build
cargo build --release          # Release build
cargo test                     # Unit tests (no permissions needed)
cargo test -- --ignored        # Integration tests (needs macOS permissions)
cargo test -p macapp-test-harness  # E2E MCP round-trip tests
```

## Key Directories

- `crates/macapp-core/src/services/` - One module per service (calendar, reminders, etc.)
- `crates/macapp-core/src/macos/` - macOS API wrappers (eventkit, contacts, accessibility, etc.)
- `crates/macapp-server/src/` - MCP server binary (stdio + HTTP transport)
- `crates/macapp-app/src/` - Tauri app (system tray, settings UI)
- `crates/macapp-test-harness/` - E2E test client

## Architecture Notes

- MacUse uses `rmcp` v0.17 with MCP protocol versions 2025-06-18 and 2025-11-25
- UI elements use XPath queries (e.g., `//AXButton[@AXTitle='Save']`) and short ref IDs (B1, T1)
- UI actions return diffs showing what changed
- Calendar/Reminders use EventKit with TTL-cached EKEventStore
- Messages/Notes/Mail read SQLite databases directly (requires Full Disk Access)
- Mail/Notes writes use AppleScript (writing directly to SQLite would corrupt sync)
- Stickies use RTFD file reading + JXA scripts
- Background daemon runs via LaunchAgent, Tauri app manages tray + settings
