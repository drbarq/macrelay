# Testing Strategy

## Why this doc exists

The first version of this doc was written after a prior session generated ~70 tautological mock tests — mocks that ignored their input and returned a hardcoded string, paired with assertions checking that the hardcoded string appeared in the output. They proved the runner got called and nothing more.

That cleanup is now done. Every formerly tautological mock has been rewritten to inspect the script the handler generates. The doc still defines the strategy, but the audit and cleanup-plan sections now reflect the post-cleanup state.

## Goals

1. **Catch regressions before they ship** — every tool has tests that would fail if its script generation, input parsing, or output formatting broke.
2. **Run in CI with no personal data** — Tier 1 and Tier 2 must run on a fresh GitHub Actions `macos-latest` runner with no Automation, Accessibility, or Full Disk Access permissions and no access to anyone's real Calendar/Mail/Notes/Messages.
3. **Validate end-to-end against real apps before release** — Tier 3 lives on the maintainer's Mac, runs against real personal accounts, and is gated behind `--ignored` so it never runs in CI.
4. **No tautological tests.** A test that asserts `mock_returns == output_after_passthrough` is worse than no test — it gives false confidence and inflates the test count.

## Test tiers

### Tier 1 — Pure unit tests (CI-safe, no osascript)

**What they validate:**
- Tool schemas are valid JSON Schema
- Every expected tool name is registered
- Tool counts per service match the spec (8 calendar, 13 mail, etc.)
- Pure helper functions: date parsing, ref ID resolution, AppleScript string escaping, XPath builders, etc.

**Constraints:**
- No `osascript` calls. No `tokio` task-locals. No filesystem access outside `target/`.
- Can run on Linux, macOS, anywhere `cargo test` works.

**Where they run:** `cargo test -p macrelay-core --lib` on every push and PR.

**Current state:** 13 `test_tool_schemas_valid` tests cover registration. Pure helper coverage has started — `ui_controller::tests::test_key_name_to_code` validates the AppleScript key code map (space=49, escape=53, f12=111, etc.) and `messages::tests::test_unix_days_to_date` validates Apple's epoch conversion. More extracted helpers (escape functions, date parsers, ref ID resolvers) should follow this pattern so they can be tested directly without going through the mock harness.

### Tier 2 — Mock-runner tests (CI-safe, requires macos-latest)

**What they validate:**
- Given specific tool args, the handler generates AppleScript/JXA containing the expected fragments (commands, escaped values, parameter substitution).
- Given a realistic raw response from `osascript`, the handler parses it correctly and returns the expected JSON/text.
- Error responses from `osascript` produce graceful, human-readable errors instead of panics.
- Required-parameter validation rejects missing args.
- Edge cases: empty results, multi-line output, special characters that need escaping.

**Mock patterns in use:**

The simple shared pattern (calendar, mail, reminders, notes, contacts, ui_viewer, maps, stickies, shortcuts, messages send) — each test instantiates `AssertingMock` with a list of fragments and a response:

```rust
struct AssertingMock {
    expected_fragments: Vec<&'static str>,
    response: String,
}

impl ScriptRunner for AssertingMock {
    fn run_applescript(&self, script: &str) -> Result<String> {
        for fragment in &self.expected_fragments {
            assert!(script.contains(fragment),
                "Script missing fragment: {}\nScript content:\n{}", fragment, script);
        }
        Ok(self.response.clone())
    }
    fn run_applescript_with_timeout(&self, script: &str, _timeout: Duration) -> Result<String> {
        self.run_applescript(script)
    }
    fn run_jxa(&self, _script: &str) -> Result<String> { unimplemented!() }
}
```

The queue-based pattern (`ui_controller`) — for tools that make multiple sequential AppleScript calls. Each call pops the next `(expected_fragment, response)` pair off the queue:

```rust
struct AssertingMock {
    expectations: Mutex<Vec<(String, Result<String, String>)>>,
}
// .expect("click at {100, 200}", Ok("Clicked at (100, 200)"))
// .expect("activate", Ok(""))
```

Both patterns **must** inspect `script`. The only acceptable use of `_script: &str` is on a runner method the test doesn't exercise (e.g., `run_jxa` in a calendar test, where calendar never calls JXA — that branch stays `unimplemented!()`).

**A mock where the primary runner method takes `_script` is a code review block.**

