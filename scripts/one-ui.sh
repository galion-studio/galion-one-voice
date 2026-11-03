#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
# One-UI note. The phone page does not need a Python virtualenv.
#   scripts/one-ui.sh           # print the note and exit 0
#   scripts/one-ui.sh --venv    # optional empty venv; the page and CI do not use it
set -euo pipefail
cd "$(dirname "$0")/.."
cat <<'EOF'
Galion Voice one-UI needs no Python virtualenv.
The mark is raw HTML: crates/galion-console/assets/v1-screen.html
The integrator is Rust: galion-render, Luti, DisplayHz::Hz240.
Run: scripts/run-v1-screen.sh
Doc: docs/galion-voice-v2/ONE-UI.md
EOF
if [[ "${1:-}" == "--venv" ]]; then
  dest="${GALION_ONE_UI_VENV:-/tmp/galion-one-ui-venv}"
  python3 -m venv "$dest"
  echo "optional empty venv: $dest"
  echo "nothing installed; the orb does not use it"
fi
