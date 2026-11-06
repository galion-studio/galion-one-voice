#!/usr/bin/env bash
set -euo pipefail
CBX=/workspace/swarm/voice-v2-sprint/bakeoff-t648/src/cbx/target/release/cbx
BASE=/workspace/swarm/voice-v2-sprint/cloning-v2/similarity
TEXT=$BASE/text; WAV=$BASE/wav; LOG=$BASE/logs
mkdir -p "$WAV" "$LOG"
# Priority: cleaned both rp; multi only rp1.2 (skip multi rp1.0)
configs=(
  "own-samantha-v2-sim-cleaned|sam-cleaned|1.2"
  "own-samantha-v2-sim-cleaned|sam-cleaned|1.0"
  "own-samantha-v2-sim-multi|sam-multi|1.2"
  "own-jarvis-v2-sim-cleaned|jar-cleaned|1.2"
  "own-jarvis-v2-sim-cleaned|jar-cleaned|1.0"
  "own-jarvis-v2-sim-multi|jar-multi|1.2"
)
lines=(t01 t02 t03 t04 t05 t06 t07 t08 t09 t10 t11 t12)
echo "PRI_START $(date -Iseconds)" | tee -a "$LOG/synth_priority.log"
for cfg in "${configs[@]}"; do
  IFS='|' read -r voice label rp <<<"$cfg"
  outdir="$WAV/${label}_rp${rp}"; mkdir -p "$outdir"
  for tid in "${lines[@]}"; do
    out="$outdir/${tid}.wav"
    if [[ -f "$out" && -s "$out" ]]; then
      echo "SKIP $label rp=$rp $tid" | tee -a "$LOG/synth_priority.log"; continue
    fi
    # If another cbx is writing this exact out, wait
    txt=$(cat "$TEXT/${tid}.txt")
    echo "SYNTH $label rp=$rp $tid $(date -Iseconds)" | tee -a "$LOG/synth_priority.log"
    nice -n 10 "$CBX" --intra-threads 2 speak --dtype fp32 --ep cpu --voice "$voice" \
      --text "$txt" --repetition-penalty "$rp" --out-wav "$out" \
      >"$LOG/synth_${label}_rp${rp}_${tid}.log" 2>&1 || {
        echo "FAIL $label rp=$rp $tid" | tee -a "$LOG/synth_priority.log"; continue; }
    echo "DONE $label rp=$rp $tid $(stat -c%s "$out")b" | tee -a "$LOG/synth_priority.log"
  done
done
echo "PRI_END $(date -Iseconds)" | tee -a "$LOG/synth_priority.log"
