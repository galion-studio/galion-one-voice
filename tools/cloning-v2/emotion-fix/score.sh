#!/bin/bash
# usage: score.sh <persona> <wavdir> <tag>   -> scores/<tag>_{emo,ecapa,pros}.json
set -e
cd /workspace/swarm/voice-v2-sprint/cloning-v2/emotion-fix
p=$1; d=$2; tag=$3
REF=../../clip-store-own/$p/reference.wav
W=$(ls $d/t[0-9][0-9].wav)
nice -n 10 ../scorers/emotion/target/release/galion-emotion score --threads 2 --compact --ref $REF $W --out scores/${tag}_emo.json
nice -n 10 ../scorers/ecapa/target/release/galion-ecapa --ref $REF $W --threads 2 --no-ceiling --out scores/${tag}_ecapa.json >/dev/null
words=""; for w in $W; do id=$(basename $w .wav); words="$words $(awk -F'\t' -v id=$id '$1==id{print $2}' test12.tsv | wc -w)"; done
./cbx-emo/target/release/cbx analyze --wav $W --words $words 2>/dev/null | grep '^{' > scores/${tag}_pros.jsonl
echo done $tag
