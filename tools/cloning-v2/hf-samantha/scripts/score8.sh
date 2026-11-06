#!/bin/bash
# score t01-t08 only. usage: score8.sh NAME WAVDIR
set -uo pipefail
NAME="$1"; WD="$2"
BASE=/workspace/swarm/voice-v2-sprint/cloning-v2/hf-samantha
S=/workspace/swarm/voice-v2-sprint/cloning-v2/scorers
REF=/workspace/swarm/voice-v2-sprint/clip-store-own/samantha/reference.wav
NMR=/workspace/swarm/voice-v2-sprint/clip-store-own/jarvis/reference.wav
OUT="$BASE/scores8/$NAME"; mkdir -p "$OUT"
W=(); for t in t01 t02 t03 t04 t05 t06 t07 t08; do W+=("$WD/$t.wav"); done
Q=$S/quality/galion-quality/target/release
nice -n 10 $S/ecapa/target/release/galion-ecapa --ref "$REF" --threads 2 --out "$OUT/ecapa.json" "${W[@]}"
nice -n 10 $Q/galion-hygiene --out "$OUT/hygiene.json" "${W[@]}"
nice -n 10 $Q/galion-mos --nmr "$NMR" --threads 2 --out "$OUT/mos.json" "${W[@]}"
nice -n 10 $Q/galion-wer --text-dir "$BASE/text" --jobs 2 --out "$OUT/wer.json" "${W[@]}"
nice -n 10 $S/emotion/target/release/galion-emotion score --ref "$REF" --threads 2 --out "$OUT/emotion.json" "${W[@]}"
nice -n 10 $S/articulation/bin/galion-artic profile -j 2 --out "$OUT/artic.json" "${W[@]}"
echo DONE_$NAME