**Constraints:**
- Runs on `macos-latest` GitHub Actions runners. `osascript` exists, but tests don't actually call it (the mock intercepts).
- No Automation/Accessibility/FDA permissions required, no personal data touched.
- Should still pass on a clean macOS install with no apps configured.

**Where they run:** `cargo test -p macrelay-core --lib` on every push and PR.

**Current state:** Every service follows this pattern. Calendar, mail, reminders, notes, contacts, ui_viewer, ui_controller, maps, stickies, shortcuts, and the messages send tool all use `AssertingMock` (or its queue-based variant in `ui_controller`) and assert on real script fragments. Examples of what's actually being checked:

- `calendar_create_event` asserts the generated AppleScript contains `summary:\"New Meeting\"`, `location:\"Room 101\"`, `description:\"Important notes\"` — validates parameter substitution and quoting.
- `calendar_find_available_times` mocks a busy time range, then asserts the computed free-slot start/end values match the working-hours math — would catch real algorithm regressions.
- `ui_controller_click` (coords) asserts `click at {100, 200}`. `ui_controller_click` (element) asserts `click button "Submit" of window 1`.
- `messages_send_message` asserts `1st account whose service type = iMessage`, `participant "test@example.com"`, `send "hello world"`.

These tests would fail if escaping broke, if a parameter stopped being threaded through, or if the underlying script template was edited incorrectly.

### Tier 3 — Integration tests against real apps (local-only)

**What they validate:**
- Handlers actually work end-to-end against real Calendar, Reminders, Notes, Mail, Messages, etc.
- Round-trips: create → search → update → delete.
- Multi-account behavior (Notes iCloud vs On My Mac, Mail across accounts).
- Permission error paths (when an app refuses Automation, when Full Disk Access is missing).

**Constraints:**
- Touches the maintainer's real accounts. Will create and delete real events, reminders, notes.
- Must clean up after itself (every `create_*` test pairs with `delete_*` in the same test).
- Gated behind `#[ignore]` so `cargo test` never runs them by default.
- Run with `cargo test -- --ignored` only on the maintainer's Mac, before tagging a release.

**Where they run:** Maintainer's Mac, manually, before each release. **Never in CI.**

**Current state:** Three integration test files exist:
- `tests/calendar_integration.rs` — 5 tests, covers create/search/update/cancel round-trips. Good template.
- `tests/notes_integration.rs` — 4 tests, covers multi-account write/delete/restore. Good template.
- `tests/smoke_all_tools.rs` — 1 test, hits every registered tool with valid args and asserts it doesn't crash. Excellent regression net.

We need to extend Tier 3 to cover the remaining 11 services with the same round-trip pattern.

### Tier 4 — Manual / exploratory testing

**What it validates:**
- Permission flows on a fresh Mac (TCC prompts, error messages).
- Real Claude Desktop / Claude Code MCP integration end-to-end.
- UI Viewer / UI Controller against real apps in real visual states.

**Where it runs:** Pre-release checklist on a clean macOS install. Cannot be automated.

## Audit of current 197 tests

```
$ cargo test -p macrelay-core --lib
test result: ok. 137 passed; 0 failed; 9 ignored; 0 measured; ~10s

$ cargo test -p macrelay-menubar
test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; ~0s

$ cargo test -p macrelay-core --all-targets
passed: 137  ignored: 29  total: 166 (core only)
```

| Tier | Count | Location | Status |
|---|---:|---|---|
| Tier 1 — `test_tool_schemas_valid` (one per service) | 13 | `src/services/*/mod.rs` | Done |
| Tier 1 — pure helper unit tests (escape, key codes, date math) | ~10 | `src/macos/escape.rs`, `services/ui_controller`, `services/messages` | Done |
| Tier 2 — script-inspecting mocks (happy path, error path, escape/injection, required-param) | ~113 | `src/services/*/mod.rs` | Done |
| Tier 2 — `test_mock_runner` harness self-test | 1 | `src/macos/applescript.rs` | Keep |
| Tier 2 — menu bar app (config writer, toggles, uninstall, plist, permissions) | 31 | `crates/macrelay-menubar/src/*.rs` | Done |
| Tier 3 — `#[ignore]`d lib tests (CoreLocation, TCC, SQLite reads) | 9 | `src/services/*/mod.rs`, `src/macos/applescript.rs` | Local-only |
| Tier 3 — `#[ignore]`d integration files (real-app round-trips) | 20 | `tests/{calendar,notes,reminders,mail,contacts,messages}_integration.rs` + `smoke_all_tools.rs` | Local-only |
| **Tier 1 + Tier 2 (CI-safe)** | **168** | — | Runs on `macos-latest` GitHub Actions |
| **Tier 3 (`#[ignore]`d, local-only)** | **29** | — | Run with `cargo test -p macrelay-core --all-targets -- --include-ignored` |

