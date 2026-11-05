#!/bin/bash
# reps 3..5 for option C (outputs proved non-deterministic across reps)
B=/workspace/swarm/voice-v2-sprint/bakeoff-t648; CBX=$B/src/cbx/target/release/cbx; LOCK=/workspace/swarm/voice-v2-sprint/latency-240/BENCH.lock
DT=${DT:-fp32}; TH=${TH:-4}
while pgrep -f tools/run_C.sh >/dev/null; do sleep 10; done
for tgt in samantha-orig samantha-ALT-src-17.84-29.25 jarvis-orig; do
  d=$B/out/C-$DT/$tgt; name=t648-$(echo $tgt | cut -c1-12)-$DT
  for r in 3 4 5; do for s in 1 2 3; do
    flock $LOCK python3 $B/tools/bench.py "C $DT $tgt s$s rep$r T$TH" -- $CBX --intra-threads $TH speak --dtype $DT --voice $name --text "$(cat $B/text/s$s.txt)" --out-wav $d/s$s-r$r.wav 2>>$d/stdout.log >> $B/logs/C-bench.jsonl
  done; done
done
