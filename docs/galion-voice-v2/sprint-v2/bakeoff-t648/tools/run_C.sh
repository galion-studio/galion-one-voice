#!/bin/bash
# Option C: Chatterbox Turbo ONNX via cbx (Rust, ort). Greedy argmax decoding => deterministic; 2 reps to verify.
B=/workspace/swarm/voice-v2-sprint/bakeoff-t648; CBX=$B/src/cbx/target/release/cbx; LOCK=/workspace/swarm/voice-v2-sprint/latency-240/BENCH.lock
DT=${DT:-fp32}; TH=${TH:-4}; REPS=${REPS:-2}
for tgt in samantha-orig samantha-ALT-src-17.84-29.25 jarvis-orig; do
  d=$B/out/C-$DT/$tgt; mkdir -p $d
  name=t648-$(echo $tgt | cut -c1-12)-$DT
  flock $LOCK python3 $B/tools/bench.py "C $DT voiceadd $tgt T$TH" -- $CBX --intra-threads $TH voice add --name $name --voice-wav $B/refs/$tgt.wav --dtype $DT 2>>$d/stdout.log >> $B/logs/C-bench.jsonl
  for r in $(seq 1 $REPS); do for s in 1 2 3; do
    flock $LOCK python3 $B/tools/bench.py "C $DT $tgt s$s rep$r T$TH" -- $CBX --intra-threads $TH speak --dtype $DT --voice $name --text "$(cat $B/text/s$s.txt)" --out-wav $d/s$s-r$r.wav 2>>$d/stdout.log >> $B/logs/C-bench.jsonl
  done; done
done
