#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
#
# V1-02 · ONE command, ONE screen. Orchestrates the V1 demo build (V4 locks + Rika/D20 + Wave-21)
# behind the single one-dot screen. LOCAL ONLY, offline, no weights, no extra windows.
# Skeleton — V1-04 hardens it (gates, timeouts, exit codes). Additive hooks for V1-01 / V1-03 below.
#
#   [0] scripts/check-locks.sh                      V4 Eureka locks (80 ms, one-dot, consent deny)
#   [1] speech turn  -> WAV                         EN cascade (spoken words: A21 Piper CPU, else flite)
#   [2] ONE galion-live-demo process (dot-viz-web), FP-02:   80 ms Heartbeat::step + Rika per frame
#        --reply <turn WAV> --loops N --mimi off --realtime --rika-opener --web 127.0.0.1:$FEED_PORT
#        you band = synthetic room/breath/voice/barge-in (Rika lean-in, listening, yield)
#        me band  = the cascade WORDS (A21 Piper CPU; flite only if A21 missing) -> dot "speaking" while they play
#        serves / (V1-01 assets/v1-screen.html) + /dot + /health; no PNGs, no /frame, no re-exec per turn
#   UI ENTRY (single, V1-01): http://127.0.0.1:$FEED_PORT/  (= scripts/run-v1-screen.sh URL, default :7878)
#        page falls back to its local idle pulse if the feed is gone >1 s (never dark between turns)
#   (FP-01: the :7870 screen-host mirror was deleted — one port, one page.)
#   fallback: live BUILD fails -> scripts/run-v1-screen.sh (V1-01 page, synthetic pump, same port)
#
# Usage: scripts/run-v1-demo.sh [--loops N(0=forever)] [--text "hello galion"] [--check]
#                               [--consent sense,adapt] [--purpose personal] [--no-opener]
# Env:   FEED_PORT=7878 V1_OUT=<repo>/galion-demo/out/v1
# --check: one loop (~16 s), verify /health + /dot + / on the feed port, exit 0/1 (timeouts: V1-04).
# --headless (V1-04): no servers/ports. locks + speech turn + live --showcase (Rika, workplace hard-OFF)
#        + EN turn + one-dot PNG frames, then scripts/v1/soft-checks.sh -> <V1_OUT>/V1-SUMMARY.{txt,json}.
#        Used by galion-gates/v1-demo.sh. V1_STRICT=1 makes soft fails (e.g. tone-not-words) exit 1.
# Exit codes (V1-04): 0 ok · 1 check/soft-check FAIL · 64 bad arg · 69 port busy · 124 check timed out
# Last stdout line: GALION_V1_SCREEN=http://127.0.0.1:$FEED_PORT/
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT="$(pwd)"
FEED_PORT="${FEED_PORT:-7878}"
UI_URL="http://127.0.0.1:$FEED_PORT/"   # the one UI entry: one dot, one port
UI_BASE="http://127.0.0.1:$FEED_PORT"
OUT="${V1_OUT:-$ROOT/galion-demo/out/v1}"
HEADLESS=0; SCREEN_SECS=8; V1_TIMEOUT="${V1_TIMEOUT:-120}"
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

PIDS=()
cleanup() { for p in "${PIDS[@]:-}"; do [ -n "$p" ] && kill "$p" 2>/dev/null || true; done; }
trap cleanup EXIT INT TERM
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
  || { echo "run-v1-demo: no turn WAV, live loop uses synthetic you band"; TURN_WAV=""; SPEECH_KIND="synthetic"; }

# ---- V1-04 headless path: no ports, artifacts + soft checks (gate / CI / no-display box) ----
if [ "$HEADLESS" = 1 ]; then
  echo "== [2h] live showcase (heartbeat + Rika, one-dot frames) -> $OUT/live"
  rc=0
  env -u DISPLAY -u WAYLAND_DISPLAY GALION_HEADLESS=1 LIVE_DOT=1 LIVE_OUT="$OUT/live" \
    timeout "$V1_TIMEOUT" scripts/run-live-demo.sh --showcase --quiet > "$OUT/live.log" 2>&1 || { rc=$?; echo "run-v1-demo: live showcase rc=$rc (see $OUT/live.log)"; }
  if [ "$rc" != 0 ]; then   # V1-04: one retry — a concurrent cargo build can swap target/release/galion-live-demo mid-run
    echo "run-v1-demo: retrying live showcase once"; sleep 2; rc=0
    env -u DISPLAY -u WAYLAND_DISPLAY GALION_HEADLESS=1 LIVE_DOT=1 LIVE_OUT="$OUT/live" \
      timeout "$V1_TIMEOUT" scripts/run-live-demo.sh --showcase --quiet >> "$OUT/live.log" 2>&1 || { rc=$?; echo "run-v1-demo: live showcase retry rc=$rc"; }
  fi
  if [ -n "$TURN_WAV" ] && [ -x "$ROOT/target/release/galion-live-demo" ]; then
    echo "== [2h] EN turn: --reply $(basename "$TURN_WAV") (FP-04: words in the me band, same as the live path)"
    timeout "$V1_TIMEOUT" "$ROOT/target/release/galion-live-demo" --out "$OUT/live/en-turn" --reply "$TURN_WAV" --loops 1 --mimi off \
      --consent "$CONSENT" --purpose "$PURPOSE" ${OPENER[@]+"${OPENER[@]}"} --quiet >> "$OUT/live.log" 2>&1 || echo "run-v1-demo: EN turn failed"
  fi
  echo "== [3h] one-dot screen frames (headless) -> $OUT/screen"
  env -u DISPLAY -u WAYLAND_DISPLAY GALION_HEADLESS=1 OUT="$OUT/screen" \
    timeout "$V1_TIMEOUT" scripts/run-dot-console-demo.sh --headless --seconds "$SCREEN_SECS" > "$OUT/screen.log" 2>&1 || echo "run-v1-demo: screen frames failed (see $OUT/screen.log)"
  state "-1" "headless-done" "$OUT/live/opener/live_output_stereo.wav" "$SPEECH_KIND"
  echo "== [4] V1-04 soft checks"
  SC=0; V1_SPEECH_KIND="$SPEECH_KIND" scripts/v1/soft-checks.sh "$OUT" || SC=$?
  echo "GALION_V1_DEMO_OUT=$OUT"
  exit $SC
