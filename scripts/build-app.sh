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

# ── Codesigning ────────────────────────────────────────────────────────────
# Sign the bundle with a Developer ID cert if one is available in the
# keychain. This is the same identity CI uses (release.yml). Critically:
# without proper signing, replacing /Applications/MacRelay.app with a local
# build BREAKS the user's TCC csreq for permissions like Calendar Full
# Access — System Settings still shows the toggle on, but EventKit silently
# degrades to write-only because the running binary's code identity
# doesn't match the original grant. If you don't have the cert, set
# MACRELAY_SKIP_CODESIGN=1 to accept the ad-hoc-signed build (read-only
# Calendar/Reminders will be broken until you reinstall a signed copy).
IDENTITY_NAME="Developer ID Application: Joseph Tustin (X98GWB4NWD)"
ENTITLEMENTS="macrelay.entitlements"

if [[ "${MACRELAY_SKIP_CODESIGN:-0}" == "1" ]]; then
    echo "Skipping codesign (MACRELAY_SKIP_CODESIGN=1) — Calendar/Reminders read access will not work against existing TCC grants."
elif security find-identity -v -p codesigning | grep -q "$IDENTITY_NAME"; then
    echo "Codesigning with $IDENTITY_NAME..."
    # Sign nested executables first (codesign is bottom-up).
    #
    # CRITICAL: identifier MUST be "com.macrelay.app" (not ".server"), even
    # for the inner binaries. Reason: when Claude Desktop launches macrelay
    # as a stdio child, it execs the binary directly — no bundle binding.
    # EventKit's in-process TCC check then uses the binary's signed
    # identifier as the lookup key. The user's Calendar grant has a csreq
    # of `identifier "com.macrelay.app" ...`; if we sign the binary as
    # `com.macrelay.server`, the csreq fails to match and the grant
    # silently degrades to write-only (EKAuth=3) — which is invisible in
    # System Settings (toggle still shows Full Access) and reports as
    # "granted" in our own permissions_status. This wasted ~2 hours of
    # debugging in the v1.2.4 session, hence the long comment.
    codesign --sign "$IDENTITY_NAME" --options runtime --entitlements "$ENTITLEMENTS" \
        --identifier "com.macrelay.app" --force \
        "${CONTENTS}/MacOS/macrelay"
    codesign --sign "$IDENTITY_NAME" --options runtime --entitlements "$ENTITLEMENTS" \
        --identifier "com.macrelay.app" --force \
        "${CONTENTS}/Resources/mcpb/server/macrelay"
    # Then the outer bundle (signs the menubar binary with the bundle id).
    codesign --sign "$IDENTITY_NAME" --options runtime --entitlements "$ENTITLEMENTS" \
        --force "$APP_DIR"
    codesign --verify --deep --strict "$APP_DIR"
    echo "Signed and verified."
else
    echo "WARNING: Developer ID cert not in keychain; falling back to ad-hoc signing."
    echo "         The resulting build will NOT satisfy TCC csreq for Calendar/Reminders."
    echo "         Set MACRELAY_SKIP_CODESIGN=1 to suppress this message."
fi

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
