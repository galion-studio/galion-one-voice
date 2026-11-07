#!/bin/bash
set -euo pipefail
cd /workspace/swarm/voice-v2-sprint/cloning-v2/emotion-fix
export HF_HUB_OFFLINE=1
CBX=./cbx-emo/target/release/cbx
REF=../../clip-store-own/jarvis/reference.wav
LOG=logs/queue_mild2.log
mkdir -p logs runs scores

wait_slot() {
  local label="$1"
  while true; do
    n=$(pgrep -af '/cbx .* (speak|batch|voice)' | grep -v grep | grep -v queue_mild | wc -l || true)
    echo "$(date -Iseconds) wait_slot $label cbx=$n load=$(awk '{print $1}' /proc/loadavg)" | tee -a "$LOG"
    if [[ "$n" -le 1 ]]; then return 0; fi
    sleep 25
  done
}

create_voice() {
  local name="$1"; shift
  if [[ -f /home/box/.cache/huggingface/cbx/voices/${name}.cbxvoice ]]; then
    echo "$(date -Iseconds) voice exists: $name" | tee -a "$LOG"; return 0
  fi
  wait_slot "voice-$name"
  echo "$(date -Iseconds) creating $name $*" | tee -a "$LOG"
  nice -n 10 $CBX --intra-threads 2 voice add --dtype fp32 --name "$name" "$@" >>"$LOG" 2>&1
  echo "$(date -Iseconds) created $name" | tee -a "$LOG"
}

run_one() {
  local voice="$1" tag="$2"; shift 2
  if [[ -f runs/$tag/t01.wav && -f scores/${tag}_emo.json ]]; then
    echo "$(date -Iseconds) skip $tag" | tee -a "$LOG"; return 0
  fi
  wait_slot "run-$tag"
  echo "$(date -Iseconds) RUN $tag $*" | tee -a "$LOG"
  ./run_cfg.sh jarvis "$voice" "$tag" dev4.tsv "$@" >>"$LOG" 2>&1
  echo "$(date -Iseconds) DONE $tag" | tee -a "$LOG"
}

echo "$(date -Iseconds) MILD2 START" | tee -a "$LOG"
# m5: mild rotate 2.0s — stays in high-valence zone (w0-w2), unlike failed r6/r10
create_voice own-jarvis-v2-emo-m5 --voice-wav "$REF" --rotate 2.0 --dump-prepped prepped/jarvis-m5-cbx.wav
# m6: warmer crop 8s (tighter than m2's 10s; still covers encoder 6s + some decoder)
create_voice own-jarvis-v2-emo-m6 --voice-wav "$REF" --crop-end 8.0 --dump-prepped prepped/jarvis-m6-cbx.wav
# m7: quarter-strength LN (~-23.8; quarter of way from -22.7 to -27)
create_voice own-jarvis-v2-emo-m7 --voice-wav "$REF" --norm-lufs -23.8 --dump-prepped prepped/jarvis-m7-cbx.wav
# m8: sim-cleaned + warm crop 10 via pre-made wav
# build warm crop of cleaned
if [[ ! -f prepped/jarvis-simc-warm10.wav ]]; then
  ffmpeg -y -nostdin -loglevel error -i ../similarity/refs/jarvis-cleaned-ln.wav -t 10 -ar 24000 -ac 1 prepped/jarvis-simc-warm10.wav
fi
create_voice own-jarvis-v2-emo-m8 --voice-wav prepped/jarvis-simc-warm10.wav --dump-prepped prepped/jarvis-m8-cbx.wav

run_one own-jarvis-v2-emo-m5 jd_m8_r2   # tag jd_m8_r2 for rotate2
run_one own-jarvis-v2-emo-m6 jd_m9_w8
run_one own-jarvis-v2-emo-m7 jd_m10_qln
run_one own-jarvis-v2-emo-m8 jd_m11_simc_w10

echo "$(date -Iseconds) MILD2_DEV4_DONE" | tee -a "$LOG"
echo MILD2_DEV4_DONE | tee -a "$LOG"
