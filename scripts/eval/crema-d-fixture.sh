#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
# Build an emotion-eval test tree from the public CREMA-D benchmark (ODbL; no own-voice audio).
#   build/<emotion>/  one actor's clips (+ <clip>.txt transcript for WER) = the "voice build" under test
#   ref/<emotion>/    the other actors' clips of that emotion = emotion references
#   speaker-ref.wav   a different sentence by the same actor = ECAPA speaker reference
# Usage: scripts/eval/crema-d-fixture.sh OUT_DIR [ACTOR=1001]
set -euo pipefail
OUT="$1"; ACTOR="${2:-1001}"
M="${GALION_MODELS:-/mnt/g/galion/models}"; D="$M/bench/crema-d/AudioWAV"
[ -d "$D" ] || { echo "CREMA-D not under $D" >&2; exit 2; }
declare -A EMO=([ANG]=angry [DIS]=disgusted [FEA]=fearful [HAP]=happy [NEU]=neutral [SAD]=sad)
declare -A TXT=([DFA]="Don't forget a jacket" [ITS]="I think I've seen this before" [IEO]="It's eleven o'clock"
  [TIE]="That is exactly what happened" [IOM]="I'm on my way to the meeting" [IWW]="I wonder what this is about"
  [TAI]="The airplane is almost full" [MTI]="Maybe tomorrow it will be cold" [IWL]="I would like a new alarm clock"
  [ITH]="I think I have a doctor's appointment" [TSI]="The surface is slick" [WSI]="We'll stop in a couple of minutes")
rm -rf "$OUT"; mkdir -p "$OUT/build" "$OUT/ref"
for f in "$D"/*.wav; do
  b="$(basename "$f" .wav)"; IFS=_ read -r a s e _ <<< "$b"; emo="${EMO[$e]:-}"; [ -n "$emo" ] || continue
  if [ "$a" = "$ACTOR" ]; then mkdir -p "$OUT/build/$emo"; ln -sf "$f" "$OUT/build/$emo/$b.wav"; printf '%s\n' "${TXT[$s]}" > "$OUT/build/$emo/$b.txt"
  else mkdir -p "$OUT/ref/$emo"; ln -sf "$f" "$OUT/ref/$emo/$b.wav"; fi
done
SPK="$M/bench/crema-d/speaker-ref/${ACTOR}_IWW_NEU_XX.wav"
[ -f "$SPK" ] || { echo "missing speaker reference $SPK" >&2; exit 2; }
ln -sf "$SPK" "$OUT/speaker-ref.wav"
echo "fixture: actor $ACTOR, $(find "$OUT/build" -name '*.wav' | wc -l) build clips, $(find "$OUT/ref" -name '*.wav' | wc -l) reference clips"
