# MacRelay Security Audit Report

**Date:** 2026-04-12
**Version:** 1.0.0
**Auditors:** Claude Opus 4.6 (automated multi-agent review)

## Security Posture Summary

MacRelay runs 100% locally with no network communication, no telemetry, and no cloud dependencies. The attack surface is limited to:
1. User-provided tool parameters (via MCP JSON-RPC over stdio)
2. macOS app interactions (AppleScript/JXA, SQLite, Accessibility API)

All identified injection vulnerabilities were **found and fixed during this audit**. The codebase is now clean.

## Methodology

Four parallel security review agents analyzed the codebase:
- **Full security audit** — OWASP-style review of all 71 tools across 13 services
- **Escape helper deep review** — Line-by-line analysis of `escape.rs` and all call sites
- **SQL injection audit** — Review of all SQLite database access patterns
- **Dependency audit** — `cargo audit` against RustSec advisory database (151 crates, 0 advisories)

## Findings (All Resolved)

### CRITICAL: AppleScript Injection in Contacts Search

**File:** `crates/macrelay-core/src/services/contacts/mod.rs`
**Issue:** The `query` parameter was interpolated directly into AppleScript without any escaping. A crafted query like `" & do shell script "id" & "` would execute arbitrary shell commands.
**Fix:** Added `escape_applescript_string()` to all user inputs.

### CRITICAL: AppleScript Injection via Unvalidated Timestamps in Calendar

**File:** `crates/macrelay-core/src/services/calendar/mod.rs`
**Issue:** `start_date` and `end_date` string parameters were interpolated as `set startEpoch to {start_str} as number` — outside any string delimiters. A value like `0\ndo shell script "id"` injects arbitrary AppleScript.
**Fix:** Parse all timestamp parameters as `i64` before interpolation. Invalid values return an error result.

### HIGH: Unescaped list_name in Reminders Search

**File:** `crates/macrelay-core/src/services/reminders/mod.rs`
**Issue:** `list_filter` interpolated into `repeat with l in {list "{list_filter}"}` without escaping, while other fields in the same file were properly escaped.
**Fix:** Applied `escape_applescript_string(list_filter)`.

### HIGH: Ad-hoc Escape in Maps Handlers

**File:** `crates/macrelay-core/src/services/maps/mod.rs`
**Issue:** All four map handlers used `.replace('"', "\\\"")` instead of `escape_applescript_string()`. This missed backslash escaping entirely.
**Fix:** Replaced all `.replace()` calls with `escape_applescript_string()`.

### HIGH: Missing Double-Escape in Stickies Read

**File:** `crates/macrelay-core/src/services/stickies/mod.rs`
**Issue:** `sticky_id` was shell-escaped but embedded in an AppleScript `do shell script "..."` string without AppleScript-level escaping.
**Fix:** Applied `escape_applescript_string(&escape_shell_single_quoted(sticky_id))`.

### HIGH: Missing Double-Escape in UI Controller Force-Quit

**File:** `crates/macrelay-core/src/services/ui_controller/mod.rs`
**Issue:** Same double-escape gap — `escape_shell_single_quoted` output embedded in AppleScript `"..."` without `escape_applescript_string`.
**Fix:** Applied `escape_applescript_string(&escape_shell_single_quoted(app_name))`.

### HIGH: Unescaped Inputs in eventkit.rs create_event

**File:** `crates/macrelay-core/src/macos/eventkit.rs`
**Issue:** `title`, `location`, `notes` interpolated into AppleScript without escaping. Currently no external call sites, but the function is `pub`.
**Fix:** Added `escape_applescript_string()` to all three parameters.

### MEDIUM: Unescaped Key Name in Return String

**File:** `crates/macrelay-core/src/services/ui_controller/mod.rs`
**Issue:** Raw `key` parameter in `return "Pressed key: {key}"` AppleScript string.
**Fix:** Used `escaped_key` from `escape_applescript_string`.

### MEDIUM: Newline Handling in escape_applescript_string

**File:** `crates/macrelay-core/src/macos/escape.rs`
**Issue:** Did not escape `\n` or `\r`. AppleScript double-quoted strings don't support literal newlines — multi-line inputs would cause parse errors.
**Fix:** Added `.replace('\n', "\\n").replace('\r', "\\r")`.

## Clean Areas

### SQL Access — No Issues

Only Messages uses SQLite. All three queries use static SQL with `rusqlite::params![]` parameterized bindings and `SQLITE_OPEN_READ_ONLY` connections.

### Dependencies — No Issues

`cargo audit`: 151 crate dependencies, **zero vulnerabilities**.

### Services Confirmed Clean (no changes needed)

- Calendar (string fields) — `escape_applescript_string` on all string fields
- Reminders (title, notes) — `escape_applescript_string` on all string fields
- Notes — `escape_applescript_string` on all fields
- Mail — `escape_applescript_string` on all fields
- Messages — `escape_applescript_string` for sends; parameterized SQL for reads
- Shortcuts — correct double-wrap: `escape_applescript_string(&escape_shell_single_quoted(...))`
- Location — no user input in scripts
- Permissions Status — no user input
- UI Viewer — `escape_applescript_string` on all inputs

### What the Codebase Gets Right

- **Escape functions are correctly implemented** with backslash-first ordering and comprehensive tests
- **Escaping applied consistently in ~90% of call sites** — bugs were omissions, not systematic failure
- **MCP transport isolation** — logging to stderr, JSON-RPC to stdout, no cross-channel leakage
- **Permission checking uses native FFI** (AXIsProcessTrusted, EKEventStore, etc.)
- **Rust memory safety** eliminates buffer overflows, use-after-free, and similar vulnerability classes

## Informational Notes

### UI Automation Risk Surface

The UI Controller can click, type, press keys, and force-quit apps. This is inherent to its purpose. macOS Accessibility permission acts as the gate.

### LIKE Pattern Widening

In Messages search, `format!("%{query}%")` preserves `%` and `_` wildcards from user input. Not SQL injection (read-only, parameterized), but allows broader searches than intended. Acceptable for a single-user local tool.

## Verification

After all fixes:
- `cargo fmt -- --check` — pass
- `cargo clippy --all-targets -- -D warnings` — pass
- `cargo test -p macrelay-core --lib` — 137/137 pass
- `cargo audit` — 0 vulnerabilities
- `grep -ri "mac-app-oss" crates/` — clean
