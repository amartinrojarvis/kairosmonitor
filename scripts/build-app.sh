#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
CARGO_BIN="${CARGO_BIN:-$HOME/.cargo/bin/cargo}"
export MACOSX_DEPLOYMENT_TARGET=12.0
"$CARGO_BIN" build --release --locked --target aarch64-apple-darwin
APP="$PWD/dist/KairosMonitor.app"
# Rebuild the generated bundle so older fonts/resources cannot leak into a release.
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp target/aarch64-apple-darwin/release/kairosmonitor "$APP/Contents/MacOS/kairosmonitor"
ICONSET="$PWD/target/KairosMonitor.iconset"
if [ ! -f target/KairosMonitor.icns ] || [ scripts/icon.swift -nt target/KairosMonitor.icns ]; then
    swift scripts/icon.swift "$ICONSET"
    iconutil -c icns "$ICONSET" -o target/KairosMonitor.icns
fi
cp target/KairosMonitor.icns "$APP/Contents/Resources/"
python3 - "$APP" <<'PY'
import plistlib, re, sys
from pathlib import Path
version = re.search(r'^version = "([^"]+)"', Path('Cargo.toml').read_text(), re.M)[1]
info = {
    'CFBundleExecutable': 'kairosmonitor',
    'CFBundleIdentifier': 'es.amartin.kairosmonitor',
    'CFBundleName': 'KairosMonitor',
    'CFBundleDisplayName': 'Kairos Monitor',
    'CFBundlePackageType': 'APPL',
    'CFBundleShortVersionString': version,
    'CFBundleVersion': version,
    'CFBundleIconFile': 'KairosMonitor',
    'LSMinimumSystemVersion': '12.0',
    'LSArchitecturePriority': ['arm64'],
    'LSUIElement': True,
    'NSHighResolutionCapable': True,
    'NSLocalNetworkUsageDescription': 'Kairos Monitor consulta las métricas del servidor que configures.',
    # Users can select their own HTTP endpoint on LAN/Tailscale, as in v0.1.
    'NSAppTransportSecurity': {'NSAllowsArbitraryLoads': True},
}
with open(Path(sys.argv[1]) / 'Contents/Info.plist', 'wb') as f:
    plistlib.dump(info, f)
PY
codesign --force --sign "${CODE_SIGN_IDENTITY:--}" "$APP"
codesign --verify --strict --verbose=2 "$APP"
plutil -lint "$APP/Contents/Info.plist"
lipo -verify_arch arm64 "$APP/Contents/MacOS/kairosmonitor"
printf '%s\n' "$APP"
