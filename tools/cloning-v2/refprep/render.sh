#!/usr/bin/env bash
# Renders test12 for both refprep voices, ONE cbx at a time, after the picker job exits.
set -u
B=/workspace/swarm/voice-v2-sprint/cloning-v2
CBX=$B/pipeline/cbx-v2/target/release/cbx
export HF_HUB_OFFLINE=1
while pgrep -f run_synth_priority.sh >/dev/null; do sleep 30; done
echo "START $(date -Iseconds)"
for p in jarvis samantha; do
  v=own-$p-v2-refprep; mkdir -p $B/refprep/wav/$v
  while IFS=$'\t' read -r id txt; do
    out=$B/refprep/wav/$v/$id.wav
    [[ -s $out ]] && continue
    t0=$(date +%s.%N)
    nice -n 10 $CBX --intra-threads 2 speak --dtype fp32 --ep cpu --voice $v --temperature 0 --text "$txt" --out-wav $out > $B/refprep/logs/synth_${v}_$id.log 2>&1
    rc=$?; t1=$(date +%s.%N)
    echo "$v $id rc=$rc wall_s=$(echo "$t1-$t0"|bc) $(date -Iseconds)"
  done < $B/emotion-fix/test12.tsv
done
echo "END $(date -Iseconds)"
