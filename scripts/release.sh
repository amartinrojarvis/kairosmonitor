#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
CARGO_BIN="${CARGO_BIN:-$HOME/.cargo/bin/cargo}"
REPO="amartinrojarvis/kairosmonitor"
VERSION=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo 'Usa una versión estable MAJOR.MINOR.PATCH.' >&2; exit 1; }
[[ -z "$(git status --porcelain)" ]] || { echo 'Haz commit de los cambios antes de publicar.' >&2; exit 1; }
[[ "$(git remote get-url origin)" == "https://github.com/$REPO.git" ]] || { echo 'El remoto origin no es el repositorio oficial.' >&2; exit 1; }
command -v gh >/dev/null || { echo 'Se necesita GitHub CLI (gh) autenticada.' >&2; exit 1; }
if git rev-parse "refs/tags/v$VERSION" >/dev/null 2>&1; then echo 'El tag ya existe; incrementa la versión.' >&2; exit 1; fi
gh auth status >/dev/null
LATEST=$(gh release view --repo "$REPO" --json tagName --jq .tagName)
python3 - "$VERSION" "$LATEST" <<'PYVERSION'
import sys
current, latest = sys.argv[1:]
try:
    newer = tuple(map(int, current.split('.'))) > tuple(map(int, latest.removeprefix('v').split('.')))
except ValueError:
    raise SystemExit('La última release no tiene una versión estable comparable.')
if not newer:
    raise SystemExit('La versión debe ser mayor que la última release publicada.')
PYVERSION
"$CARGO_BIN" fmt -- --check
"$CARGO_BIN" test --locked
"$CARGO_BIN" clippy --locked --all-targets -- -D warnings
./scripts/package.sh
NOTES=$(mktemp "${TMPDIR:-/tmp}/kairos-release.XXXXXX")
trap 'rm -f "$NOTES"' EXIT
python3 - "$VERSION" "$NOTES" <<'PY'
import pathlib, sys
version, dest = sys.argv[1:]
text = pathlib.Path('CHANGELOG.md').read_text()
section = text.split('## ' + version + '\n', 1)
if len(section) != 2:
    raise SystemExit('Falta la sección de la versión en CHANGELOG.md')
notes = section[1].split('\n## ', 1)[0].strip()
pathlib.Path(dest).write_text(notes + '\n\nSolo Apple Silicon, macOS 12 o posterior. Firma ad hoc. Consulta el README para instalar. Se adjunta SHA256SUMS.txt.\n')
PY
git push origin HEAD:main
git tag "v$VERSION"
git push origin "v$VERSION"
gh release create "v$VERSION" "dist/KairosMonitor-$VERSION-arm64.dmg" dist/SHA256SUMS.txt --repo "$REPO" --verify-tag --latest --title "Kairos Monitor $VERSION" --notes-file "$NOTES"
