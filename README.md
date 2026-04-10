# mac-app-oss

Open-source MCP server for macOS native app integration and universal UI control.

A local, privacy-first replacement for [MacUse](https://macuse.app) that works with Claude Desktop, Cursor, Claude Code, and any MCP-compatible client. No cloud, no subscriptions, no telemetry.

## What It Does

mac-app-oss gives AI assistants direct access to your Mac's native apps and universal control of any application's UI:

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

## Status

**In development.** See [Roadmap](#roadmap) for current progress.

## Quick Start

> Coming soon. Phase 1 is under development.

The goal is:
1. Download the `.dmg`
2. Drag to Applications
3. Launch - system tray icon appears
4. Click "Configure Claude Desktop" in the tray menu
5. Start using your Mac's apps through Claude

## Architecture

### How We Got Here

We dissected the installed MacUse binary (v1.7.3) to understand exactly how it works:

- **Tech stack:** Rust + Tauri v2, `rmcp` crate for MCP, SQLite for internal state
- **Transports:** Streamable HTTP (background daemon) + stdio
- **macOS integration:** EventKit, Contacts framework, CoreLocation, MapKit, Accessibility API, CGEvent, SQLite direct access to Messages/Notes/Mail databases, AppleScript/JXA for apps without framework APIs

We're building the same thing, open-source, using the same proven technical approach.

### Tech Stack

| Component | Technology | Why |
|---|---|---|
| Language | Rust | Best macOS FFI via objc2, high performance, matches MacUse |
| MCP Server | rmcp crate | Same library MacUse uses, mature Rust MCP implementation |
| macOS APIs | objc2 crate family | Type-safe Objective-C bindings for EventKit, Contacts, CoreLocation, MapKit |
| Database Access | rusqlite | Read Messages (chat.db), Notes (NoteStore.sqlite), Mail (Envelope Index) |
| UI Automation | Accessibility API + CGEvent | XPath queries on AX tree, mouse/keyboard simulation |
| App GUI | Tauri v2 | System tray, settings UI, permission wizard |
| Scripting | AppleScript/JXA | Fallback for apps without framework APIs (Mail, Notes writes, Stickies) |

### Complete Tool Surface (55 tools across 12 services)

| Service | # | Tools | Implementation |
|---|---|---|---|
| **Calendar** | 8 | list_calendars, search_events, create_event, reschedule_event, cancel_event, update_event, open_event, find_available_times | EventKit (EKEventStore with TTL cache), deep links |
| **Reminders** | 7 | list_lists, search_reminders, create_reminder, update_reminder, delete_reminder, complete_reminder, open_reminder | EventKit, deep links |
| **Notes** | 8 | list_accounts, list_folders, search_notes, read_note, write_note, delete_note, restore_note, open_note | SQLite for reads + AppleScript for writes |
| **Mail** | 13 | list_accounts, list_mailboxes, search_messages, get_messages, get_thread, compose_message, reply_message, forward_message, update_read_state, move_message, delete_message, open_message, get_attachment | SQLite for search + AppleScript for actions |
| **Messages** | 4 | search_chats, get_chat, search_messages, send_messages | SQLite (chat.db) for reads + Automation for sends |
| **Contacts** | 2 | search, get_all | Contacts framework (CNContactStore with cache) |
| **Maps** | 4 | search_places, get_directions, explore_places, calculate_eta | MapKit |
| **Location** | 1 | get_current | CoreLocation |
| **UI Viewer** | 6 | list_apps, get_frontmost, get_ui_tree, get_visible_text, find_elements, capture_snapshot | Accessibility API, XPath queries, ref IDs (B1, T1) |
| **UI Controller** | 10 | click, type_text, press_key, scroll, drag, select_menu, manage_window, manage_app, file_dialog, dock | Accessibility actions + CGEvent, returns UI diffs |
| **Stickies** | 4 | list, read, create, open | RTFD file reader + JXA scripts |
| **Shortcuts** | 3 | list, get, run | /usr/bin/shortcuts CLI wrapper |

### Key Design Patterns

- **UI actions return diffs** - After clicking/typing, the response shows what changed in the UI
- **Short ref IDs** - UI elements get IDs like B1 (button 1), T1 (text field 1) for easy AI referencing
- **XPath queries** - Find UI elements with familiar syntax: `//AXButton[@AXTitle='Save']`
- **Compact mode** - Reduce token usage for large UI trees
- **Permission-aware** - Graceful errors with human-readable instructions when permissions are missing
- **System tray** - Always visible status indicator so users know it's running

## Project Structure

```
mac-app-oss/
  Cargo.toml                        # Workspace root
  crates/
    macuse-app/                     # Tauri app (system tray + settings UI)
      src/
        main.rs                     # Tauri entry, system tray, LaunchAgent
        tray.rs                     # System tray icon + menu
        setup.rs                    # First-run permission wizard
        commands.rs                 # Tauri IPC commands
      ui/                           # Frontend (HTML/JS)
        index.html                  # Settings/status dashboard
        setup.html                  # Permission wizard
    macuse-server/                  # MCP server binary (standalone)
      src/
        main.rs                     # Entry point, stdio + HTTP transport
        config.rs                   # CLI args, env vars
    macuse-core/                    # Core library (all services)
      src/
        lib.rs
        registry.rs                 # Service registry, tool routing
        permissions.rs              # Permission checking/requesting
        services/                   # One module per service, each with tests
          calendar/
          reminders/
          notes/
          mail/
          messages/
          contacts/
          maps/
          location/
          ui_viewer/
          ui_controller/
          stickies/
          shortcuts/
        macos/                      # macOS API wrappers
          eventkit.rs
          contacts.rs
          location.rs
          mapkit.rs
          accessibility.rs
          sqlite.rs
          applescript.rs
          jxa.rs
    macuse-test-harness/            # E2E MCP test client
      src/lib.rs                    # Spawn server, send tool calls, validate
      tests/                        # Per-service E2E tests
  scripts/
    install.sh
    build-dmg.sh
```

## Permissions

mac-app-oss needs macOS permissions to access native apps. The first-run wizard walks you through each one:

| Permission | Required For | How to Grant |
|---|---|---|
| Accessibility | UI automation (click, type, inspect) | System Settings > Privacy & Security > Accessibility |
| Screen Recording | Screenshots | System Settings > Privacy & Security > Screen Recording |
| Full Disk Access | Messages, Notes, Mail databases | System Settings > Privacy & Security > Full Disk Access |
| Calendar | Calendar events | Prompted automatically on first use |
| Reminders | Reminders | Prompted automatically on first use |
| Contacts | Contacts | Prompted automatically on first use |
| Location | Current location, Maps | Prompted automatically on first use |

If a permission is missing, the tool returns a helpful error message explaining exactly what to enable and where.

## Testing

Three tiers of tests ensure reliability:

| Tier | Command | What It Tests | Needs Permissions? |
|---|---|---|---|
| 1. Unit | `cargo test` | Parsing, serialization, query building | No |
| 2. Integration | `cargo test -- --ignored` | Real macOS API calls, CRUD round-trips | Yes |
| 3. E2E | `cargo test -p macuse-test-harness` | Full MCP JSON-RPC round-trips | Yes |

Every service ships with tests. CI runs Tier 1; release gates on all three tiers.

## Roadmap

### Phase 1: MVP (In Progress)
- [ ] Project scaffolding (Cargo workspace, Tauri app, rmcp server)
- [ ] System tray with status indicator
- [ ] First-run permission wizard
- [ ] Permission manager
- [ ] CalendarService (8 tools) + tests
- [ ] RemindersService (7 tools) + tests
- [ ] ContactsService (2 tools) + tests
- [ ] MCP protocol tests
- [ ] DMG installer + Claude Desktop auto-configuration

**Exit criteria:** Drag .app to Applications, launch, see tray icon, use Calendar/Reminders/Contacts through Claude Desktop.

### Phase 2: All Native Apps
- [ ] SQLite helper (Messages, Notes, Mail DB access)
- [ ] AppleScript runner
- [ ] MessagesService (4 tools) + tests
- [ ] MailService (13 tools) + tests
- [ ] NotesService (8 tools) + tests
- [ ] LocationService (1 tool) + tests
- [ ] MapsService (4 tools) + tests
- [ ] E2E test suite

**Exit criteria:** All native app tools working and tested.

### Phase 3: UI Automation
- [ ] Accessibility API wrapper
- [ ] XPath query engine for AX tree
- [ ] Ref ID system (B1, T1, etc.)
- [ ] UI Viewer (6 tools) + tests
- [ ] CGEvent input simulation
- [ ] UI diff engine
- [ ] UI Controller (10 tools) + tests

**Exit criteria:** Ask Claude to open an app, read its UI, and interact with it.

### Phase 4: Polish + Distribution
- [ ] StickiesService (4 tools) + tests
- [ ] ShortcutsService (3 tools) + tests
- [ ] HTTP+SSE transport
- [ ] Homebrew formula
- [ ] GitHub Actions CI
- [ ] Code signing + notarization

**Exit criteria:** Feature parity with MacUse. `brew install mac-app-oss` works. DMG installs without Gatekeeper warnings.

## Contributing

> Contributing guidelines coming soon.

The project is structured so adding a new service is self-contained:
1. Create a module in `crates/macuse-core/src/services/`
2. Implement the service trait with tools
3. Register in the service registry
4. Add tests
5. No changes needed to server, transport, or protocol code

## License

MIT
