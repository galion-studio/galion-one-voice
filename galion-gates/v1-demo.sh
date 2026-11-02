#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License. Copyright (c) 2026 Maciej Grajczyk.
# V1 — V1 demo smoke (V1-04, soft gate). Runs scripts/run-v1-demo.sh --headless into a temp dir.
#   rc 0 = PASS, rc 3 = SKIP (script absent), other = SOFTFAIL. No display, no network, no weights.
#   Speech-as-tone (C7) is reported but only fails here with V1_STRICT=1.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
S="$ROOT/scripts/run-v1-demo.sh"
[ -x "$S" ] || { echo "V1 SKIP — scripts/run-v1-demo.sh not present yet"; exit 3; }
T="${GALION_V1_TIMEOUT:-300}"
OUT="$(mktemp -d)"; trap 'rm -rf "$OUT"' EXIT
env -u DISPLAY -u WAYLAND_DISPLAY GALION_HEADLESS=1 V1_OUT="$OUT/v1" timeout "$T" "$S" --headless --seconds 2 >"$OUT/run.txt" 2>&1; rc=$?
grep -E '^(C[0-9]+ |== V1 RESULT)' "$OUT/run.txt"
SPEECH="$(grep "^== V1 RESULT" "$OUT/run.txt" | grep -o "speech=[a-z]*" | tail -n1)"
case $rc in
  0)   echo "V1 PASS — run-v1-demo.sh --headless rc=0 ($SPEECH)"; exit 0;;
  3)   echo "V1 SKIP — run-v1-demo.sh had nothing to check"; exit 3;;
  124) echo "V1 FAIL — run-v1-demo.sh timed out after ${T}s"; exit 1;;
  *)   echo "V1 FAIL — run-v1-demo.sh rc=$rc ($SPEECH; see gate log)"; exit 1;;
esac
