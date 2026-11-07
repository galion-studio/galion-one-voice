#!/usr/bin/env bash
set -euo pipefail
DIR=$(cd "$(dirname "$0")" && pwd)
# Poll up to ~90 min for Agent 1/2 artifacts, then run rescore.py
IFACE=/workspace/swarm/voice-v2-sprint/cloning-v2/scorers/persona/INTERFACE.md
DET=/workspace/swarm/voice-v2-sprint/cloning-v2/scorers/persona/detectors
DEADLINE=$((SECONDS + 5400))
while (( SECONDS < DEADLINE )); do
  if [[ -f "$IFACE" ]] && compgen -G "$DET/love_seductive*.py" >/dev/null; then
    echo "[$(date -Iseconds)] persona artifacts ready"
    break
  fi
  echo "[$(date -Iseconds)] waiting for INTERFACE.md + love_seductive*.py"
  sleep 60
done
if [[ ! -f "$IFACE" ]] || ! compgen -G "$DET/love_seductive*.py" >/dev/null; then
  echo "TIMEOUT waiting for persona scorer" >&2
  python3 "$DIR/rescore.py" || true
  exit 3
fi
exec python3 "$DIR/rescore.py"
