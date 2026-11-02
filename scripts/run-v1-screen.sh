#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
# V1 single screen: one dot on near-black at http://127.0.0.1:7878/, driven by the 80 ms heartbeat.
# LOCAL ONLY (galion-render refuses public binds). No PNGs, no second port, no extra window.
#
#   scripts/run-v1-screen.sh                  # serve for 1 h (Ctrl-C to stop)
#   scripts/run-v1-screen.sh --open           # + open it once, fullscreen kiosk (first browser found)
#   scripts/run-v1-screen.sh --check          # ~15 s self-test: page = one canvas, /dot modes cycle
#   scripts/run-v1-screen.sh --seconds 30     # other dot_console_demo flags pass through
#
# Env: V1_SCREEN_ADDR=127.0.0.1:7878 · V1_SCREEN_SECONDS=3600 · OUT=target/v1-screen (log) · RELEASE=1
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
  exit 0
fi

if [[ "$OPEN" == "1" ]]; then   # exactly one window: first browser found wins
  for b in chromium chromium-browser google-chrome google-chrome-stable; do
    command -v "$b" >/dev/null && { "$b" --kiosk --app="$URL" --noerrdialogs --disable-infobars --no-first-run >/dev/null 2>&1 & OPEN=0; break; }
  done
  [[ "$OPEN" == "1" ]] && command -v firefox >/dev/null && { firefox --kiosk "$URL" >/dev/null 2>&1 & }
fi
wait $PID || true
