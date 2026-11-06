#!/bin/bash
set -euo pipefail
cd /workspace/swarm/voice-v2-sprint/cloning-v2/emotion-fix
export HF_HUB_OFFLINE=1
CBX=./cbx-emo/target/release/cbx
REF=../../clip-store-own/jarvis/reference.wav
LOG=logs/queue_mild.log
mkdir -p logs runs scores prepped

wait_slot() {
  local label="$1"
  while true; do
    # count active speak/batch/voice-add cbx (exclude this script's grep)
    n=$(pgrep -af '/cbx .* (speak|batch|voice)' | grep -v grep | grep -v queue_mild | wc -l || true)
    load=$(awk '{print $1}' /proc/loadavg)
    echo "$(date -Iseconds) wait_slot $label cbx=$n load=$load" | tee -a "$LOG"
    # allow start when <=1 other cbx (box already busy; stay at one of ours)
    if [[ "$n" -le 1 ]]; then
      return 0
    fi
    sleep 20
  done
}

create_voice() {
  local name="$1"; shift
  if [[ -f /home/box/.cache/huggingface/cbx/voices/${name}.cbxvoice ]]; then
    echo "$(date -Iseconds) voice exists: $name" | tee -a "$LOG"
    return 0
  fi
  wait_slot "voice-$name"
  echo "$(date -Iseconds) creating $name $*" | tee -a "$LOG"
  nice -n 10 $CBX --intra-threads 2 voice add --dtype fp32 --name "$name" "$@" >>"$LOG" 2>&1
  echo "$(date -Iseconds) created $name" | tee -a "$LOG"
}

run_one() {
  local persona=jarvis voice="$1" tag="$2" man="$3"; shift 3
  if [[ -f runs/$tag/t01.wav && -f runs/$tag/t08.wav && -f scores/${tag}_emo.json ]]; then
    echo "$(date -Iseconds) skip done $tag" | tee -a "$LOG"
    return 0
  fi
  wait_slot "run-$tag"
  echo "$(date -Iseconds) RUN $tag voice=$voice extras=$*" | tee -a "$LOG"
  ./run_cfg.sh "$persona" "$voice" "$tag" "$man" "$@" >>"$LOG" 2>&1
  echo "$(date -Iseconds) DONE $tag" | tee -a "$LOG"
}

echo "$(date -Iseconds) MILD QUEUE START" | tee -a "$LOG"

# Voices (new only own-jarvis-v2-emo-m*)
# m1: half-strength LUFS norm via cbx (-24.85)
create_voice own-jarvis-v2-emo-m1 --voice-wav "$REF" --norm-lufs -24.85 --dump-prepped prepped/jarvis-m1-cbx.wav
# m2: warm crop first 10s (keeps high-valence start in encoder window; no LN)
create_voice own-jarvis-v2-emo-m2 --voice-wav "$REF" --crop-end 10.0 --dump-prepped prepped/jarvis-m2-cbx.wav
# m3: better-sim multiclip long LN pack
create_voice own-jarvis-v2-emo-m3 --voice-wav ../similarity/refs/jarvis-multiclip-long-ln.wav --dump-prepped prepped/jarvis-m3-cbx.wav
# m4: better-sim cleaned LN
create_voice own-jarvis-v2-emo-m4 --voice-wav ../similarity/refs/jarvis-cleaned-ln.wav --dump-prepped prepped/jarvis-m4-cbx.wav

# Dev4 renders — milder variants
# Isolate ref changes at temp 0 (preferred default; prior wins for Sam used 0.8 which we won't rely on)
run_one own-jarvis-v2-emo-m1 jd_m1_ln   dev4.tsv
run_one own-jarvis-v2-emo-m2 jd_m2_warm dev4.tsv
run_one own-jarvis-v2-emo-m3 jd_m3_sim  dev4.tsv
run_one own-jarvis-v2-emo-m4 jd_m4_simc dev4.tsv
# Milder sampling on baseline voice (half of previous up: temp 0.4, top-p 0.975)
run_one own-jarvis jd_m5_mup dev4.tsv --temperature 0.4 --top-k 1000 --top-p 0.975 --rep-unique --punc-norm
# Combos: mild LN + mild up; sim-multi + mild up
run_one own-jarvis-v2-emo-m1 jd_m6_ln_mup  dev4.tsv --temperature 0.4 --top-k 1000 --top-p 0.975 --rep-unique --punc-norm
run_one own-jarvis-v2-emo-m3 jd_m7_sim_mup dev4.tsv --temperature 0.4 --top-k 1000 --top-p 0.975 --rep-unique --punc-norm

echo "$(date -Iseconds) DEV4 PHASE DONE" | tee -a "$LOG"
# Summarize for selection (parent/follow-up script will pick best for test12)
$CBX summarize jd_m1_ln jd_m2_warm jd_m3_sim jd_m4_simc jd_m5_mup jd_m6_ln_mup jd_m7_sim_mup jar_old 2>/dev/null | tee -a "$LOG" || true
echo MILD_DEV4_DONE | tee -a "$LOG"
