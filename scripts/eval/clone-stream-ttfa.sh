#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
#
# Stream TTFA gate.
#   GALION_SPEECH=a21 scripts/eval/clone-stream-ttfa.sh
#
# The live pass is real Piper PCM (engine=piper, product_claim true,
# piper_measured true). A protocol-stub clock is FAIL and is not a pass.
# Nano streaming stays in scripts/clone/nano_stream.py. This script does
# not download weights and does not write a WAV.
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
  echo "clone stream hook returned ${stub_code}, want 2 when the stub clock is rejected" >&2
  exit 1
fi
printf '%s\n' "$stub_out" | grep -q '"gate": "FAIL"'
printf '%s\n' "$stub_out" | grep -q '"hard_pass": false'
printf '%s\n' "$stub_out" | grep -q '"product_claim": false'
printf '%s\n' "$stub_out" | grep -q '"stub_clock": "FAIL"'

if [ "${GALION_SPEECH:-}" = "a21" ]; then
  exec python3 scripts/eval/piper_stream_ttfa.py
fi

echo "FAIL"
echo '{"engine":null,"gate":"FAIL","hard_pass":false,"model_measured":false,"note":"GALION_SPEECH is not a21. Protocol-stub clocks stay FAIL.","piper_measured":false,"product_claim":false,"stub":false}'
exit 2
