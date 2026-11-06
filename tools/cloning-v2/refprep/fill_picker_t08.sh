#!/usr/bin/env bash
# Fill the one picker line that failed (sam-cleaned rp1.2 t08) with the SAME binary/args as run_synth_priority.sh; runs only after render.sh ends.
S=/workspace/swarm/voice-v2-sprint/cloning-v2/similarity
while pgrep -f "render.sh|run_synth_priority.sh" >/dev/null; do sleep 30; done
out=$S/wav/sam-cleaned_rp1.2/t08.wav; [[ -s $out ]] && exit 0
nice -n 10 /workspace/swarm/voice-v2-sprint/bakeoff-t648/src/cbx/target/release/cbx --intra-threads 2 speak --dtype fp32 --ep cpu --voice own-samantha-v2-sim-cleaned \
  --text "$(cat $S/text/t08.txt)" --repetition-penalty 1.2 --out-wav $out > $S/logs/synth_sam-cleaned_rp1.2_t08_refill.log 2>&1
echo "REFILL sam-cleaned rp=1.2 t08 rc=$? $(date -Iseconds)" | tee -a $S/logs/synth_priority.log
