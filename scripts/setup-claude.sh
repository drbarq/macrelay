#!/usr/bin/env bash
set -euo pipefail

BINARY_NAME="macrelay"
INSTALL_DIR="$HOME/.local/bin"
INSTALL_PATH="$INSTALL_DIR/$BINARY_NAME"
MCP_KEY="macrelay"

# ── 1. Build the release binary ─────────────────────────────────────────────

echo "Building release binary..."
cargo build --release

BUILT_BINARY="target/release/$BINARY_NAME"
if [ ! -f "$BUILT_BINARY" ]; then
    echo "Error: build succeeded but $BUILT_BINARY not found." >&2
    exit 1
fi

# ── 2. Install the binary ───────────────────────────────────────────────────

mkdir -p "$INSTALL_DIR"
cp "$BUILT_BINARY" "$INSTALL_PATH"
chmod +x "$INSTALL_PATH"
echo "Installed $BINARY_NAME to $INSTALL_PATH"

# ── Helper: upsert an MCP server entry ───────────────────────────────────────

upsert_mcp_config() {
    local config_path="$1"
    local label="$2"

    local mcp_entry
    mcp_entry=$(jq -n --arg cmd "$INSTALL_PATH" '{"command":$cmd}')

    if [ -f "$config_path" ]; then
        local tmp
        tmp=$(mktemp)
        jq --argjson entry "$mcp_entry" \
           ".mcpServers //= {} | .mcpServers[\"$MCP_KEY\"] = \$entry" \
           "$config_path" > "$tmp" && mv "$tmp" "$config_path"
        echo "  Updated $label config: $config_path"
    else
        mkdir -p "$(dirname "$config_path")"
        jq -n --argjson entry "$mcp_entry" \
           "{mcpServers: {\"$MCP_KEY\": \$entry}}" > "$config_path"
        echo "  Created $label config: $config_path"
    fi
}

# ── 3. Cleanup old grouped connectors ───────────────────────────────────────

clean_grouped_configs() {
    local config_path="$1"
    if [ -f "$config_path" ]; then
        local tmp
        tmp=$(mktemp)
        jq 'del(.mcpServers["macrelay-pim", "macrelay-communication", "macrelay-productivity", "macrelay-navigation", "macrelay-ui", "macrelay-system"])' \
           "$config_path" > "$tmp" && mv "$tmp" "$config_path"
    fi
}

# ── 4. Configuration Logic ──────────────────────────────────────────────────

CLAUDE_DESKTOP_CONFIG="$HOME/Library/Application Support/Claude/claude_desktop_config.json"
CLAUDE_CODE_CONFIG="$HOME/.claude/mcp.json"

echo "Cleaning up old grouped connectors..."
clean_grouped_configs "$CLAUDE_DESKTOP_CONFIG"
clean_grouped_configs "$CLAUDE_CODE_CONFIG"

echo "Configuring monolithic MacRelay..."
if [ -f "$CLAUDE_DESKTOP_CONFIG" ]; then
    upsert_mcp_config "$CLAUDE_DESKTOP_CONFIG" "Claude Desktop"
fi
upsert_mcp_config "$CLAUDE_CODE_CONFIG" "Claude Code"

# ── Done ─────────────────────────────────────────────────────────────────────

echo ""
echo "Setup complete!"
echo ""
echo "  Binary installed to:  $INSTALL_PATH"
echo "  Registered as:        $MCP_KEY (monolithic)"
echo ""
echo "  Restart Claude Desktop / Claude Code to see all 71 tools under ONE extension."
