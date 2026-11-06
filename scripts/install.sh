#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
./scripts/build-app.sh
APP="$HOME/Applications/KairosMonitor.app"
CONFIG_DIR="$HOME/Library/Application Support/KairosMonitor"
LOG_DIR="$HOME/Library/Logs/KairosMonitor"
AGENT="$HOME/Library/LaunchAgents/es.amartin.kairosmonitor.plist"
mkdir -p "$HOME/Applications" "$CONFIG_DIR" "$LOG_DIR" "$(dirname "$AGENT")"
launchctl bootout "gui/$(id -u)/es.amartin.kairosmonitor" 2>/dev/null || true
pkill -TERM -f "^$APP/Contents/MacOS/kairosmonitor( |$)" 2>/dev/null || true
# Only replace this application's bundle; preserve the user's configuration.
ditto dist/KairosMonitor.app "$APP"
if [ ! -f "$CONFIG_DIR/config.toml" ]; then cp config.example.toml "$CONFIG_DIR/config.toml"; fi
python3 - "$APP" "$LOG_DIR" "$AGENT" <<'PY'
import plistlib, sys
app, logs, dest = sys.argv[1:]
with open(dest, 'wb') as f:
    plistlib.dump({'Label':'es.amartin.kairosmonitor',
        'ProgramArguments':[app+'/Contents/MacOS/kairosmonitor'],
        'RunAtLoad':True, 'ProcessType':'Interactive',
        'StandardOutPath':logs+'/stdout.log', 'StandardErrorPath':logs+'/stderr.log'}, f)
PY
launchctl bootstrap "gui/$(id -u)" "$AGENT"
printf 'Instalado en %s; arranca al iniciar sesión.\n' "$APP"
