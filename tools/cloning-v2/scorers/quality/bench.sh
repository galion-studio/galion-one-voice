#!/usr/bin/env bash
# Back-to-back Python vs Rust on the 24 own-clone test lines (nice 10, 2 threads). Writes out/bench.tsv.
set -u; cd "$(dirname "$0")"
S=/workspace/swarm/voice-v2-sprint; B=bin; PY=$S/venv/bin/python; T=$S/own-clone-analysis/text
SAM=$(ls $S/own-clone-analysis/wav/samantha/*.wav); JAR=$(ls $S/own-clone-analysis/wav/jarvis/*.wav)
export OMP_NUM_THREADS=2 MKL_NUM_THREADS=2
TIMEFORMAT=$'%R\t%U\t%S'
echo -e "task\timpl\twall_s\tuser_s\tsys_s" > out/bench.tsv
run(){ local task=$1 impl=$2; shift 2; local t; t=$( { time nice -n 10 "$@" >/dev/null 2>&1; } 2>&1 ); echo -e "$task\t$impl\t$t" | tee -a out/bench.tsv; }
run hygiene python python3 py/hyg_ref.py out/bench_py_hyg.json $SAM $JAR
run hygiene ffmpeg-r128 bash -c "for f in $(echo $SAM $JAR); do ffmpeg -nostats -hide_banner -i \$f -af ebur128=peak=true -f null - ; done"
run hygiene rust $B/galion-hygiene --out out/bench_rs_hyg.json $SAM $JAR
run wer python $PY py/wer_ref.py out/bench_py_wer.json $T $SAM $JAR
run wer rust $B/galion-wer --text-dir $T --out out/bench_rs_wer.json $SAM $JAR
run wer rust-jobs2 $B/galion-wer --jobs 2 --text-dir $T --out out/bench_rs_wer_j2.json $SAM $JAR
run mos python $PY py/squim_ref.py out/bench_py_squim.json
run mos rust bash -c "$B/galion-mos --nmr $S/clip-store-own/jarvis/reference.wav --out out/bench_rs_mos_sam.json $(echo $SAM) $S/clip-store-own/samantha/reference.wav && $B/galion-mos --nmr $S/clip-store-own/samantha/reference.wav --out out/bench_rs_mos_jar.json $(echo $JAR) $S/clip-store-own/jarvis/reference.wav"
