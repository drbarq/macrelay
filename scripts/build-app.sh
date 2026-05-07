#!/usr/bin/env bash
set -euo pipefail

# Build MacRelay.app — a proper macOS application bundle containing
# the menu bar app, the MCP server binary, and the MCPB bundle.

APP_NAME="MacRelay"
APP_DIR="dist/${APP_NAME}.app"
CONTENTS="${APP_DIR}/Contents"

echo "Building release binaries..."
cargo build --release

echo "Creating ${APP_NAME}.app bundle..."
rm -rf "$APP_DIR"
mkdir -p "${CONTENTS}/MacOS"
mkdir -p "${CONTENTS}/Resources"

# ── Binaries ───────────────────────────────────────────────────────────────
cp target/release/macrelay-menubar "${CONTENTS}/MacOS/macrelay-menubar"
cp target/release/macrelay "${CONTENTS}/MacOS/macrelay"
chmod +x "${CONTENTS}/MacOS/macrelay-menubar"
chmod +x "${CONTENTS}/MacOS/macrelay"

# ── Icon ───────────────────────────────────────────────────────────────────
cp assets/macrelay.icns "${CONTENTS}/Resources/macrelay.icns"

# ── MCPB bundle (embedded for Claude Desktop) ─────────────────────────────
mkdir -p "${CONTENTS}/Resources/mcpb/server"
cp mcpb/manifest.json "${CONTENTS}/Resources/mcpb/manifest.json"
cp mcpb/icon.png "${CONTENTS}/Resources/mcpb/icon.png"
# Copy server binary for MCPB bundle (not symlink — symlinks break codesign after zip)
cp target/release/macrelay "${CONTENTS}/Resources/mcpb/server/macrelay"

# ── Info.plist ─────────────────────────────────────────────────────────────
cat > "${CONTENTS}/Info.plist" << 'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>MacRelay</string>
    <key>CFBundleDisplayName</key>
    <string>MacRelay</string>
    <key>CFBundleIdentifier</key>
    <string>com.macrelay.app</string>
    <key>CFBundleVersion</key>
    <string>1.0.0</string>
    <key>CFBundleShortVersionString</key>
    <string>1.0.0</string>
    <key>CFBundleExecutable</key>
    <string>macrelay-menubar</string>
    <key>CFBundleIconFile</key>
    <string>macrelay</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>LSMinimumSystemVersion</key>
    <string>14.0</string>
    <key>LSUIElement</key>
    <true/>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSAppleEventsUsageDescription</key>
    <string>MacRelay drives Notes, Calendar, Mail, Reminders, and other Apple apps via AppleScript so that AI assistants can read and write your data on your behalf.</string>
    <key>NSCalendarsUsageDescription</key>
    <string>MacRelay reads and creates calendar events on your behalf.</string>
    <key>NSContactsUsageDescription</key>
    <string>MacRelay reads your contacts so AI assistants can look up phone numbers, emails, and addresses.</string>
    <key>NSRemindersUsageDescription</key>
    <string>MacRelay reads and creates reminders on your behalf.</string>
</dict>
</plist>
PLIST

echo ""
echo "Built: ${APP_DIR}"
echo ""
echo "  To install:  cp -r ${APP_DIR} /Applications/"
echo "  To run:      open /Applications/${APP_NAME}.app"
echo ""
echo "  The app contains:"
echo "    - Menu bar manager (macrelay-menubar)"
echo "    - MCP server (macrelay)"
echo "    - MCPB bundle for Claude Desktop"
