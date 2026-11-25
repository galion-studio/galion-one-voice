#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Galion Labs.
#
# V1-02 · ONE command, ONE screen. Orchestrates the V1 demo build (V4 locks + Rika/D20 + Wave-21)
# behind the single one-dot screen. LOCAL ONLY, offline, no weights, no extra windows.
# Skeleton — V1-04 hardens it (gates, timeouts, exit codes). Additive hooks for V1-01 / V1-03 below.
#
#   [0] scripts/check-locks.sh                      V4 Eureka locks (80 ms, one-dot, consent deny)
#   [1] speech turn  -> WAV                         EN cascade (spoken words: A21 Piper CPU, else flite)
#   [2] HORBM duplex. galion-live-demo is archived and is not built.
#        scripts/run-live-demo.sh runs `cargo test --manifest-path horbm/Cargo.toml -p horbm-duplex`.
#        The old --web feed and --reply me-band are archived with that crate.
#        One-dot page without a live feed: scripts/run-v1-screen.sh
#
# Usage: scripts/run-v1-demo.sh [--text "hello galion"] [--check] [--headless]
#                               [--consent sense,adapt] [--purpose personal] [--no-opener]
# Env:   V1_OUT=<repo>/galion-demo/out/v1
#        V1_DUPLEX_TIMEOUT seconds for the duplex step (default 600)
# --check: run the duplex tests. Exit 0 when they pass. The archived web feed is not started.
# --headless: no servers. locks + speech turn + duplex tests + one-dot PNG frames,
#        then scripts/v1/soft-checks.sh -> <V1_OUT>/V1-SUMMARY.{txt,json}.
#        Used by galion-gates/v1-demo.sh. V1_STRICT=1 makes soft fails (e.g. tone-not-words) exit 1.
# Exit codes: 0 ok · 1 check/soft-check FAIL · 64 bad arg · 124 duplex step timed out
# Last stdout line: GALION_V1_SCREEN=archived   or, headless, GALION_V1_DEMO_OUT=<dir>
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT="$(pwd)"
OUT="${V1_OUT:-$ROOT/galion-demo/out/v1}"
HEADLESS=0; SCREEN_SECS=8; V1_TIMEOUT="${V1_TIMEOUT:-120}"
DUPLEX_TIMEOUT="${V1_DUPLEX_TIMEOUT:-600}"
LOOPS=0; TEXT="hello galion"; CHECK=0; CONSENT="sense,adapt"; PURPOSE="personal"; OPENER=(--rika-opener)
while [ $# -gt 0 ]; do
  case "$1" in
    --loops) LOOPS="$2"; shift 2 ;;
    --text) TEXT="$2"; shift 2 ;;
    --check) CHECK=1; LOOPS=1; shift ;;
    --consent) CONSENT="$2"; shift 2 ;;
    --purpose) PURPOSE="$2"; shift 2 ;;
    --no-opener) OPENER=(); shift ;;
    --headless) HEADLESS=1; shift ;;
    --seconds) SCREEN_SECS="$2"; shift 2 ;;
    -h|--help) sed -n '2,/^set -euo/{/^#/p}' "$0"; exit 0 ;;
    *) echo "run-v1-demo: unknown arg: $1" >&2; exit 64 ;;
  esac
done
mkdir -p "$OUT"
STATE="$OUT/state.json"

state() { printf '{"turn":%s,"stage":"%s","audio":"%s","speech":"%s"}\n' "$1" "$2" "${3:-}" "${4:-}" > "$STATE.tmp" && mv "$STATE.tmp" "$STATE"; }

echo "== [0] V4 locks"
scripts/check-locks.sh

echo "== [1] speech turn"
SPEECH_KIND="en-cascade-tone"   # FP-03: cascade speaks via A21 (Piper, CPU) if PRESENT, else flite fallback
if [ -z "${TURN_WAV:-}" ] || [ ! -f "$TURN_WAV" ]; then
  TURN_WAV=$(GALION_DEMO_OUT="$OUT/cascade" scripts/run-en-cascade-demo.sh --text "$TEXT" | sed -n 's/^GALION_EN_CASCADE_WAV=//p' | tail -1 || true)
  # FP-03: cascade speaks words (A21 Piper CPU, else flite fallback); read the kind from its JSON.
  [ -f "$OUT/cascade/en-cascade.json" ] && SPEECH_KIND="en-cascade:$(python3 -c "import json,sys;j=json.load(open(sys.argv[1]));print(j.get('reply_kind','tone')+'/'+str(j.get('speech_backend_label','?')))" "$OUT/cascade/en-cascade.json" 2>/dev/null || echo tone)"
