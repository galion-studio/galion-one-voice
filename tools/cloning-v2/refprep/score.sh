#!/bin/bash
# usage: score.sh <persona> <wavdir> <tag>  -> scores/<tag>/{ecapa,wer,mos,hygiene,emotion}.json
set -eu
B=/workspace/swarm/voice-v2-sprint/cloning-v2; S=$B/scorers
p=$1; d=$2; tag=$3; o=${OUTROOT:-$B/refprep/scores}/$tag; mkdir -p $o
REF=$B/../clip-store-own/$p/reference.wav
[[ $p == samantha ]] && NMR=$B/../clip-store-own/jarvis/reference.wav || NMR=$B/../clip-store-own/samantha/reference.wav
mapfile -t W < <(ls $d/t[0-9][0-9].wav | sort)
t0=$(date +%s)
nice -n 10 $S/ecapa/target/release/galion-ecapa --ref $REF --threads 2 --out $o/ecapa.json "${W[@]}" >/dev/null
nice -n 10 $S/quality/bin/galion-hygiene --out $o/hygiene.json "${W[@]}" >/dev/null
nice -n 10 $S/quality/bin/galion-mos --nmr $NMR --threads 2 --out $o/mos.json "${W[@]}" >/dev/null
nice -n 10 $S/quality/bin/galion-wer --text-dir $B/similarity/text --jobs 2 --out $o/wer.json "${W[@]}" >/dev/null
nice -n 10 $S/emotion/target/release/galion-emotion score --threads 2 --ref $REF --out $o/emotion.json "${W[@]}" >/dev/null
echo "scored $tag n=${#W[@]} in $(( $(date +%s)-t0 )) s"
