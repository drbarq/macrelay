# MacRelay: Future Roadmap

Ideas for v2 and beyond — services that no Mac MCP server offers today.

## v2: New Services

### Safari / Browser
Full browser integration via AppleScript + SQLite (History.db, Bookmarks.plist).

| Tool | Description |
|---|---|
| safari_list_tabs | List all open tabs across windows |
| safari_get_tab | Get URL and page title of a tab |
| safari_open_url | Open a URL in a new tab |
| safari_close_tab | Close a specific tab |
| safari_get_history | Search browsing history by date/query |
| safari_get_bookmarks | List/search bookmarks |
| safari_get_reading_list | List reading list items |
| safari_get_page_content | Get the text content of the current page |

**Why:** "What tabs do I have open?" / "Find that article I read yesterday" / "Save this page to my reading list"

### Music
Control Apple Music and Spotify via AppleScript.

| Tool | Description |
|---|---|
| music_get_now_playing | What's currently playing (title, artist, album, position) |
| music_play | Play/resume |
| music_pause | Pause |
| music_next | Skip to next track |
| music_previous | Go to previous track |
| music_search_library | Search your music library |
| music_add_to_queue | Add a song to Up Next |
| music_set_volume | Set music volume (0-100) |
| music_get_playlists | List all playlists |
| music_play_playlist | Play a specific playlist |

**Why:** Voice-control your music through Claude. Works with both Apple Music and Spotify.

### Photos
Search and browse your photo library via SQLite (Photos.sqlite) + AppleScript.

| Tool | Description |
|---|---|
| photos_search | Search photos by date, location, people, keywords |
| photos_list_albums | List all albums |
| photos_get_album | Get photos from a specific album |
| photos_get_recent | Get most recent N photos |
| photos_get_metadata | Get EXIF data, location, date for a photo |
| photos_get_memories | List photo memories |
| photos_open | Open a photo in Photos.app |

**Why:** "Find photos from my trip to Japan" / "Show me photos of Sarah from last month"

### System Controls
Control macOS system settings via AppleScript and shell commands.

| Tool | Description |
|---|---|
| system_get_volume | Get current volume level |
| system_set_volume | Set volume (0-100) or mute/unmute |
| system_get_brightness | Get screen brightness |
| system_set_brightness | Set screen brightness (0-100) |
| system_toggle_wifi | Turn Wi-Fi on/off |
| system_get_wifi_status | Get Wi-Fi network name and status |
| system_toggle_bluetooth | Turn Bluetooth on/off |
| system_list_bluetooth_devices | List paired Bluetooth devices |
| system_toggle_dnd | Toggle Do Not Disturb / Focus mode |
| system_get_focus_status | Get current Focus mode |
| system_get_battery | Battery level, charging status, time remaining |
| system_get_storage | Disk usage and available space |
| system_get_uptime | How long since last restart |
| system_sleep | Put display to sleep |
| system_lock_screen | Lock the screen |

**Why:** "Turn on Do Not Disturb" / "What's my battery at?" / "Mute my computer"

### Notifications
Send native macOS notifications from Claude.

| Tool | Description |
|---|---|
| notification_send | Send a macOS notification with title, body, sound |
| notification_send_with_action | Send notification with clickable action button |
| notification_schedule | Schedule a notification for a future time |
| notification_list_settings | List notification settings per app |

**Why:** Claude can proactively notify you - "I'll send you a notification when the build finishes" / Timer and alarm functionality without needing Reminders.

### Clipboard
Read and manage clipboard contents.

| Tool | Description |
|---|---|
| clipboard_get | Get current clipboard contents (text, rich text, or image info) |
| clipboard_set | Set clipboard contents |
| clipboard_get_history | Get clipboard history (if a clipboard manager is running) |
| clipboard_clear | Clear the clipboard |

**Why:** "What did I just copy?" / "Copy this to my clipboard" - natural bridge between AI and manual workflows.

### Finder / Files
Advanced file operations beyond basic shell commands.

| Tool | Description |
|---|---|
| finder_spotlight_search | Search files via Spotlight (mdfind) |
| finder_get_tags | Get Finder tags on a file |
| finder_set_tags | Set Finder tags on a file |
| finder_get_recent | Get recently opened files |
| finder_get_downloads | List recent downloads |
| finder_reveal | Reveal a file in Finder |
| finder_quick_look | Quick Look preview a file |
| finder_trash | Move a file to Trash |
| finder_empty_trash | Empty the Trash |
| finder_get_info | Get file metadata (size, dates, kind) |

**Why:** "Find all PDFs tagged 'work'" / "What did I download today?" / Organize files by tags

