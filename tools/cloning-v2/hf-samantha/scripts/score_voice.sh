#!/bin/bash
set -euo pipefail
VOICE="$1"
BASE=/workspace/swarm/voice-v2-sprint/cloning-v2/hf-samantha
REF=/workspace/swarm/voice-v2-sprint/clip-store-own/samantha/reference.wav
NMR=/workspace/swarm/voice-v2-sprint/clip-store-own/jarvis/reference.wav
OUT="$BASE/scores/$VOICE"
mkdir -p "$OUT"
mapfile -t WAVS < <(ls "$BASE/wav/$VOICE"/t*.wav | sort)
echo "Scoring $VOICE n=${#WAVS[@]}"
ECAPA=/workspace/swarm/voice-v2-sprint/cloning-v2/scorers/ecapa/target/release/galion-ecapa
MOS=/workspace/swarm/voice-v2-sprint/cloning-v2/scorers/quality/galion-quality/target/release/galion-mos
WER=/workspace/swarm/voice-v2-sprint/cloning-v2/scorers/quality/galion-quality/target/release/galion-wer
HYG=/workspace/swarm/voice-v2-sprint/cloning-v2/scorers/quality/galion-quality/target/release/galion-hygiene
EMO=/workspace/swarm/voice-v2-sprint/cloning-v2/scorers/emotion/target/release/galion-emotion
nice -n 10 "$ECAPA" --ref "$REF" --threads 2 --out "$OUT/ecapa.json" "${WAVS[@]}"
nice -n 10 "$HYG" --out "$OUT/hygiene.json" "${WAVS[@]}"
nice -n 10 "$MOS" --nmr "$NMR" --threads 2 --out "$OUT/mos.json" "${WAVS[@]}"
nice -n 10 "$WER" --text-dir "$BASE/text" --jobs 2 --out "$OUT/wer.json" "${WAVS[@]}"
nice -n 10 "$EMO" score --ref "$REF" --threads 2 --out "$OUT/emotion.json" "${WAVS[@]}"
echo DONE_SCORE_"$VOICE"
