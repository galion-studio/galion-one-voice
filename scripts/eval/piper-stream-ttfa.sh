#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
#
# t301 · live Piper CPU stream TTFA. One JSON object on stdout.
#
#   scripts/eval/piper-stream-ttfa.sh                 # SpeechStack + raw spawn + drain
#   scripts/eval/piper-stream-ttfa.sh --wav /tmp/x.wav  # also write drained 24 kHz audio
#
# Forces GALION_SPEECH=a21 and GALION_A21_RUNTIME (default: repo piper).
# Exit 0 only when warm_first_pcm_ms was measured. Exit 2 when the A21 pack
# or the runtime is missing (measured:false, no ms fields). Never invents ms.
set -euo pipefail
cd "$(dirname "$0")/../.."
export GALION_SPEECH=a21
export GALION_A21_RUNTIME="${GALION_A21_RUNTIME:-$PWD/weights/runtime/piper/piper}"
out=$(python3 scripts/eval/piper_stream_ttfa.py "$@") || { rc=$?; printf '%s\n' "${out:-}"; exit "$rc"; }
printf '%s\n' "$out"
printf '%s\n' "$out" | grep -q '"stub": false'
printf '%s\n' "$out" | grep -q '"warm_first_pcm_ms": [0-9]'
