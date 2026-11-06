#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
CARGO_BIN="${CARGO_BIN:-$HOME/.cargo/bin/cargo}"
"$CARGO_BIN" build --release --locked
APP="$PWD/dist/KairosMonitor.app"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources/Fonts"
cp target/release/kairosmonitor "$APP/Contents/MacOS/kairosmonitor"
cp assets/JetBrainsMonoNerdFont-Regular.ttf "$APP/Contents/Resources/Fonts/"
cp assets/FONT-LICENSE.txt "$APP/Contents/Resources/"
cat > "$APP/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>kairosmonitor</string>
<key>CFBundleIdentifier</key><string>es.amartin.kairosmonitor</string>
<key>CFBundleName</key><string>KairosMonitor</string>
<key>CFBundleDisplayName</key><string>Kairos Monitor</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>0.1.0</string>
<key>CFBundleVersion</key><string>1</string>
<key>LSMinimumSystemVersion</key><string>12.0</string>
<key>LSUIElement</key><true/>
<key>NSHighResolutionCapable</key><true/>
<key>ATSApplicationFontsPath</key><string>Fonts</string>
</dict></plist>
PLIST
codesign --force --sign - "$APP"
plutil -lint "$APP/Contents/Info.plist"
printf '%s\n' "$APP"
