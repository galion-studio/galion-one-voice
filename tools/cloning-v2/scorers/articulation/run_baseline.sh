#!/usr/bin/env bash
# Reproduce galion-artic baselines (read-only on inputs; writes only into ./results). nice'd, 2 threads.
set -euo pipefail
cd "$(dirname "$0")"
S=/workspace/swarm/voice-v2-sprint; B=./bin/galion-artic
for v in samantha jarvis; do
  nice -n 10 $B profile -j 2 --out results/profile_${v}_original.json $S/clip-store-own/$v/original.wav
  nice -n 10 $B profile -j 2 --out results/profile_${v}_clones_t01-t12.json $S/own-clone-analysis/wav/$v/t*.wav
done
nice -n 10 $B compare -j 2 --ref $S/clip-store-own/samantha/original.wav --clone work/selfclone/own-samantha.wav --clone work/selfclone/own-samantha-v2-hf-a-clean.wav --out results/compare_samantha_selfclone.json
nice -n 10 $B compare -j 2 --ref $S/clip-store-own/jarvis/original.wav --clone work/selfclone/own-jarvis.wav --out results/compare_jarvis_selfclone.json
# accuracy vs Praat (needs ./venv with praat-parselmouth)
nice -n 10 venv/bin/python crosscheck/crosscheck.py $B $S/clip-store-own/samantha/original.wav $S/clip-store-own/jarvis/original.wav $S/own-clone-analysis/wav/samantha/t0{1,6,9}.wav $S/own-clone-analysis/wav/jarvis/t0{1,6,9}.wav work/selfclone/own-samantha.wav > results/crosscheck_praat.json