## Current Coverage Report

This table provides a high-signal overview of what is validated for each service. 100% of the 71 registered tools have Tier 2 mock-based coverage.

| Service | Tier 1/2 Coverage | Tier 3 (Real App) | Permissions Required |
|---|---|---|---|
| **Calendar** | 8/8 tools | Round-trip | Automation (Calendar) |
| **Reminders** | 7/7 tools | Round-trip | Automation (Reminders) |
| **Notes** | 8/8 tools | Round-trip | Automation (Notes) |
| **Mail** | 13/13 tools | Read-only | Automation (Mail) |
| **Messages** | 4/4 tools | Read-only | Full Disk Access |
| **Contacts** | 2/2 tools | Read-only | Automation (Contacts) |
| **Maps** | 4/4 tools | URL Smoke | None |
| **Location** | 1/1 tool | Real Check | Location Services |
| **UI Viewer** | 6/6 tools | Mock-only | Accessibility |
| **UI Controller** | 10/10 tools | Mock-only | Accessibility |
| **Stickies** | 4/4 tools | Mock-only | Automation (Stickies) |
| **Shortcuts** | 3/3 tools | Mock-only | None |
| **Permissions** | 1/1 tool | Real Check | None |
| **Menu Bar App** | 31 tests | — | None (tempdir-based) |

*Note: "Mock-only" means the tools are validated for script generation but haven't been added to the Tier 3 round-trip suite yet.*

## Known gaps

These don't block CI but should be fixed when the surrounding work is touched:

1. **Integration test assertions could be stronger.** Some Tier 3 tests (Messages, Contacts) currently assert that the output "contains Found or No results", which is a "doesn't panic" smoke check. As these services mature, they should grow into real round-trips with stricter assertions on returned data.
2. **More pure helpers should be extracted for Tier 1.** Anything that builds an AppleScript fragment, escapes a value, parses a date, or formats a response is a pure function and should be tested directly, not only through the mock harness. The escape helpers in particular catch injection bugs that script-fragment assertions can miss.
3. **Tier 3 round-trip coverage for UI tools.** Currently, 6 of 13 services have integration test files. The UI Viewer and UI Controller tools are mock-tested for script generation but lack Tier 3 integration tests because they require specific target applications to be in known visual states.

## CI status

GitHub Actions runs three gates on every push and PR, on `macos-latest`:

1. `cargo fmt -- --check` — formatting must match rustfmt
2. `cargo clippy --all-targets -- -D warnings` — lints are errors
3. `cargo test -p macrelay-core --lib` — all 137 CI-safe core tests must pass
4. `cargo test -p macrelay-menubar` — all 31 menubar tests must pass

All three must be green. Tier 3 tests are gated behind `#[ignore]` and never run in CI — they require personal accounts and TCC-granted permissions, which CI does not have.

See the workflow definition at `.github/workflows/ci.yml`.

Pre-release ritual (manual, on maintainer's Mac):
1. `cargo test -p macrelay-core --all-targets -- --include-ignored` — runs all 29 Tier 3 tests (9 lib + 20 integration files)
1. `cargo test -p macrelay-menubar` — all 31 menubar tests
2. Manual exploratory pass through Claude Desktop / Claude Code against a couple of services, including a permission-denied scenario

## Rules going forward

- **A mock where the primary runner method takes `_script` is a code review block.** If the test doesn't inspect what the handler generates, it isn't testing the handler. Unused stub branches (e.g., `run_jxa` in a calendar test) may use `_script` and `unimplemented!()`.
- **Every new tool needs Tier 1 (schema), Tier 2 (script generation + parsing), and Tier 3 (real round-trip) tests before it ships.**
- **Tier 3 tests must clean up after themselves.** No leftover events, reminders, notes, drafts.
- **CI never touches personal data.** If a test needs my Calendar, my Mail, my Notes, or any TCC-gated API, it's Tier 3 and `#[ignore]`d.
- **Don't celebrate test counts.** 93 tests that catch real bugs beats 101 that pass when the implementation is wrong. The current 93 are real because every Tier 2 mock asserts on the script the handler generates — verify that property holds before adding to the count.
