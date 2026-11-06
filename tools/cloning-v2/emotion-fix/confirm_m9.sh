#!/bin/bash
set -euo pipefail
cd /workspace/swarm/voice-v2-sprint/cloning-v2/emotion-fix
export HF_HUB_OFFLINE=1
LOG=logs/confirm_m9.log
TAG=jd_m9_w8_12
VOICE=own-jarvis-v2-emo-m6
REF=../../clip-store-own/jarvis/reference.wav
mkdir -p runs/$TAG scores logs

wait_slot() {
  while true; do
    n=$(pgrep -af '/cbx .* (speak|batch|voice)' | grep -v grep | grep -v confirm_m9 | wc -l || true)
    echo "$(date -Iseconds) wait cbx=$n load=$(awk '{print $1}' /proc/loadavg)" | tee -a "$LOG"
    [[ "$n" -le 1 ]] && return 0
    sleep 20
  done
}

echo "$(date -Iseconds) CONFIRM START $TAG" | tee -a "$LOG"
wait_slot
nice -n 10 ./cbx-emo/target/release/cbx --intra-threads 2 batch --dtype fp32 --voice $VOICE --manifest test12.tsv --out-dir runs/$TAG --target targets/jarvis.json > runs/$TAG/log.txt 2>&1
echo "$(date -Iseconds) RENDER DONE" | tee -a "$LOG"

W=$(ls runs/$TAG/t[0-9][0-9].wav | sort)
nice -n 10 ../scorers/emotion/target/release/galion-emotion score --threads 2 --compact --ref $REF $W --out scores/${TAG}_emo.json
nice -n 10 ../scorers/ecapa/target/release/galion-ecapa --ref $REF $W --threads 2 --no-ceiling --out scores/${TAG}_ecapa.json >/dev/null
# WER needs text dir - reuse validation12 texts or build from test12.tsv
TEXTDIR=runs/$TAG/texts
mkdir -p "$TEXTDIR"
while IFS=$'\t' read -r id text; do
  [[ -z "${id:-}" ]] && continue
  printf '%s\n' "$text" > "$TEXTDIR/${id}.txt"
done < test12.tsv
nice -n 10 ../scorers/quality/bin/galion-wer --jobs 2 --text-dir "$TEXTDIR" --out scores/${TAG}_wer.json $W
# MOS nmr: validation used samantha ref by mistake? persona report used jarvis. Use jarvis ref as nmr.
nice -n 10 ../scorers/quality/bin/galion-mos --threads 2 --nmr $REF --out scores/${TAG}_mos.json $W

# prosody
words=""; for w in $W; do id=$(basename $w .wav); words="$words $(awk -F'\t' -v id=$id '$1==id{print $2}' test12.tsv | wc -w)"; done
./cbx-emo/target/release/cbx analyze --wav $W --words $words 2>/dev/null | grep '^{' > scores/${TAG}_pros.jsonl || true
./cbx-emo/target/release/cbx summarize $TAG 2>/dev/null | grep '^{' | tee -a scores/summary.jsonl || true
echo "$(date -Iseconds) CONFIRM_DONE" | tee -a "$LOG"
echo CONFIRM_DONE
