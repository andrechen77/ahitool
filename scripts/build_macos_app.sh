#! /usr/bin/env bash

set -euo pipefail

APP_NAME="ahitool"
BUNDLE_ID="com.asburyhomeimprovements.ahitool"
BIN_PATH="target/release/gui"
OUT_DIR="dist"

APP_DIR="$OUT_DIR/$APP_NAME.app"
DMG_NAME="$APP_NAME-macos.dmg"
DMG_ROOT="$OUT_DIR/dmg-root"

VERSION="1.1"

rm -rf "$APP_DIR"

mkdir -p "$APP_DIR/Contents/MacOS"

cp "$BIN_PATH" "$APP_DIR/Contents/MacOS/$APP_NAME"

cat > "$APP_DIR/Contents/Info.plist" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
 "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>$APP_NAME</string>

    <key>CFBundleIdentifier</key>
    <string>$BUNDLE_ID</string>

    <key>CFBundleName</key>
    <string>$APP_NAME</string>

    <key>CFBundleDisplayName</key>
    <string>$APP_NAME</string>

    <key>CFBundlePackageType</key>
    <string>APPL</string>

    <key>CFBundleVersion</key>
    <string>$VERSION</string>

    <key>CFBundleShortVersionString</key>
    <string>$VERSION</string>
</dict>
</plist>
EOF

echo "Built $APP_DIR"

mkdir -p "$DMG_ROOT"
cp -R "$APP_DIR" "$DMG_ROOT/"
ln -s /Applications "$DMG_ROOT/Applications"
xattr -dr com.apple.quarantine "$DMG_ROOT/$APP_NAME.app"

hdiutil create \
	-volname "$APP_NAME" \
	-srcfolder "$DMG_ROOT" \
	-ov \
	-format UDZO \
	"$DMG_NAME"

echo "Built $DMG_NAME"


