# MacRelay Performance Profile

**Last updated:** 2026-04-12
**Binary:** v1.0.0 (Apple Silicon, macOS 14+)

## Summary

MacRelay is fast. The Rust binary starts in 3ms. The bottleneck is macOS itself — each AppleScript call spawns a subprocess that costs 20-50ms of system overhead before the script even runs. This is inherent to the AppleScript architecture and is the right tradeoff for modularity and maintainability.

## Benchmarks

| Metric | Value | Tool |
|---|---|---|
| Binary startup | **3.2ms** (mean) | hyperfine |
| Binary size | 6.8 MB (unstripped) | ls -lh |
| Binary size (stripped) | 5.7 MB | strip -x |
| Clean release build | 20 seconds | cargo build --release --timings |
| Incremental build | ~2 seconds | cargo build --release |
| CI-safe test suite | 137 tests in 0.01s | cargo test |
| Dependencies | 151 crates | Cargo.lock |
| Dependency vulnerabilities | 0 | cargo audit |

## Binary Size Breakdown

Top crates by contribution to the 6.8 MB binary:

| Crate | Size | What it does |
|---|---|---|
| std (Rust stdlib) | 748 KB | Standard library |
| macrelay_core | 440 KB | All 71 tools and 13 services |
| regex (tracing) | 287 KB | Log filtering |
| serde | 251 KB | JSON serialization |
| clap | 231 KB | CLI argument parsing |
| rmcp | 205 KB | MCP protocol implementation |
| tokio | 191 KB | Async runtime |
| libsqlite3 | 94 KB | Bundled SQLite for Messages |

## Where Time Goes

Tool call latency is dominated by `osascript` subprocess spawning, not Rust code:

| Phase | Time | Notes |
|---|---|---|
| Rust argument parsing + script construction | <1ms | Pure string operations |
| `osascript` process spawn (fork + exec) | 20-50ms | macOS system cost per call |
| AppleScript interpretation + app automation | 50-500ms | Depends on target app responsiveness |
| SQLite queries (Messages, Notes reads) | 1-5ms | Direct rusqlite, no subprocess |

Services that use SQLite directly (Messages search, Notes read) are significantly faster than AppleScript-based services because they skip the subprocess spawn entirely.

## Future Optimization Opportunities

These are documented for reference — none are needed for v1.0.

### Binary Size (optional)

Adding a `[profile.release]` section to `Cargo.toml` would reduce the binary from 6.8 MB to ~3 MB with no behavioral change:

```toml
[profile.release]
strip = true        # Remove debug symbols (~2 MB savings)
lto = true          # Link-time optimization (dead code elimination)
codegen-units = 1   # Better optimization (slower compile)
```

Tradeoff: clean release builds go from ~20s to ~40s. Not applied yet because 6.8 MB is fine for distribution.

### Tool Call Latency (architectural)

If AppleScript latency ever becomes a problem, three paths exist:

1. **Batch commands** — combine multiple AppleScript statements into a single `osascript` call where possible
2. **Native objc2 calls** — replace AppleScript with direct Objective-C framework calls via the `objc2-*` crates already in the dependency tree (Calendar, Reminders, and Contacts could use EventKit/CNContactStore directly)
3. **Persistent osascript** — keep a long-running `osascript` process and pipe commands to it instead of spawning a new one per call

None of these are needed today. The current architecture prioritizes modularity and maintainability — each service is a self-contained module with its own AppleScript templates, easy to read, test, and modify.

## Profiling Tools

For anyone who wants to dig deeper:

```bash
# Startup time
brew install hyperfine
hyperfine -N 'target/release/macrelay --help'

# Binary size breakdown
cargo install cargo-bloat
cargo bloat --release --bin macrelay --crates

# Compile time analysis
cargo clean && cargo build --release --timings
# Open target/cargo-timings/cargo-timing.html

# CPU profiling (interactive flamegraph)
cargo install samply
echo "" | samply record target/release/macrelay

# Memory profiling (macOS native)
# Use Instruments.app > Allocations template
```
