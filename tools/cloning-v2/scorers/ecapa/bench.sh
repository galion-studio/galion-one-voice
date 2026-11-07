#!/bin/bash
# Benchmark Python (score_t648 emb/cos) vs Rust galion-ecapa on the own-clone test set. nice 10, 2 threads.
cd "$(dirname "$0")"; V=../../../venv/bin/python; OA=/workspace/swarm/voice-v2-sprint/own-clone-analysis/wav; CS=/workspace/swarm/voice-v2-sprint/clip-store-own
TIMEFORMAT='%R wall %U user %S sys'
for rep in 1 2; do for p in samantha jarvis; do
  echo -n "py $p rep$rep: "; { time nice -n 10 $V py/ref_score.py out/py_$p.json --ref $CS/$p/reference.wav $OA/$p/t*.wav >/dev/null 2>&1; } 2>&1
  echo -n "rs $p rep$rep: "; { time nice -n 10 target/release/galion-ecapa --ref $CS/$p/reference.wav $OA/$p/t*.wav --out out/rs_$p.json >/dev/null; } 2>&1
done; done
