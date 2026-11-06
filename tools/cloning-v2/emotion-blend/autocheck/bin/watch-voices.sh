#!/usr/bin/env bash
# Optional voice-dir watcher (polling). Does NOT install a systemd unit.
# Watches ~/.cache/huggingface/cbx/voices for new own-*.cbxvoice files and
# runs hook-on-voice-add.sh. Skips files already reported. Ctrl-C to stop.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
HOOK="$ROOT/bin/hook-on-voice-add.sh"
VOICES="${CBX_VOICES_DIR:-$HOME/.cache/huggingface/cbx/voices}"
STATE="$ROOT/work/watch-seen.txt"
INTERVAL="${VOICECHECK_WATCH_INTERVAL:-5}"
mkdir -p "$(dirname "$STATE")"
touch "$STATE"
echo "watch-voices: polling $VOICES every ${INTERVAL}s (no systemd). state=$STATE"
echo "watch-voices: only triggers on NEW own-*-v2-*.cbxvoice (not own-samantha/own-jarvis production)."
while true; do
  shopt -s nullglob
  for f in "$VOICES"/own-*.cbxvoice; do
    base="$(basename "$f" .cbxvoice)"
    case "$base" in
      own-samantha|own-jarvis) continue;;  # never auto-mutate production via watcher
      *-v2-check-*|*-v2-*) ;;
      *) continue;;
    esac
    if grep -qxF "$base" "$STATE" 2>/dev/null; then
      continue
    fi
    echo "watch-voices: new voice $base — running hook"
    # Infer persona + base wav when possible
    persona="${base#own-}"; persona="${persona%%-*}"
    base_wav="/workspace/swarm/voice-v2-sprint/clip-store-own/${persona}/original.wav"
    if [[ -f "$base_wav" ]]; then
      nice -n 10 "$HOOK" --voice "$base" --base "$base_wav" --skip-fixes || \
        echo "watch-voices: hook failed for $base (recorded as seen to avoid loop)"
    else
      echo "watch-voices: no base wav for $persona; skipping auto-check"
    fi
    echo "$base" >> "$STATE"
  done
  sleep "$INTERVAL"
done
