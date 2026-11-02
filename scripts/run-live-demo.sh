#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
#
# W21-01 · one-command LIVE demo on the real 80 ms path. LOCAL ONLY, offline, no weights.
#   you WAV (or synthetic) -> 80 ms frames @ 24 kHz -> Heartbeat::step + RikaReplayBackend
#   -> me band (tone voice -> W21-02 Mimi lane, synthetic IS_STUB, DryBypass)
#   -> stereo WAV (L=you, R=me) + console_capture.txt + SHA256SUMS
#
# Usage: scripts/run-live-demo.sh [--in you.wav] [--out DIR] [--consent none|sense,adapt]
#                                 [--purpose personal|workplace|education] [--rika-opener]
#                                 [--realtime] [--mimi off|bypass|codec|nokernels] [--quiet]
#        scripts/run-live-demo.sh --showcase    # 3 variants: default / opener / workplace hard-OFF
# Env:   LIVE_OUT       default out dir (default: <repo>/galion-demo/out/live)
#        LIVE_REALTIME=1  add --realtime (sleep to the wall clock, 80 ms per frame)
#        LIVE_DOT=1       build with --features dot-viz and add --dot (W21-04 PNG dot frames in <out>/dot/)
#        LIVE_EN_CASCADE=1 feed the W21-03 EN cascade WAV (scripts/run-en-cascade-demo.sh) as --in
# No --in → synthetic you band (room tone / breath / tone "speech", seed 13).
# Last stdout line: GALION_LIVE_DEMO_OUT=<absolute dir>
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT="$(pwd)"
OUT="${LIVE_OUT:-$ROOT/galion-demo/out/live}"
ARGS=(); SHOWCASE=0; HAVE_OUT=0; HAVE_IN=0
while [ $# -gt 0 ]; do
  case "$1" in
    --showcase) SHOWCASE=1; shift ;;
    --out) OUT="$2"; HAVE_OUT=1; shift 2 ;;
    --in) HAVE_IN=1; ARGS+=("$1" "$2"); shift 2 ;;
    -h|--help) sed -n '2,19p' "$0"; exit 0 ;;
    *) ARGS+=("$1"); shift ;;
  esac
done
[ "${LIVE_REALTIME:-0}" = 1 ] && ARGS+=(--realtime)
FEAT=()
if [ "${LIVE_DOT:-0}" = 1 ]; then FEAT=(--features dot-viz); ARGS+=(--dot); fi
if [ "${LIVE_EN_CASCADE:-0}" = 1 ] && [ "$HAVE_IN" = 0 ]; then
  EN_WAV=$(scripts/run-en-cascade-demo.sh | sed -n 's/^GALION_EN_CASCADE_WAV=//p' || true)
  if [ -n "${EN_WAV:-}" ] && [ -f "$EN_WAV" ]; then ARGS+=(--in "$EN_WAV"); echo "live-demo: you band = EN cascade WAV $EN_WAV"
  else echo "live-demo: EN cascade unavailable, using synthetic input"; fi
fi
echo "live-demo: cargo build --offline --release -p galion-live-demo ${FEAT[*]:-}"
cargo build --offline --release -p galion-live-demo -q "${FEAT[@]}"
BIN="$ROOT/target/release/galion-live-demo"
mkdir -p "$OUT"
if [ "$SHOWCASE" = 1 ]; then
  "$BIN" --out "$OUT/default"   --quiet "${ARGS[@]}"
  "$BIN" --out "$OUT/opener"    --consent sense,adapt --purpose personal  --rika-opener "${ARGS[@]}"
  "$BIN" --out "$OUT/workplace" --consent sense,adapt --purpose workplace --rika-opener --quiet "${ARGS[@]}"
  cmp -s "$OUT/default/live_output_stereo.wav" "$OUT/workplace/live_output_stereo.wav" \
    || { echo "live-demo: FAIL workplace is not hard-OFF (me band differs from default)"; exit 1; }
  grep -q 'OFF(Gate(DenyProhibitedPurpose))' "$OUT/workplace/console_capture.txt" \
    || { echo "live-demo: FAIL workplace capture lacks hard-OFF"; exit 1; }
  grep -q 'lean_in_latency=160 ms' "$OUT/opener/console_capture.txt" \
    || echo "live-demo: note: opener lean-in latency != 160 ms for this input"
  echo "live-demo: PASS showcase (default / opener / workplace hard-OFF)"
else
  "$BIN" --out "$OUT" "${ARGS[@]}"
fi
echo "GALION_LIVE_DEMO_OUT=$OUT"
