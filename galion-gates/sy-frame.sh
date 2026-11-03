#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
# SY — symphony frame bar (soft gate). 80 ms tick, 1% stub-step ceiling.
#   rc 0 = PASS, other = SOFTFAIL unless GALION_GATES_HARD=1.
# Skip with GALION_GATES_NO_SY=1. Does not invent a latency number.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
S="$ROOT/scripts/symphony/check-g-latency.sh"
[ -f "$S" ] || { echo "SY FAIL — check-g-latency.sh missing"; exit 1; }
out="$("$S" 2>&1)"
rc=$?
printf '%s\n' "$out"
last="$(printf '%s\n' "$out" | tail -n 1)"
if [ "$rc" -eq 0 ]; then
  echo "SY PASS — $last"
  exit 0
fi
echo "SY FAIL — $last"
exit 1
