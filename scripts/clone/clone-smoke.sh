#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
#
# Gate tests always. Real Nano generate only when GALION_CLONE_SMOKE=1
# and the pinned weights plus an allowlisted reference are already on disk.
# Does not fetch, does not write a WAV, does not edit LOCKS.toml.
set -euo pipefail
cd "$(dirname "$0")/../.."
PY="${GALION_CLONE_PYTHON:-python3}"
"$PY" scripts/clone/test_worker_gate.py
if [ "${GALION_CLONE_SMOKE:-}" != 1 ]; then
  echo "clone-smoke: gate tests passed. Generate not run. Set GALION_CLONE_SMOKE=1 on a machine that already has the pinned weights and the allowlisted refs."
  exit 0
fi
if [ "${GALION_CLONE_LOCAL_REFS:-}" != 1 ]; then
  echo "clone-smoke: GALION_CLONE_SMOKE=1 also needs GALION_CLONE_LOCAL_REFS=1" >&2
  exit 2
fi
export GALION_CLONE_MODEL="${GALION_CLONE_MODEL:-nano}"
smoke_one() {
  local persona="$1"
  local text="$2"
  local err out bytes
  err="$(mktemp)"
  out="$(mktemp)"
  line="$("$PY" -c 'import json,sys; print(json.dumps({"text":sys.argv[1],"model":"nano","persona":sys.argv[2],"tags":True}))' "$text" "$persona")"
  printf '%s\n' "$line" | "$PY" scripts/clone/chatterbox_worker.py >"$out" 2>"$err" || {
      echo "clone-smoke: $persona worker failed" >&2
      cat "$err" >&2
      rm -f "$err" "$out"
      exit 1
    }
  bytes=$(wc -c < "$out" | tr -d ' ')
  if [ "$bytes" -lt 2 ] || [ $((bytes % 2)) -ne 0 ]; then
    echo "clone-smoke: $persona PCM length $bytes is not even s16le" >&2
    cat "$err" >&2
    rm -f "$err" "$out"
    exit 1
  fi
  if grep -q 'galion-clone' "$out"; then
    echo "clone-smoke: $persona stdout contains framing text" >&2
    rm -f "$err" "$out"
    exit 1
  fi
  if ! grep -q 'galion-clone-engine: chatterbox-nano' "$err"; then
    echo "clone-smoke: $persona did not report chatterbox-nano" >&2
    cat "$err" >&2
    rm -f "$err" "$out"
    exit 1
  fi
  if ! grep -q "galion-clone-utt: $bytes" "$err"; then
    echo "clone-smoke: $persona marker does not match $bytes stdout bytes" >&2
    cat "$err" >&2
    rm -f "$err" "$out"
    exit 1
  fi
  echo "clone-smoke: $persona bytes=$bytes engine=chatterbox-nano"
  rm -f "$err" "$out"
}
smoke_one samantha "Hi. I'm here, and I'm listening."
smoke_one jarvis "Good morning. All systems are online."
echo "clone-smoke: generate passed for samantha and jarvis. No WAV written."