fi
# V1-04: label the turn from the cascade JSON (V1-03: reply_kind speech|earcon-fallback-NOT-SPEECH; exit 3 = no speech backend)
if [ "$SPEECH_KIND" = en-cascade-tone ] && [ -n "${TURN_WAV:-}" ] && [ -f "${TURN_WAV%.wav}.json" ]; then
  RK=$(sed -n 's/.*"reply_kind":"\([^"]*\)".*/\1/p' "${TURN_WAV%.wav}.json"); SB=$(sed -n 's/.*"speech_backend":"\([^"]*\)".*/\1/p' "${TURN_WAV%.wav}.json")
  case "$RK" in speech) SPEECH_KIND="en-cascade-words:${SB:-unknown}";; ?*) SPEECH_KIND="en-cascade-$RK";; esac
fi
[ -z "${TURN_WAV:-}" ] && echo "run-v1-demo: EN cascade gave no speech WAV (exit 3 = SPEECH=ABSENT: run scripts/a21/fetch-a21.sh for A21, or apt install libflite1)"
[ -n "${TURN_WAV:-}" ] && [ -f "$TURN_WAV" ] && echo "run-v1-demo: turn WAV = $TURN_WAV ($SPEECH_KIND)" \
  || { echo "run-v1-demo: no turn WAV. horbm-duplex does not read one."; TURN_WAV=""; SPEECH_KIND="synthetic"; }

echo "run-v1-demo: flags consent=$CONSENT purpose=$PURPOSE loops=$LOOPS opener=${OPENER[*]:-(none)} are not passed to horbm-duplex."

# Headless: no ports. Duplex tests + one-dot frames + soft checks.
if [ "$HEADLESS" = 1 ]; then
  echo "== [2h] horbm duplex (galion-live-demo archived)"
  rc=0
  env -u DISPLAY -u WAYLAND_DISPLAY GALION_HEADLESS=1 \
    timeout "$DUPLEX_TIMEOUT" scripts/run-live-demo.sh --showcase > "$OUT/live.log" 2>&1 || { rc=$?; echo "run-v1-demo: duplex rc=$rc (see $OUT/live.log)"; }
  if [ "$rc" != 0 ]; then
    echo "run-v1-demo: retrying duplex once"; sleep 2; rc=0
    env -u DISPLAY -u WAYLAND_DISPLAY GALION_HEADLESS=1 \
      timeout "$DUPLEX_TIMEOUT" scripts/run-live-demo.sh --showcase >> "$OUT/live.log" 2>&1 || { rc=$?; echo "run-v1-demo: duplex retry rc=$rc"; }
  fi
  echo "== [3h] one-dot screen frames (headless) -> $OUT/screen"
  env -u DISPLAY -u WAYLAND_DISPLAY GALION_HEADLESS=1 OUT="$OUT/screen" \
    timeout "$V1_TIMEOUT" scripts/run-dot-console-demo.sh --headless --seconds "$SCREEN_SECS" > "$OUT/screen.log" 2>&1 || echo "run-v1-demo: screen frames failed (see $OUT/screen.log)"
  state "-1" "headless-done" "" "$SPEECH_KIND"
  echo "== [4] soft checks"
  SC=0; V1_SPEECH_KIND="$SPEECH_KIND" scripts/v1/soft-checks.sh "$OUT" || SC=$?
  echo "GALION_V1_DEMO_OUT=$OUT"
  exit $SC
fi

echo "== [2] horbm duplex (galion-live-demo archived; web feed not started)"
echo "run-v1-demo: one-dot page without the live feed: scripts/run-v1-screen.sh"
rc=0
scripts/run-live-demo.sh --showcase || rc=$?
if [ "$rc" = 124 ]; then echo "run-v1-demo: duplex step timed out"; exit 124; fi
if [ "$CHECK" = 1 ]; then
  if [ "$rc" = 0 ]; then
    echo "run-v1-demo: PASS check (horbm-duplex). The archived web feed was not started."
  else
    echo "run-v1-demo: FAIL check (horbm-duplex rc=$rc)"; exit 1
  fi
elif [ "$rc" != 0 ]; then
  echo "run-v1-demo: duplex rc=$rc"; exit "$rc"
fi
state "-1" "done" "" "$SPEECH_KIND"
echo "GALION_V1_SCREEN=archived"
