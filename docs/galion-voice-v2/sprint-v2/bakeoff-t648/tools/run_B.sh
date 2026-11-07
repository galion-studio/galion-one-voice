#!/bin/bash
# Option B: Pocket TTS official Rust binary v3.3.0 (release v4.0.1). Cloning weights GATED -> RTF only, predefined voice 'alba'.
B=/workspace/swarm/voice-v2-sprint/bakeoff-t648; PT=$B/src/pocket-bin/pocket-tts-3.3.0-x86_64-unknown-linux-gnu/pocket-tts; LOCK=/workspace/swarm/voice-v2-sprint/latency-240/BENCH.lock
for L in english english_18m_2026-10; do for TH in 1 2 4; do
  d=$B/out/B/rtf-$L-T$TH; mkdir -p $d
  for s in 1 2 3; do
    echo "== $L T$TH s$s" >> $d/stdout.log
    HF_HOME=$B/hf RAYON_NUM_THREADS=$TH flock $LOCK python3 $B/tools/bench.py "B $L alba s$s T$TH" -- $PT generate --language $L --voice alba --text "$(cat $B/text/s$s.txt)" --output-path $d/s$s.wav 2>>$d/stdout.log >> $B/logs/B-bench.jsonl
  done
done; done
