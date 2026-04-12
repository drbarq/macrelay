#!/usr/bin/env bash
set -euo pipefail

# Uninstall MacRelay — removes all installed files and config entries.
# Safe to run multiple times. Does not touch source code or repo.

echo "Uninstalling MacRelay..."
echo ""

# ── 1. Kill running processes ──────────────────────────────────────────────
if pgrep -x macrelay-menubar > /dev/null 2>&1; then
    echo "  Stopping macrelay-menubar..."
    pkill -x macrelay-menubar || true
    sleep 1
fi

# ── 2. Remove app bundle ──────────────────────────────────────────────────
if [ -d "/Applications/MacRelay.app" ]; then
    echo "  Removing /Applications/MacRelay.app"
    rm -rf "/Applications/MacRelay.app"
else
    echo "  /Applications/MacRelay.app — not found, skipping"
fi

# ── 3. Remove standalone binaries ─────────────────────────────────────────
for bin in macrelay macrelay-menubar; do
    if [ -f "$HOME/.local/bin/$bin" ]; then
        echo "  Removing ~/.local/bin/$bin"
        rm -f "$HOME/.local/bin/$bin"
    else
        echo "  ~/.local/bin/$bin — not found, skipping"
    fi
done

# ── 4. Remove LaunchAgent ─────────────────────────────────────────────────
PLIST="$HOME/Library/LaunchAgents/com.macrelay.menubar.plist"
if [ -f "$PLIST" ]; then
    echo "  Removing LaunchAgent plist"
    launchctl bootout "gui/$(id -u)" "$PLIST" 2>/dev/null || true
    rm -f "$PLIST"
else
    echo "  LaunchAgent — not found, skipping"
fi

# ── 5. Remove MacRelay preferences ────────────────────────────────────────
PREFS_DIR="$HOME/Library/Application Support/MacRelay"
if [ -d "$PREFS_DIR" ]; then
    echo "  Removing preferences ($PREFS_DIR)"
    rm -rf "$PREFS_DIR"
else
    echo "  Preferences — not found, skipping"
fi

# ── 6. Remove Claude Desktop extension ────────────────────────────────────
EXT_DIR="$HOME/Library/Application Support/Claude/Claude Extensions/com.macrelay.app"
if [ -d "$EXT_DIR" ]; then
    echo "  Removing Claude Desktop extension"
    rm -rf "$EXT_DIR"
else
    echo "  Claude Desktop extension — not found, skipping"
fi

# ── 7. Remove MacRelay entry from Claude Desktop config (legacy) ──────────
CLAUDE_DESKTOP="$HOME/Library/Application Support/Claude/claude_desktop_config.json"
if [ -f "$CLAUDE_DESKTOP" ]; then
    if jq -e '.mcpServers["MacRelay"] // .mcpServers["macrelay"]' "$CLAUDE_DESKTOP" > /dev/null 2>&1; then
        echo "  Removing MacRelay from Claude Desktop config"
        tmp=$(mktemp)
        jq 'del(.mcpServers["MacRelay", "macrelay"])' "$CLAUDE_DESKTOP" > "$tmp" && mv "$tmp" "$CLAUDE_DESKTOP"
    else
        echo "  Claude Desktop config — no MacRelay entry, skipping"
    fi
else
    echo "  Claude Desktop config — not found, skipping"
fi

# ── 7. Remove MacRelay entry from Claude Code config ──────────────────────
CLAUDE_CODE="$HOME/.claude/mcp.json"
if [ -f "$CLAUDE_CODE" ]; then
    if jq -e '.mcpServers["MacRelay"] // .mcpServers["macrelay"]' "$CLAUDE_CODE" > /dev/null 2>&1; then
        echo "  Removing MacRelay from Claude Code config"
        tmp=$(mktemp)
        jq 'del(.mcpServers["MacRelay", "macrelay"])' "$CLAUDE_CODE" > "$tmp" && mv "$tmp" "$CLAUDE_CODE"
    else
        echo "  Claude Code config — no MacRelay entry, skipping"
    fi
else
    echo "  Claude Code config — not found, skipping"
fi

echo ""
echo "MacRelay uninstalled."
echo ""
echo "  Note: macOS permission grants (Accessibility, Calendar, etc.) persist"
echo "  in System Settings. Remove them manually if desired:"
echo "  System Settings > Privacy & Security > [permission type]"
