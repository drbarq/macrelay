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

# ── Helper: upsert the MCP server entry in a JSON config file ───────────────

upsert_mcp_config() {
    local config_path="$1"
    local label="$2"

    local mcp_entry
    mcp_entry=$(cat <<ENTRY
{"command":"$INSTALL_PATH"}
ENTRY
)

    if [ -f "$config_path" ]; then
        # File exists – merge into .mcpServers
        local tmp
        tmp=$(mktemp)
        # Ensure .mcpServers exists then set the key
        jq --argjson entry "$mcp_entry" \
           ".mcpServers //= {} | .mcpServers[\"$MCP_KEY\"] = \$entry" \
           "$config_path" > "$tmp" && mv "$tmp" "$config_path"
        echo "  Updated existing $label config: $config_path"
    else
        # File does not exist – create it
        mkdir -p "$(dirname "$config_path")"
        jq -n --argjson entry "$mcp_entry" \
           "{mcpServers: {\"$MCP_KEY\": \$entry}}" > "$config_path"
        echo "  Created $label config: $config_path"
    fi
}

# ── 3. Configure Claude Desktop ─────────────────────────────────────────────

CLAUDE_DESKTOP_CONFIG="$HOME/Library/Application Support/Claude/claude_desktop_config.json"

echo ""
echo "Configuring Claude Desktop..."
if [ -f "$CLAUDE_DESKTOP_CONFIG" ]; then
    upsert_mcp_config "$CLAUDE_DESKTOP_CONFIG" "Claude Desktop"
else
    echo "  Claude Desktop config not found at:"
    echo "    $CLAUDE_DESKTOP_CONFIG"
    echo "  Skipping. (Install Claude Desktop first, then re-run this script.)"
fi

# ── 4. Configure Claude Code ────────────────────────────────────────────────

CLAUDE_CODE_CONFIG="$HOME/.claude/mcp.json"

echo ""
echo "Configuring Claude Code..."
upsert_mcp_config "$CLAUDE_CODE_CONFIG" "Claude Code"

# ── Done ─────────────────────────────────────────────────────────────────────

echo ""
echo "Setup complete!"
echo ""
echo "  Binary installed to:  $INSTALL_PATH"
echo "  MCP server key:       $MCP_KEY"
echo ""
echo "  The following MCP configs were updated:"
[ -f "$CLAUDE_DESKTOP_CONFIG" ] && echo "    - Claude Desktop: $CLAUDE_DESKTOP_CONFIG"
echo "    - Claude Code:    $CLAUDE_CODE_CONFIG"
echo ""
echo "  Restart Claude Desktop / Claude Code to pick up the new server."
