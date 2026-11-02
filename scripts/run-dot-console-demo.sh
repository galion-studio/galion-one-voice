#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
# W21-04 · one-dot render + live console, driven by the real 80 ms heartbeat.
# LOCAL ONLY: no network, no weights, no display server needed.
#
#   scripts/run-dot-console-demo.sh                 # TTY: live dot + bars (8 s); no TTY: headless
#   scripts/run-dot-console-demo.sh --headless      # fast headless run, PNG sequence + summary
#   (FP-01: --web removed — the one screen is scripts/run-v1-screen.sh / run-v1-demo.sh on :7878)
#   scripts/run-dot-console-demo.sh --seconds 30 --feed stub --sense-off   # any example flag passes through
#
# Env: OUT=target/dot-demo (PNG dir) · RELEASE=1 (release build) · NO_COLOR=1 · VIDEO=1 (mp4 if ffmpeg exists)
set -euo pipefail
cd "$(dirname "$0")/.."

OUT="${OUT:-${GALION_DEMO_OUT:+$GALION_DEMO_OUT/dot}}"; OUT="${OUT:-target/dot-demo}"
FEATURES="dot-viz"
ARGS=()
for a in "$@"; do
  case "$a" in
    --web) echo "run-dot-console-demo: --web removed (FP-01); use scripts/run-v1-screen.sh" >&2; exit 64 ;;
    *) ARGS+=("$a") ;;
  esac
done
PROFILE=()
[[ "${RELEASE:-0}" == "1" ]] && PROFILE=(--release)

mkdir -p "$OUT" && rm -f "$OUT"/frame_*.png "$OUT"/latest.png "$OUT"/summary.txt "$OUT"/dot.mp4
cargo run -q "${PROFILE[@]}" --offline -p galion-console --features "$FEATURES" \
  --example dot_console_demo -- --png-dir "$OUT" "${ARGS[@]}"

n=$(ls "$OUT"/frame_*.png 2>/dev/null | wc -l || true)
echo "artifacts: $OUT/latest.png · $n numbered frames · $OUT/summary.txt"
if [[ "${VIDEO:-0}" == "1" && "$n" -gt 0 ]] && command -v ffmpeg >/dev/null; then
  ffmpeg -loglevel error -y -framerate 12.5 -i "$OUT/frame_%06d.png" -pix_fmt yuv420p "$OUT/dot.mp4" \
    && echo "video: $OUT/dot.mp4"
fi
echo "GALION_DOT_DIR=$(cd "$OUT" && pwd)"
