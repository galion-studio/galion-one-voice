#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
# V1 screen + phone path: one orb on near-black at http://127.0.0.1:7878/.
# The page posts /mic and /turn; the turn worker (off the 80 ms thread) answers on /reply.
# LOCAL ONLY (galion-render refuses public binds). Public HTTPS is ngrok (or Caddy later).
#
#   scripts/run-v1-screen.sh                  # serve for 1 h (Ctrl-C to stop)
#   scripts/run-v1-screen.sh --open           # + open it once, fullscreen kiosk (first browser found)
#   scripts/run-v1-screen.sh --check          # ~15 s self-test: page, /dot modes, /mic, /turn
#   ngrok http 127.0.0.1:7878                 # local only. Staging must add OAuth or basic auth
#                                         # and set GALION_CORS_ORIGINS. No secrets in this file.
#                                         # docs/galion-voice-v2/SECURITY-EDGE.md
#
# Env: V1_SCREEN_ADDR=127.0.0.1:7878 · V1_SCREEN_SECONDS=3600 · OUT=target/v1-screen
#      GALION_ASR=vosk when weights/asr/vosk-model-small-en-us-0.15 is on disk,
#      otherwise stub (typed text works; mic does not invent a transcript).
#      GALION_ASR=interim is refused here (test-only placeholder).
#      GALION_LLM=canned|echo|hermes · GALION_SPEECH=auto · RELEASE=1
#      Model: docs/asr/VOSK.md · scripts/asr/fetch-vosk.sh
#
# Hermes (approved gateway, OpenAI-compatible, numeric loopback, no TLS):
#   GALION_LLM_URL=http://127.0.0.1:8642/v1/chat/completions \
#   GALION_LLM_MODEL=hermes-agent \
#   GALION_LLM_KEY=... \
#   GALION_LLM_TIMEOUT_MS=3000 \
#   scripts/run-v1-screen.sh
# GALION_LLM defaults to hermes when GALION_LLM_URL is set and GALION_LLM is unset.
# GALION_LLM=canned is an explicit opt-in for tests. It is not the demo default.
# If the gateway is down the spoken line is "model can't be reached".
# That failure does not fall back to a canned reply.
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
# Canned is opt-in (`GALION_LLM=canned`). A gateway URL with no mode selects Hermes.
if [[ -z "${GALION_LLM:-}" && -n "${GALION_LLM_URL:-}" ]]; then
  export GALION_LLM=hermes
fi

mkdir -p "$OUT"
cargo build -q "${PROFILE[@]}" --offline -p galion-console --features dot-viz-web --example dot_console_demo
cargo build -q "${PROFILE[@]}" --offline -p galion-asr-vosk --bin galion-asr-vosk
TARGET="target/$([[ ${#PROFILE[@]} -gt 0 ]] && echo release || echo debug)"
BIN="$TARGET/examples/dot_console_demo"
export GALION_VOSK_BIN="$PWD/$TARGET/galion-asr-vosk"
MODEL="${GALION_VOSK_MODEL:-$PWD/weights/asr/vosk-model-small-en-us-0.15}"
export GALION_VOSK_MODEL="$MODEL"
# Library default is stub. This screen uses Vosk when the model is present.
# GALION_ASR=interim is test-only and must not invent words on :7878.
ASR_MODE="${GALION_ASR:-}"
if [[ "$ASR_MODE" == "interim" ]]; then
  echo "refusing GALION_ASR=interim on the live screen. Placeholder transcripts are test-only."
  echo "  Mic will not invent a transcript. See docs/asr/VOSK.md"
  ASR_MODE=""
fi
if [[ -z "$ASR_MODE" ]]; then
  if [[ -f "$MODEL/am/final.mdl" && -f "$MODEL/conf/model.conf" ]]; then
    export GALION_ASR=vosk
    echo "mic ASR: vosk ($MODEL)"
  else
    export GALION_ASR=stub
    echo "mic ASR: Vosk model missing at $MODEL"
    echo "  typed text still works. Mic turns return an error and do not invent a transcript."
    echo "  docs/asr/VOSK.md"
  fi
else
  export GALION_ASR="$ASR_MODE"
  echo "mic ASR: $GALION_ASR"
fi

"$BIN" --headless --realtime --no-png --web "$ADDR" --seconds "$SECS" ${ARGS[@]+"${ARGS[@]}"} > "$OUT/heartbeat.log" 2>&1 &
PID=$!
trap 'kill $PID 2>/dev/null || true' EXIT INT TERM

for _ in $(seq 1 50); do curl -fsS "${URL}health" >/dev/null 2>&1 && break; sleep 0.1; done
curl -fsS "${URL}health" >/dev/null 2>&1 || { echo "V1 screen failed to start (port in use?) — $OUT/heartbeat.log:"; cat "$OUT/heartbeat.log"; exit 1; }
echo "Galion Voice · one dot → $URL"

if [[ "$CHECK" == "1" ]]; then
  sleep 2
  page=$(curl -fsS "$URL")
  grep -q '<canvas id="s"' <<<"$page" && echo "  / : one canvas page OK ($(wc -c <<<"$page") bytes)" || { echo "  / : FAIL"; exit 1; }
  grep -q 'data-persona="samantha"' <<<"$page" && grep -q 'id="say-text"' <<<"$page" && grep -q 'data-mouth="piper"' <<<"$page" && echo "  / : persona, mic page, text box OK" || { echo "  / : persona/text FAIL"; exit 1; }
  v2hdr=$(curl -sI "${URL}v2")
  grep -q '302' <<<"$v2hdr" && grep -qi 'location: /' <<<"$v2hdr" && echo "  /v2 : redirect OK" || { echo "  /v2 : FAIL ($v2hdr)"; exit 1; }
  state=$(curl -fsS "${URL}v2/state")
  grep -q '"rights":"hold"' <<<"$state" && grep -q '"film_voice":false' <<<"$state" && grep -q '"llm"' <<<"$state" && echo "  /v2/state : rights hold OK" || { echo "  /v2/state : FAIL ($state)"; exit 1; }
  say=$(curl -fsS -X POST -H 'Content-Type: application/json' --data '{"text":"My mother died this morning."}' "${URL}v2/say")
  if grep -qi 'i hear you' <<<"$say"; then echo "  POST /v2/say : FAIL canned empathy ($say)"; exit 1; fi
  if ! grep -q '"llm"' <<<"$say"; then echo "  POST /v2/say : FAIL missing llm ($say)"; exit 1; fi
  if [[ -z "${GALION_LLM_URL:-}" && "${GALION_LLM:-}" != "hermes" ]]; then
    grep -q "model can't be reached" <<<"$say" || { echo "  POST /v2/say : FAIL ($say)"; exit 1; }
    echo "  POST /v2/say : no gateway, plain unreachable, no canned empathy"
  else
    echo "  POST /v2/say : hermes path, no canned empathy"
  fi
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
