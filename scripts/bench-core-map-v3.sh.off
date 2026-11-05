#!/usr/bin/env bash
# Core-map v3 A/B (bench only, not shipped). Run under the bench lock:
#   flock latency-240/BENCH.lock scripts/bench-core-map-v3.sh
# Needs: cargo build --release -p galion-cores; cargo build --release -p galion-ml-voice --features speech --example core_map_bench
set -euo pipefail
# Paths: GALION_BENCH_DIR holds llm-local/ (llama.cpp b12076 + the Qwen2.5-1.5B gguf);
# GALION_REPO is a checkout with weights/ fetched (piper-rt, tts).
W=${GALION_BENCH_DIR:?set GALION_BENCH_DIR}
R=${GALION_REPO:?set GALION_REPO}
T=${CARGO_TARGET_DIR:-$R/target}/release
OUT=${OUT:-$W/results/core-map-v3}
mkdir -p "$OUT"
LL=$W/llm-local/llama-b12076
export LD_LIBRARY_PATH=$LL${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}
export GALION_A21_RUNTIME=$R/weights/runtime/piper-rt/galion-piper-rt
export GALION_PIPER_RUNTIME_DIR=$R/weights/runtime
export GALION_TTS_WEIGHTS=$R/weights/tts/en_US-ljspeech-medium.onnx
export BENCH_LLM_URL=http://127.0.0.1:18080
export BENCH_REPS=${BENCH_REPS:-12}
LPID=
cleanup() { if [ -n "$LPID" ]; then kill "$LPID" 2>/dev/null; wait "$LPID" 2>/dev/null; fi; return 0; }
trap cleanup EXIT
run_cfg() {
  local label=$1; shift
  echo "== $label $* (load $(cut -d' ' -f1 /proc/loadavg))" >&2
  env "$@" $T/galion-pin llm -- $LL/llama-server -m $W/llm-local/qwen2.5-1.5b-instruct-q4_k_m.gguf \
      -t '{threads}' -c 2048 --host 127.0.0.1 --port 18080 --no-webui >"$OUT/llm-$label.log" 2>&1 &
  LPID=$!
  for _ in $(seq 1 120); do curl -sf -m 1 http://127.0.0.1:18080/health >/dev/null 2>&1 && break; sleep 0.25; done
  echo "llama cpus: $(grep Cpus_allowed_list /proc/$(pgrep -P $LPID -f llama-server || echo $LPID)/status 2>/dev/null | cut -f2)" >&2
  env "$@" BENCH_LABEL=$label $T/examples/core_map_bench | tee -a "$OUT/bench.ndjson"
  grep -m1 "galion-pin:" "$OUT/llm-$label.log" >&2 || true
  kill $LPID; wait $LPID 2>/dev/null || true; LPID=
}
CFGS=${CFGS:-"off v2 v3 timeshare v3split v3overlap"}
for c in $CFGS; do
  case $c in
    off) run_cfg off GALION_CORE_MAP=off GALION_PIN_DEFAULT_THREADS=4 ;;
    v2) run_cfg v2 GALION_CORE_MAP=v2 ;;
    v3) run_cfg v3 GALION_CORE_MAP=v3 ;;
    v3split) run_cfg v3split GALION_CORE_MAP=v3 GALION_CORES_LLM=2-4 GALION_CORES_PIPER=5-7 ;;
    v3overlap) run_cfg v3overlap GALION_CORE_MAP=v3 GALION_CORES_LLM=2-5 GALION_CORES_PIPER=4-7 ;;
    v3p2) run_cfg v3p2 GALION_CORE_MAP=v3 GALION_CORES_LLM=2-5 GALION_CORES_PIPER=6-7 ;;
    v3l14) run_cfg v3l14 GALION_CORE_MAP=v3 GALION_CORES_LLM=1-4 GALION_CORES_PIPER=5-7 ;;
    timeshare) run_cfg timeshare GALION_CORE_MAP=v3 GALION_CORES_LLM=4-7 GALION_CORES_PIPER=4-7 ;;
    timeshare-nospin) run_cfg timeshare-nospin GALION_CORE_MAP=v3 GALION_CORES_LLM=4-7 GALION_CORES_PIPER=4-7 GALION_PIPER_SPIN=0 ;;
  esac
done
