#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
# V1 screen + phone path: one orb on near-black at http://127.0.0.1:7878/.
# The page posts /mic and /turn; the turn worker (off the 80 ms thread) answers on /reply.
# LOCAL ONLY (galion-render refuses public binds). Public HTTPS is ngrok (or Caddy later).
#
#   scripts/run-v1-screen.sh                  # serve for 1 h (Ctrl-C to stop)
#   scripts/run-v1-screen.sh --open           # + open it once, fullscreen kiosk (first browser found)
#   scripts/run-v1-screen.sh --check          # ~15 s self-test: page, /dot modes, /mic, /turn
#   ngrok http 127.0.0.1:7878                 # phone HTTPS; see docs/agent-v1-demo/EDGE-NGROK.md
#
# Env: V1_SCREEN_ADDR=127.0.0.1:7878 · V1_SCREEN_SECONDS=3600 · OUT=target/v1-screen
#      GALION_ASR=interim (default here: labelled placeholder, not recognition)
#      GALION_LLM=canned · GALION_SPEECH=auto · RELEASE=1
set -euo pipefail
cd "$(dirname "$0")/.."

ADDR="${V1_SCREEN_ADDR:-127.0.0.1:7878}"; SECS="${V1_SCREEN_SECONDS:-3600}"; OUT="${OUT:-target/v1-screen}"
OPEN=0; CHECK=0; ARGS=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --open) OPEN=1 ;;
    --check) CHECK=1; SECS=16 ;;
    --seconds) SECS="$2"; shift ;;
    *) ARGS+=("$1") ;;
  esac
  shift
done
URL="http://$ADDR/"
PROFILE=(); [[ "${RELEASE:-0}" == "1" ]] && PROFILE=(--release)
# Library default stays stub (real speech → AsrUnratified). The screen demo opts into the
# labelled placeholder so a phone turn can reach Piper. Override by exporting GALION_ASR.
export GALION_ASR="${GALION_ASR:-interim}"
export GALION_LLM="${GALION_LLM:-canned}"

mkdir -p "$OUT"
cargo build -q "${PROFILE[@]}" --offline -p galion-console --features dot-viz-web --example dot_console_demo
BIN="target/$([[ ${#PROFILE[@]} -gt 0 ]] && echo release || echo debug)/examples/dot_console_demo"

"$BIN" --headless --realtime --no-png --web "$ADDR" --seconds "$SECS" ${ARGS[@]+"${ARGS[@]}"} > "$OUT/heartbeat.log" 2>&1 &
PID=$!
trap 'kill $PID 2>/dev/null || true' EXIT INT TERM

for _ in $(seq 1 50); do curl -fsS "${URL}health" >/dev/null 2>&1 && break; sleep 0.1; done
curl -fsS "${URL}health" >/dev/null 2>&1 || { echo "V1 screen failed to start (port in use?) — $OUT/heartbeat.log:"; cat "$OUT/heartbeat.log"; exit 1; }
echo "Galion Voice · one dot → $URL"

if [[ "$CHECK" == "1" ]]; then
  sleep 2
  page=$(curl -fsS "$URL")
  grep -q '<canvas id="s"></canvas>' <<<"$page" && echo "  / : one canvas page OK ($(wc -c <<<"$page") bytes)" || { echo "  / : FAIL"; exit 1; }
  seen=""
  for _ in $(seq 1 150); do seen+=" $(curl -fsS "${URL}dot" 2>/dev/null | sed -n 's/.*"mode":"\([a-z]*\)".*/\1/p')"; sleep 0.07; done
  echo "  /dot modes over ~12 s:$(tr ' ' '\n' <<<"$seen" | sort -u | tr '\n' ' ')"
  code=$(curl -s -o /dev/null -w '%{http_code}' -X POST -H 'Content-Type: application/json' --data '{"rms":0.02,"t":1}' "${URL}mic")
  [[ "$code" == "204" ]] && echo "  POST /mic 204 OK" || { echo "  POST /mic FAIL ($code)"; exit 1; }
  code=$(curl -s -o /dev/null -w '%{http_code}' -X OPTIONS -H 'Origin: http://127.0.0.1:7878' -H 'Access-Control-Request-Method: POST' "${URL}turn")
  [[ "$code" == "204" ]] && echo "  OPTIONS /turn 204 OK" || { echo "  OPTIONS /turn FAIL ($code)"; exit 1; }
  # 100 ms of silence, 16 kHz mono PCM16. Acceptance is 202 (inbox), not a spoken reply.
  python3 - <<'PY' >"$OUT/silence.wav"
import struct,sys
n=1600
sys.stdout.buffer.write(b"RIFF"+struct.pack("<I",36+n*2)+b"WAVEfmt "+struct.pack("<IHHIIHH",16,1,1,16000,32000,2,16)+b"data"+struct.pack("<I",n*2)+b"\x00"*(n*2))
PY
  code=$(curl -s -o /dev/null -w '%{http_code}' -X POST -H 'Content-Type: audio/wav' -H 'X-Sample-Rate: 16000' --data-binary @"$OUT/silence.wav" "${URL}turn")
  [[ "$code" == "202" ]] && echo "  POST /turn 202 OK" || { echo "  POST /turn FAIL ($code)"; exit 1; }
  grep -q 'X-Content-Type-Options' <<<"$(curl -sI "${URL}health")" && echo "  security headers OK" || { echo "  security headers FAIL"; exit 1; }
  exit 0
fi

if [[ "$OPEN" == "1" ]]; then   # exactly one window: first browser found wins
  for b in chromium chromium-browser google-chrome google-chrome-stable; do
    command -v "$b" >/dev/null && { "$b" --kiosk --app="$URL" --noerrdialogs --disable-infobars --no-first-run >/dev/null 2>&1 & OPEN=0; break; }
  done
  [[ "$OPEN" == "1" ]] && command -v firefox >/dev/null && { firefox --kiosk "$URL" >/dev/null 2>&1 & }
fi
wait $PID || true
