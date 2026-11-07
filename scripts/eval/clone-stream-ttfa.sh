#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
#
# Stream TTFA gate. Piper is the hard live path (t301).
# Nano streaming stays in scripts/clone/nano_stream.py.
#
#   scripts/eval/clone-stream-ttfa.sh
#
# A protocol-stub clock, including a 15 ms first-PCM figure, is FAIL.
# product_claim stays false for that clock. The live pass is
# scripts/eval/piper-stream-ttfa.sh when the A21 pack is on disk.
# No pack and no model engine: one JSON line, measured false, gate BLOCKED.
set -euo pipefail
cd "$(dirname "$0")/../.."

set +e
stub_out=$(python3 scripts/eval/clone_stream_ttfa.py)
stub_code=$?
set -e
printf '%s\n' "$stub_out" >&2
if [ "$stub_code" = 1 ]; then
  printf '%s\n' "$stub_out"
  exit 1
fi
if [ "$stub_code" != 2 ]; then
  echo "FAIL" >&2
  echo "stub clock was not rejected (exit ${stub_code})" >&2
  exit 1
fi
printf '%s\n' "$stub_out" | grep -q '"gate": "FAIL"'
printf '%s\n' "$stub_out" | grep -q '"hard_pass": false'
printf '%s\n' "$stub_out" | grep -q '"product_claim": false'
printf '%s\n' "$stub_out" | grep -q '"stub_clock": "FAIL"'
# A 15 ms stub must not be reported as ttfa_ms.
if printf '%s\n' "$stub_out" | grep -q '"ttfa_ms"'; then
  echo "FAIL: stub report carried ttfa_ms" >&2
  exit 1
fi

if [ -x weights/runtime/piper/piper ] && [ -f weights/tts/en_US-ljspeech-medium.onnx ]; then
  out=$(scripts/eval/piper-stream-ttfa.sh)
  printf '%s\n' "$out"
  printf '%s\n' "$out" | grep -q '"engine": "piper"'
  printf '%s\n' "$out" | grep -q '"stub": false'
  printf '%s\n' "$out" | grep -q '"product_claim": true'
  printf '%s\n' "$out" | grep -q '"measured": true'
  exit 0
fi

echo '{"engine":null,"gate":"BLOCKED","hard_pass":false,"measured":false,"model_measured":false,"note":"Piper pack absent. No model engine ran. product_claim is false.","piper_measured":false,"product_claim":false,"stub":false}'
exit 2
