#!/bin/bash
# usage: run_cfg.sh <persona> <voice> <tag> <manifest> [extra cbx batch args...]
set -e
cd /workspace/swarm/voice-v2-sprint/cloning-v2/emotion-fix
p=$1; v=$2; tag=$3; man=$4; shift 4
export HF_HUB_OFFLINE=1
mkdir -p runs/$tag
nice -n 10 ./cbx-emo/target/release/cbx --intra-threads 2 batch --dtype fp32 --voice $v --manifest $man --out-dir runs/$tag --target targets/$p.json "$@" > runs/$tag/log.txt 2>&1
./score.sh $p runs/$tag $tag >> runs/$tag/log.txt 2>&1
./cbx-emo/target/release/cbx summarize $tag 2>/dev/null | grep "^{" | tee -a scores/summary.jsonl