### Terminal (Sandboxed)
Controlled shell command execution with safety guardrails.

| Tool | Description |
|---|---|
| terminal_run | Execute a shell command with timeout and output capture |
| terminal_run_background | Run a command in the background |
| terminal_get_output | Get output from a background command |
| terminal_kill | Kill a background process |

**Why:** Power users who want Claude to run build commands, check processes, etc. Must be sandboxed with allowlists.

## v2: Distribution

| Feature | Status |
|---|---|
| **Homebrew Cask** | Done — `brew install --cask drbarq/tap/macrelay` |
| **Menu bar app** | Done — pure Rust (tray-icon + muda + tao), service toggles, permissions view |
| **GitHub Actions** | Done — CI + release workflow builds universal .app bundle |
| **LaunchAgent** | Done — "Launch at Login" toggle in menu bar |
| **Claude Desktop extension** | Done — installs with icon + manifest, no LOCAL DEV badge |
| **DMG installer** | Future — drag-to-Applications for non-technical users |
| **Code signing** | Future — Apple Developer certificate + notarization |
| **Auto-update** | Future — Sparkle framework or custom updater |

## v2: Performance

| Improvement | Description |
|---|---|
| **Direct EventKit** | Replace Calendar/Reminders AppleScript with objc2 EventKit for 10x speed |
| **Direct Contacts** | Replace Contacts AppleScript with objc2 CNContactStore |
| **SQLite for Notes** | Read NoteStore.sqlite directly instead of AppleScript |
| **SQLite for Mail** | Read Envelope Index directly for faster mail search |
| **Async AppleScript** | Run AppleScript in background threads to avoid blocking |
| **Tool caching** | Cache results for tools like list_calendars that change infrequently |

## v2: Advanced Features

| Feature | Description |
|---|---|
| **Compound actions** | Chain multiple tools in a single call ("create event AND set reminder") |
| **Webhooks** | HTTP callbacks when events happen (new email, calendar change) |
| **HTTP+SSE transport** | Streamable HTTP for web-based MCP clients |
| **Tool permissions** | Per-tool access control (allow calendar but deny messages) |
| **Audit log** | Log all tool calls for security review |
| **Rate limiting** | Configurable limits per service |

## Tool Count Projection

| Version | Tools | New |
|---|---|---|
| v1.0 (current) | 71 | - |
| v2.0 (new services) | ~140 | ~70 |
| v2.0 (distribution) | ~140 | +Homebrew, tray, DMG |

## Priority Order for v2 Services

Based on user value and implementation difficulty:

1. **System Controls** (15 tools) - High value, easy to implement with AppleScript
2. **Notifications** (4 tools) - Unique capability, simple AppleScript
3. **Safari** (8 tools) - Very high user demand, moderate complexity
4. **Clipboard** (4 tools) - Simple, high daily utility
5. **Music** (10 tools) - Fun, good demo, easy AppleScript
6. **Finder** (10 tools) - Useful but overlaps with shell commands
7. **Photos** (7 tools) - Complex (SQLite + MediaLibrary), high value
8. **Terminal** (4 tools) - Powerful but security-sensitive

## Testing & Infrastructure (post-v1)

Current state at v1.1: 197 tests (168 CI-safe Tier 1/2, 29 local-only Tier 3). GitHub Actions CI on `macos-latest` runs `fmt` + `clippy -D warnings` + tests for both `macrelay-core` and `macrelay-menubar` on every push/PR. See [TESTING.md](TESTING.md) for the full strategy.

Possible additions:

- **Real code coverage reporting** — wire up `cargo-llvm-cov` or `cargo-tarpaulin` to CI and upload to [codecov.io](https://codecov.io). Would give real line/branch coverage percentages in PR comments and a badge for the README. Currently the README shows a static "tests: 137 passing" badge; a codecov badge would show actual coverage %.
- **Integration test coverage for UI Viewer / UI Controller** — these are Tier 2 mock-tested only because Tier 3 requires a target app in a known visual state. A dedicated "testbed app" (small SwiftUI or Tauri shell with known buttons, text fields, menus) would let us write deterministic Tier 3 UI round-trip tests.
- **Mutation testing** — run `cargo-mutants` against the escape helpers and date-math functions to catch tests that pass but don't actually prevent bugs.
- **Fuzzing** — `cargo-fuzz` or `arbitrary` against the argument-parsing and escape functions to catch weird inputs (zero bytes, unpaired surrogates, massive strings).
- **Release builds in CI** — the workflow currently builds debug on every PR. A release build job could catch optimizer-only regressions.