fi

# V1-04: fail fast with a clear message if a port is taken (stale demo still running)
if (exec 3<>"/dev/tcp/127.0.0.1/$FEED_PORT") 2>/dev/null; then echo "run-v1-demo: port $FEED_PORT busy (old demo running? pkill -f galion-live-demo; pkill -f dot_console_demo)"; exit 69; fi

SPEECH_WAV="${TURN_WAV:-}"   # reply words -> me band (recorded in state.json for soft checks)

echo "== [2] build live pipeline (dot-viz-web)"
LIVE_OK=1
cargo build --offline --release -q -p galion-live-demo --features dot-viz-web || LIVE_OK=0
# FP-02: run a private copy so a concurrent `cargo build` cannot swap the binary mid-demo.
BIN="$OUT/bin/galion-live-demo"
if [ "$LIVE_OK" = 1 ]; then mkdir -p "$OUT/bin" && cp -f "$ROOT/target/release/galion-live-demo" "$BIN.tmp" && mv -f "$BIN.tmp" "$BIN" || LIVE_OK=0; fi

echo "== [3] single screen (one dot) on $UI_URL"
state 0 "starting" "" "$SPEECH_KIND"

live() {  # $1 = loops (0 = forever), $2 = hold secs. ONE process, ONE bind, monotonic dot clock.
  local REPLY=(); [ -n "$SPEECH_WAV" ] && REPLY=(--reply "$SPEECH_WAV")
  state 1 "live" "${SPEECH_WAV:-$OUT/live-web/live_output_stereo.wav}" "$SPEECH_KIND"
  "$BIN" --out "$OUT/live-web" --consent "$CONSENT" --purpose "$PURPOSE" ${OPENER[@]+"${OPENER[@]}"} \
    --realtime --quiet --mimi off --web "127.0.0.1:$FEED_PORT" --loops "$1" --hold "$2" ${REPLY[@]+"${REPLY[@]}"}
}

if [ "$LIVE_OK" = 1 ]; then
  if [ "$CHECK" = 1 ]; then
    live 1 2 > "$OUT/live-web.log" 2>&1 & TP=$!; PIDS+=($TP)
    ok=1; sleep 2
    C=(curl -fsS --max-time 3 --retry 3 --retry-delay 1 --retry-connrefused)   # V1-04: bounded
    "${C[@]}" "$UI_BASE/health" >/dev/null || { echo "check: /health failed"; ok=0; }
    "${C[@]}" "$UI_BASE/dot" | grep -q '"state"' || { echo "check: /dot has no state"; ok=0; }
    "${C[@]}" "$UI_BASE/" | grep -qi '<html' || { echo "check: / is not html"; ok=0; }
    W=0; while kill -0 "$TP" 2>/dev/null; do sleep 1; W=$((W+1)); [ "$W" -ge "$V1_TIMEOUT" ] && { kill "$TP"; echo "run-v1-demo: check timed out"; exit 124; }; done
    rc=0; wait "$TP" || rc=$?
    [ "$rc" = 69 ] && { cat "$OUT/live-web.log"; exit 69; }
    [ "$rc" = 0 ] || ok=0
    sed -n 's/^galion-live-demo: OK /live: /p' "$OUT/live-web.log"
    [ "$ok" = 1 ] && echo "run-v1-demo: PASS check (screen + live /dot fed by Heartbeat+Rika)" || { echo "run-v1-demo: FAIL check (see $OUT/live-web.log)"; exit 1; }
  else
    echo "run-v1-demo: open $UI_URL (V1-01 single screen: one dot; F = fullscreen). Ctrl-C to stop."
    rc=0; live "$LOOPS" 0 || rc=$?   # no restart loop: a crash ends the demo with its exit code
    [ "$rc" = 0 ] || { echo "run-v1-demo: live loop exited rc=$rc"; exit "$rc"; }
  fi
else
  # V1-01's own screen (same page, synthetic 80 ms pump) — still one screen, no second window.
  echo "run-v1-demo: live build failed -> fallback scripts/run-v1-screen.sh (V1-01, synthetic pump) on :$FEED_PORT"
  state 0 "fallback-synthetic" "" "$SPEECH_KIND"
  if [ "$CHECK" = 1 ]; then   # FP-04: the page alone is not the demo — a synthetic-pump pass must not read as PASS
    V1_SCREEN_ADDR="127.0.0.1:$FEED_PORT" scripts/run-v1-screen.sh --check || true
    echo "run-v1-demo: FAIL check (live build failed; only the synthetic fallback screen ran — Heartbeat+Rika+words NOT verified)"; exit 1
  else V1_SCREEN_ADDR="127.0.0.1:$FEED_PORT" V1_SCREEN_SECONDS="${FALLBACK_SECONDS:-3600}" scripts/run-v1-screen.sh; fi
fi
state "-1" "done" "" "$SPEECH_KIND"
echo "GALION_V1_SCREEN=$UI_URL"
