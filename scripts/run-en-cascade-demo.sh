#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
#
# W21-03 · EN cascade demo lane (Sector 11 + EN ml-voice). LOCAL ONLY, offline,
# no weights, English only.
#   ASR-stub -> route (pin > hint > LID) -> MockLlm reply -> TTS-stub -> WAV
# FP-03: the reply WAV speaks REAL ENGLISH WORDS via the A21 neural TTS (Piper VITS
#   en_US-ljspeech-medium, CPU only, MIT; fetch: scripts/a21/fetch-a21.sh) when PRESENT,
#   else the flite fallback (libflite1, dlopen). None => exit 3 = SPEECH=ABSENT (FRXDEL Q2).
#
# Usage: scripts/run-en-cascade-demo.sh [--test] [--play] [--text "hello galion"]
#                                       [--pin en] [--hint en] [--emotion neutral] [--allow-tones 1]
# Env:   GALION_DEMO_OUT (default: <repo>/target/galion-demo)
#
# Stable outputs (live-demo hook; scripts/run-live-demo.sh should read these):
#   $GALION_DEMO_OUT/en-cascade.wav          reply as SPOKEN WORDS, 24 kHz mono PCM16, 80 ms frames
#   $GALION_DEMO_OUT/en-cascade-speech.wav   same speech (explicit name)
#   $GALION_DEMO_OUT/en-cascade-earcon.wav   diagnostic earcon (tones; NOT the reply)
#   $GALION_DEMO_OUT/en-cascade-fixture.wav  exact StubTts signal (loop check)
#   $GALION_DEMO_OUT/en-cascade.json         turn summary
#   $GALION_DEMO_OUT/en-roundtrip.wav        galion-ml-voice roundtrip example output
# Env:   GALION_SPEECH=auto|a21|flite|off  GALION_FLITE_VOICE=slt|kal16|awb|rms|kal
#        GALION_A21_RUNTIME=<piper exe>  GALION_A21_NOISE=natural (sampled prosody; default deterministic)
#        GALION_TTS_WEIGHTS / GALION_SPEECH_WEIGHTS (+ GALION_TTS_WEIGHTS_SHA256) = A21 slot
# Last stdout line: GALION_EN_CASCADE_WAV=<absolute path>
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT="$(pwd)"
OUT="${GALION_DEMO_OUT:-$ROOT/target/galion-demo}"
TEST=0; PLAY=0; TEXT="hello galion"; LANE_ARGS=()
while [ $# -gt 0 ]; do
  case "$1" in
    --test) TEST=1; shift ;;
    --play) PLAY=1; shift ;;
    --text) TEXT="$2"; shift 2 ;;
    --pin|--hint|--emotion|--allow-tones) LANE_ARGS+=("$1" "$2"); shift 2 ;;
    -h|--help) sed -n '2,25p' "$0"; exit 0 ;;
    *) echo "unknown arg: $1" >&2; exit 64 ;;
  esac
done
mkdir -p "$OUT"

if [ "$TEST" = 1 ]; then
  echo "== tests (offline)"
  cargo test --offline -q -p galion-cascade --features cascade
  cargo test --offline -q -p galion-ml-voice
  cargo test --offline -q -p galion-ml-voice --features speech
  cargo test --offline -q -p galion-sideband
fi

echo "== galion-ml-voice roundtrip (EN)"
cargo run --offline -q -p galion-ml-voice --example roundtrip -- \
  --out "$OUT/en-roundtrip.wav" --lang en --text "$TEXT"

echo "== galion-cascade EN lane"
rc=0; cargo run --offline -q -p galion-cascade --features cascade --example en_cascade -- \
  --out-dir "$OUT" --text "$TEXT" ${LANE_ARGS[@]+"${LANE_ARGS[@]}"} || rc=$?
if [ "$rc" = 3 ]; then
  echo "run-en-cascade-demo: exit 3 = SPEECH=ABSENT (no A21 pack, no libflite1). Fix: scripts/a21/fetch-a21.sh  or  apt install libflite1" >&2
  exit 3
fi
[ "$rc" = 0 ] || exit "$rc"

if command -v python3 >/dev/null 2>&1; then
  python3 - "$OUT" <<'PY'
import sys, wave, os
d = sys.argv[1]
import json
j = json.load(open(os.path.join(d, "en-cascade.json")))
print(f"speech : {j['reply_kind']} via {j['speech_backend_label']} (SPEECH={j.get('speech','?')})")
print(f"a21    : {j['a21_weights_detail']}")
for f in ("en-cascade.wav", "en-cascade-earcon.wav", "en-cascade-fixture.wav", "en-roundtrip.wav"):
    w = wave.open(os.path.join(d, f))
    assert (w.getframerate(), w.getnchannels(), w.getsampwidth()) == (24000, 1, 2), f
    print(f"check  : {f} ok (24000 Hz mono PCM16, {w.getnframes()*1000//24000} ms)")
PY
fi

if [ "$PLAY" = 1 ]; then
  if command -v paplay >/dev/null 2>&1; then paplay "$OUT/en-cascade.wav" || true
  elif command -v aplay >/dev/null 2>&1; then aplay -q "$OUT/en-cascade.wav" || true
  elif command -v afplay >/dev/null 2>&1; then afplay "$OUT/en-cascade.wav" || true
  else echo "play   : no audio player found (paplay/aplay/afplay); skipping"; fi
fi
echo "GALION_EN_CASCADE_WAV=$OUT/en-cascade.wav"
