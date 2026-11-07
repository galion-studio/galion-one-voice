#!/usr/bin/env bash
set -euo pipefail
CBX=/workspace/swarm/voice-v2-sprint/cloning-v2/pipeline/cbx-v2/target/release/cbx
TEXT=/workspace/swarm/voice-v2-sprint/own-clone-analysis/text
OUT_ROOT=/workspace/samples/v2/pipeline
PIPE_WAV=/workspace/swarm/voice-v2-sprint/cloning-v2/pipeline/wav
LOG=/workspace/swarm/voice-v2-sprint/cloning-v2/pipeline/logs/synth.log
export LD_LIBRARY_PATH=/workspace/swarm/voice-v2-sprint/cloning-v2/pipeline/lib:${LD_LIBRARY_PATH:-}
mkdir -p "$OUT_ROOT/samantha" "$OUT_ROOT/jarvis" "$PIPE_WAV/samantha" "$PIPE_WAV/jarvis"
: > "$LOG"

synth_one() {
  local voice="$1" persona="$2" tid="$3" pace="$4" pitch="$5"
  local text; text=$(cat "$TEXT/${tid}.txt")
  local out="$OUT_ROOT/$persona/${tid}.wav"
  local mirror="$PIPE_WAV/$persona/${tid}.wav"
  echo "[$(date +%H:%M:%S)] START $persona $tid pace=$pace pitch=$pitch" | tee -a "$LOG"
  # Note: temperature left at 0 (greedy). Non-zero temp can miss STOP and run to max_new_tokens
  # on this ONNX vocab; flags remain available for A/B. Seed accepted but unused at temp=0.
  nice -n 10 "$CBX" --intra-threads 2 speak \
    --dtype fp32 \
    --voice "$voice" \
    --text "$text" \
    --out-wav "$out" \
    --max-new-tokens 1500 \
    --repetition-penalty 1.0 \
    --temperature 0.0 \
    --top-p 0.9 \
    --seed 42 \
    --pace "$pace" \
    --pitch-semitones="$pitch" \
    >>"$LOG" 2>&1
  cp -f "$out" "$mirror"
  echo "[$(date +%H:%M:%S)] DONE $persona $tid size=$(stat -c%s "$out")" | tee -a "$LOG"
}

for tid in t01 t02 t03 t04 t05 t06 t07 t08 t09 t10 t11 t12; do
  synth_one own-samantha-v2-pipe samantha "$tid" 0.85 -1.0
done
for tid in t01 t02 t03 t04 t05 t06 t07 t08 t09 t10 t11 t12; do
  synth_one own-jarvis-v2-pipe jarvis "$tid" 0.82 -1.5
done
echo "[$(date +%H:%M:%S)] ALL DONE" | tee -a "$LOG"
