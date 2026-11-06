#!/usr/bin/env bash
# score12.sh <tag> <wavdir>  -> candidates/scores/<tag>/{ecapa,wer,mos,hygiene,emotion_orig,emotion_calm,pros}.json*
set -euo pipefail
B=/workspace/swarm/voice-v2-sprint/cloning-v2
S=$B/scorers
C=$B/emotion-blend/candidates
tag=$1; d=$2
o=$C/scores/$tag; mkdir -p "$o" "$C/logs"
REF=/workspace/swarm/voice-v2-sprint/clip-store-own/samantha/reference.wav
CALM=$C/prep/samantha-calm-target.wav
NMR=/workspace/swarm/voice-v2-sprint/clip-store-own/jarvis/reference.wav
TEXT=$B/emotion-fix/text12
mapfile -t W < <(ls "$d"/t[0-9][0-9].wav | sort)
echo "[$(date '+%F %T %Z')] score $tag n=${#W[@]}" | tee -a "$C/logs/score.log"
nice -n 10 $S/ecapa/target/release/galion-ecapa --ref "$REF" --threads 2 --out "$o/ecapa.json" "${W[@]}" >/dev/null
nice -n 10 $S/quality/bin/galion-hygiene --out "$o/hygiene.json" "${W[@]}" >/dev/null
nice -n 10 $S/quality/bin/galion-mos --nmr "$NMR" --threads 2 --out "$o/mos.json" "${W[@]}" >/dev/null
nice -n 10 $S/quality/bin/galion-wer --text-dir "$TEXT" --jobs 2 --out "$o/wer.json" "${W[@]}" >/dev/null
nice -n 10 $S/emotion/target/release/galion-emotion score --threads 2 --ref "$REF" --out "$o/emotion_orig.json" "${W[@]}" >/dev/null
nice -n 10 $S/emotion/target/release/galion-emotion score --threads 2 --ref "$CALM" --out "$o/emotion_calm.json" "${W[@]}" >/dev/null
# F0 / pace via cbx-emo analyze
words=""
for w in "${W[@]}"; do
  id=$(basename "$w" .wav)
  wc=$(wc -w < "$TEXT/$id.txt")
  words="$words $wc"
done
$B/emotion-fix/cbx-emo/target/release/cbx analyze --wav "${W[@]}" --words $words 2>/dev/null | grep '^{' > "$o/pros.jsonl" || true
echo "[$(date '+%F %T %Z')] scored $tag" | tee -a "$C/logs/score.log"
