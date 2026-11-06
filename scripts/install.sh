#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
./scripts/build-app.sh
APP="$HOME/Applications/KairosMonitor.app"
mkdir -p "$HOME/Applications"
# Stop the old instance without changing its next-login preference.
launchctl bootout "gui/$(id -u)/es.amartin.kairosmonitor" 2>/dev/null || true
launchctl bootout "gui/$(id -u)/es.amartin.kairosmonitor.login" 2>/dev/null || true
pkill -TERM -f "^$APP/Contents/MacOS/kairosmonitor( |$)" 2>/dev/null || true
# Wait for the old process to leave LaunchServices before replacing its bundle.
for attempt in {1..30}; do
    if ! pgrep -f "^$APP/Contents/MacOS/kairosmonitor( |$)" >/dev/null; then break; fi
    sleep 0.1
done
if pgrep -f "^$APP/Contents/MacOS/kairosmonitor( |$)" >/dev/null; then
    printf 'Cierra KairosMonitor antes de actualizar.\n' >&2
    exit 1
fi
# Replace only this bundle; configuration and login preference remain untouched.
rm -rf "$APP"
ditto dist/KairosMonitor.app "$APP"
if ! open "$APP"; then
    sleep 1
    open "$APP"
fi
printf 'Instalado en %s. El inicio de sesión se controla desde Ajustes.\n' "$APP"
