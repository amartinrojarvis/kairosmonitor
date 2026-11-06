#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
./scripts/build-app.sh
VERSION=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)
STAGE=$(mktemp -d "${TMPDIR:-/tmp}/kairos-dmg.XXXXXX")
trap 'rm -rf "$STAGE"' EXIT
ditto dist/KairosMonitor.app "$STAGE/KairosMonitor.app"
ln -s /Applications "$STAGE/Applications"
cp docs/INSTALAR.txt "$STAGE/LEEME.txt"
DMG="dist/KairosMonitor-${VERSION}-arm64.dmg"
hdiutil create -volname "KairosMonitor" -srcfolder "$STAGE" -ov -fs HFS+ -format UDZO -imagekey zlib-level=9 "$DMG"
hdiutil verify "$DMG"
(cd dist && shasum -a 256 "KairosMonitor-${VERSION}-arm64.dmg" > SHA256SUMS.txt)
du -h "$DMG"
