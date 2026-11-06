#!/usr/bin/env bash
# Post-clone hook: run galion-voicecheck on a newly added voice.
# Usage (from clone scripts, after voice add):
#   hook-on-voice-add.sh --voice OWN_NAME --base /path/original.wav [--ref reference.wav]
# Skips protected production names for *modification*, but will CHECK them read-only.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="$ROOT/target/release/galion-voicecheck"
if [[ ! -x "$BIN" ]]; then
  BIN="$ROOT/target/debug/galion-voicecheck"
fi
if [[ ! -x "$BIN" ]]; then
  echo "hook-on-voice-add: galion-voicecheck not built; build with: cargo build --release -j 2" >&2
  exit 1
fi
VOICE=""
BASE=""
REF=""
EXTRA=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --voice) VOICE="$2"; shift 2;;
    --base) BASE="$2"; shift 2;;
    --ref) REF="$2"; shift 2;;
    *) EXTRA+=("$1"); shift;;
  esac
done
if [[ -z "$VOICE" ]]; then
  echo "usage: hook-on-voice-add.sh --voice NAME --base WAV [--ref WAV]" >&2
  exit 2
fi
# Never let the hook rewrite production voices as fix outputs (enforced inside CLI).
ARGS=(check --voice "$VOICE")
[[ -n "$BASE" ]] && ARGS+=(--base "$BASE")
[[ -n "$REF" ]] && ARGS+=(--ref "$REF")
ARGS+=("${EXTRA[@]}")
exec nice -n 10 "$BIN" "${ARGS[@]}"
