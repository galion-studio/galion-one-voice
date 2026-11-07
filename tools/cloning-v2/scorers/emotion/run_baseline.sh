#!/usr/bin/env bash
# Baseline emotion scores for own-samantha / own-jarvis refs + own-clone-analysis test lines.
set -euo pipefail
cd "$(dirname "$0")"
B=target/release/galion-emotion
O=/workspace/swarm/voice-v2-sprint/own-clone-analysis/wav
R=/workspace/swarm/voice-v2-sprint/clip-store-own
TIMEFORMAT='real %R user %U sys %S'
for v in samantha jarvis; do
  { time nice -n 10 $B score --threads 2 --ref $R/$v/reference.wav $O/$v/t*.wav --out results/baseline_$v.json ; } 2> logs/baseline_$v.time
  cat logs/baseline_$v.time
done
