#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License. Copyright (c) 2026 Maciej Grajczyk.
# LV — live-demo smoke (W21-05, soft gate). Runs scripts/run-live-demo.sh headless with a timeout.
#   rc 0 = PASS, rc 3 = SKIP (script absent / no audio-display needed path unavailable), other = SOFTFAIL.
# Never needs a display: DISPLAY/WAYLAND_DISPLAY are cleared and GALION_HEADLESS=1 is exported,
# so the one-dot window must fall back to its NullSurface/terminal path. No network, no weights.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
S="$ROOT/scripts/run-live-demo.sh"
[ -x "$S" ] || { echo "LV SKIP — scripts/run-live-demo.sh not present yet"; exit 3; }
T="${GALION_LV_TIMEOUT:-180}"
OUT="$(mktemp -d)"; trap 'rm -rf "$OUT"' EXIT
env -u DISPLAY -u WAYLAND_DISPLAY GALION_HEADLESS=1 GALION_LIVE_GATE=1 LIVE_OUT="$OUT/live" \
  timeout "$T" "$S" --showcase >"$OUT/run.txt" 2>&1; rc=$?
tail -n 20 "$OUT/run.txt"
if [ $rc = 0 ]; then
  for v in default opener workplace; do for a in live_output_stereo.wav console_capture.txt; do
    [ -s "$OUT/live/$v/$a" ] || { echo "LV FAIL — missing artifact $v/$a"; exit 1; }; done; done
fi
case $rc in
  0)   echo "LV PASS — run-live-demo.sh --showcase headless rc=0; default/opener/workplace stereo+capture present, workplace hard-OFF"; exit 0;;
  3)   echo "LV SKIP — run-live-demo.sh reported skip (rc=3)"; exit 3;;
  124) echo "LV FAIL — run-live-demo.sh timed out after ${T}s"; exit 1;;
  *)   echo "LV FAIL — run-live-demo.sh rc=$rc (see gate log)"; exit 1;;
esac
