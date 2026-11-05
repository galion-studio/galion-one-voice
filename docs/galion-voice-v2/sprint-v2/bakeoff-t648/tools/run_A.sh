#!/bin/bash
# Option A: OpenVoice v2 tone-colour converter (jingangdidi/voice_clone, candle) on Piper A21 output.
B=/workspace/swarm/voice-v2-sprint/bakeoff-t648; VC=$B/src/voice_clone/target/release/voice_clone; LOCK=/workspace/swarm/voice-v2-sprint/latency-240/BENCH.lock
T=${T:-4}
for tgt in samantha-orig samantha-ALT-src-17.84-29.25 jarvis-orig; do
  d=$B/out/A/$tgt; mkdir -p $d; cp -n $B/refs/$tgt.wav $d/target.wav
  for r in 1 2 3 4 5; do
    flock $LOCK python3 $B/tools/bench.py "A $tgt rep$r T$T" -- $VC -s $B/out/piper/s1.wav:$B/out/piper/s2.wav:$B/out/piper/s3.wav -t $d/target.wav -n s1-r$r.wav:s2-r$r.wav:s3-r$r.wav -m $B/src/checkpoints_v2/converter -T $T -o $d -S 2>>$d/stdout.log >> $B/logs/A-bench.jsonl
  done
done
