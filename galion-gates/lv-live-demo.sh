#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License. Copyright (c) 2026 Galion Labs.
# LV — live-loop smoke. Runs scripts/run-live-demo.sh, which runs the horbm-duplex
# tests. It does not build galion-live-demo and it does not check showcase WAVs.
#   rc 0 = PASS, rc 3 = SKIP (script absent), other = SOFTFAIL from run-gates.sh.
# No display. No weights. A cold horbm-duplex build can take longer than the old
# demo binary, so the default timeout is 600s (GALION_LV_TIMEOUT).
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
S="$ROOT/scripts/run-live-demo.sh"
[ -x "$S" ] || { echo "LV SKIP — scripts/run-live-demo.sh not present yet"; exit 3; }
T="${GALION_LV_TIMEOUT:-600}"
OUT="$(mktemp -d)"; trap 'rm -rf "$OUT"' EXIT
env -u DISPLAY -u WAYLAND_DISPLAY GALION_HEADLESS=1 \
  timeout "$T" "$S" --showcase >"$OUT/run.txt" 2>&1; rc=$?
tail -n 20 "$OUT/run.txt"
if [ $rc = 0 ]; then
  grep -q 'live-demo: PASS duplex horbm-duplex' "$OUT/run.txt" \
    || { echo "LV FAIL — duplex pass line missing"; exit 1; }
fi
case $rc in
  0)   echo "LV PASS — run-live-demo.sh ran horbm-duplex (galion-live-demo archived)"; exit 0;;
  3)   echo "LV SKIP — run-live-demo.sh reported skip (rc=3)"; exit 3;;
  124) echo "LV FAIL — run-live-demo.sh timed out after ${T}s"; exit 1;;
  *)   echo "LV FAIL — run-live-demo.sh rc=$rc (see gate log)"; exit 1;;
esac
