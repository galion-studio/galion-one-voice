#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
# Samantha · Galion Voice V2. 432 Hz tuning standard.
#
# Emotion eval run (laptop CI). Thin glue only: it pins the scorer models,
# builds a file list, and calls existing tools:
#   - galion-emotion bench  (tools/cloning-v2/scorers/emotion, Rust + ONNX Runtime)
#   - the Emotion V2 harness `score` (tools/emotion-v2/harness) for the locked gate formula
# It never downloads, never trains, and never reads own-voice audio. Public
# benchmark (CREMA-D, ODbL) or a synthetic/CC0 render tree only.
#
# Usage:
#   scripts/eval/emotion-eval.sh --bench crema-d [--out DIR] [--scorers a,b] [--limit N]
#   scripts/eval/emotion-eval.sh --build-dir DIR [--reference-dir DIR] [--out DIR] [--scorers a,b]
#     DIR/<emotion>/*.wav, emotion = one of the twelve (neutral … disgusted)
#   scripts/eval/emotion-eval.sh --matrix --build-dir DIR --reference-dir REF --speaker-ref S.wav [--out DIR]
#     Gate matrix for every emotion in tools/emotion-v2/eval/emotions.toml (12 or 14 rows):
#     emotion2vec+ (ort; CUDA when present) + galion-wer (vosk; DIR/<emotion>/<clip>.txt) + galion-ecapa.
# Env: GALION_MODELS (default /mnt/g/galion/models)
#      EMOTION_EVAL_DEVICE auto|cpu|cuda (default auto; production stays CPU — this is eval only)
#      EMOTION_EVAL_CUDA_LIBS  extra dir(s) with libcudart/cublas/cudnn for the CUDA build of ONNX Runtime
#      EMOTION_EVAL_PYTHON     python with torch+transformers for `harness` rows (skipped with a reason if unset)
# Exit: 0 all scorers ran, 2 a pinned scorer/model/benchmark is missing (never reported as a pass).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
M="${GALION_MODELS:-/mnt/g/galion/models}"
DEVICE="${EMOTION_EVAL_DEVICE:-auto}"
LOCK="$ROOT/tools/emotion-v2/eval/MODELS.lock"
SCORERS="$ROOT/tools/emotion-v2/eval/scorers.tsv"
BENCH="" BUILD="" REFS="" OUT="" ONLY="" LIMIT=0 MATRIX=0 SPK=""
CONFIG="$ROOT/tools/emotion-v2/eval/emotions.toml"
while [ $# -gt 0 ]; do
  case "$1" in
    --bench) BENCH="$2"; shift 2 ;;
    --build-dir) BUILD="$2"; shift 2 ;;
    --reference-dir) REFS="$2"; shift 2 ;;
    --out) OUT="$2"; shift 2 ;;
    --scorers) ONLY="$2"; shift 2 ;;
    --limit) LIMIT="$2"; shift 2 ;;
    --matrix) MATRIX=1; shift ;;
    --speaker-ref) SPK="$2"; shift 2 ;;
    --config) CONFIG="$2"; shift 2 ;;
    -h|--help) sed -n '2,24p' "$0"; exit 0 ;;
    *) echo "unknown argument $1" >&2; exit 2 ;;
  esac
done
[ -n "$BENCH$BUILD" ] || { echo "need --bench crema-d or --build-dir DIR" >&2; exit 2; }
OUT="${OUT:-${RUNNER_TEMP:-/tmp}/emotion-eval-$(date +%Y%m%d-%H%M%S)}"
mkdir -p "$OUT"
die2() { echo "emotion-eval: NOT MEASURED: $*" | tee -a "$OUT/summary.md" >&2; exit 2; }

# 1. Pinned models/benchmark: every file in MODELS.lock must exist with its sha256.
[ -d "$M" ] || die2 "models root $M is missing"
( cd "$M" && grep -v '^#' "$LOCK" | awk 'NF{print $1"  "$2}' | sha256sum --quiet -c - ) > "$OUT/models-check.log" 2>&1 \
  || { cat "$OUT/models-check.log" >&2; die2 "pinned model files missing or changed under $M (see models-check.log)"; }
echo "models: $(grep -v "^#" "$LOCK" | grep -c .) pinned files verified under $M" | tee "$OUT/models-check.log"

# 2. ONNX Runtime: CUDA build when an NVIDIA GPU answers, CPU build otherwise.
ORT_CPU="$M/emotion/ort/onnxruntime-linux-x64-1.31.1/lib/libonnxruntime.so"
ORT_GPU="$M/emotion/ort/onnxruntime-linux-x64-gpu_cuda13-1.31.1/lib/libonnxruntime.so"
GPU="none"
if [ "$DEVICE" != cpu ] && command -v nvidia-smi >/dev/null 2>&1 && GPU="$(nvidia-smi --query-gpu=name,memory.total,driver_version --format=csv,noheader 2>/dev/null | head -1)" && [ -n "$GPU" ] && [ -f "$ORT_GPU" ]; then
  export ORT_DYLIB_PATH="$ORT_GPU"
  # $M/runtime/cuda13/lib: CUDA 13 libs not in the torch venv (libcurand.so.10 from NVIDIA's nvidia-curand 10.4.5.25 wheel).
  export LD_LIBRARY_PATH="$(dirname "$ORT_GPU"):$M/runtime/cuda13/lib${EMOTION_EVAL_CUDA_LIBS:+:$EMOTION_EVAL_CUDA_LIBS}${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
else
  GPU="${GPU:-none}"; [ "$DEVICE" = cuda ] && die2 "--device cuda requested but no NVIDIA GPU / CUDA ONNX Runtime"
  DEVICE=cpu; export ORT_DYLIB_PATH="$ORT_CPU"
fi
echo "host: $(nproc) CPU threads; GPU: $GPU; ORT: $ORT_DYLIB_PATH; device request: $DEVICE" | tee "$OUT/device.log"

# 3. Build the scorer once (existing crate).
CRATE="$ROOT/tools/cloning-v2/scorers/emotion"
cargo build -q --release --manifest-path "$CRATE/Cargo.toml"
BIN="${CARGO_TARGET_DIR:-$CRATE/target}/release/galion-emotion"

if [ "$MATRIX" = 1 ]; then
  [ -d "$BUILD" ] && [ -d "$REFS" ] && [ -f "$SPK" ] || die2 "--matrix needs --build-dir, --reference-dir and --speaker-ref"
  # Every emotion in the V2 inventory (emotion-v2/emotions.toml) must have a gate row here.
  for id in $(sed -n 's/^id = "\(.*\)"/\1/p' "$ROOT/emotion-v2/emotions.toml"); do
    grep -q "^id = \"$id\"" "$CONFIG" || die2 "emotion $id from emotion-v2/emotions.toml has no row in $CONFIG"
  done
  # Existing scorers: galion-ecapa (ort) and galion-wer (vosk; needs Rust >= 1.92 for ort rc.15).
  cargo build -q --release --manifest-path "$ROOT/tools/cloning-v2/scorers/ecapa/Cargo.toml"
  cargo +stable build -q --release --bin galion-wer --manifest-path "$ROOT/tools/cloning-v2/scorers/quality/galion-quality/Cargo.toml"
  T="${CARGO_TARGET_DIR:-}"
  ECAPA="${T:-$ROOT/tools/cloning-v2/scorers/ecapa/target}/release/galion-ecapa"
  WERB="${T:-$ROOT/tools/cloning-v2/scorers/quality/galion-quality/target}/release/galion-wer"
  mkdir -p "$OUT/wer" "$OUT/ecapa"
  t0=$(date +%s)
  for e in $(sed -n 's/^id = "\(.*\)"/\1/p' "$CONFIG"); do
    d="$BUILD/$e"; ls "$d"/*.wav >/dev/null 2>&1 || { echo "  $e: no clips (UNMEASURED)"; continue; }
    "$WERB" --text-dir "$d" --model "$M/asr/vosk-model-small-en-us-0.15" --lib "$M/asr/vosk-linux-x86_64-0.3.45/libvosk.so" \
      --jobs "$(nproc)" --out "$OUT/wer/$e.json" "$d"/*.wav > "$OUT/wer/$e.log" 2>&1 || echo "  $e: galion-wer failed (see wer/$e.log)"
    "$ECAPA" --ref "$SPK" --model "$M/speaker/ecapa/ecapa.onnx" --threads "$(nproc)" --no-ceiling --out "$OUT/ecapa/$e.json" "$d"/*.wav \
      > "$OUT/ecapa/$e.log" 2>&1 || echo "  $e: galion-ecapa failed (see ecapa/$e.log)"
  done
  echo "wer+ecapa: $(( $(date +%s) - t0 )) s (cpu, $(nproc) threads)" | tee -a "$OUT/device.log"
  # Emotion scorer: A2's galion-emotion12 (PR #187, emotion2vec+ with FunASR's utterance recipe) when
  # EMOTION12_DIR points at its crate (tools/emotion-v2/emotion12); else galion-emotion's own ONNX path.
  SCORES=()
  if [ -n "${EMOTION12_DIR:-}" ]; then
    cargo build -q --release --manifest-path "$EMOTION12_DIR/Cargo.toml"
    E12="${CARGO_TARGET_DIR:-$EMOTION12_DIR/target}/release/galion-emotion12"
    E12M="$OUT/emotion12-models"; mkdir -p "$E12M"; ln -sfn "$M/emotion/emotion2vec-plus-base-onnx" "$E12M/emotion2vec_plus_base_onnx"
    files=(); for e in $(sed -n 's/^id = "\(.*\)"/\1/p' "$CONFIG"); do
      ls "$BUILD/$e"/*.wav >/dev/null 2>&1 || continue   # no clips: UNMEASURED, refs not needed
      for f in "$BUILD/$e"/*.wav "$REFS/$e"/*.wav; do [ -f "$f" ] && files+=("$f"); done
    done
    t1=$(date +%s)
    "$E12" score --emotions "$EMOTION12_DIR/gate-emotions.toml" --models-dir "$E12M" --device "$DEVICE" --threads "$(nproc)" \
      --out "$OUT/emotion12.json" "${files[@]}" > "$OUT/emotion12.log" 2>&1 || die2 "galion-emotion12 failed (see emotion12.log)"
    echo "emotion scorer: galion-emotion12 ($(grep -o '"device": *"[^"]*"' "$OUT/emotion12.json" | head -1)), ${#files[@]} files, $(( $(date +%s) - t1 )) s" | tee -a "$OUT/device.log"
    SCORES=(--scores "$OUT/emotion12.json")
  fi
  set +e
  "$BIN" matrix "${SCORES[@]}" --config "$CONFIG" --build-dir "$BUILD" --reference-dir "$REFS" --models-root "$M" \
    --wer-dir "$OUT/wer" --ecapa-dir "$OUT/ecapa" --device "$DEVICE" --threads 0 --out-dir "$OUT" 2> "$OUT/matrix.log"
  c=$?; set -e
  cat "$OUT/matrix.log"; echo "reports: $OUT"
  exit $c
fi

# 4. File list: path<TAB>emotion.
LIST="$OUT/files.tsv"; : > "$LIST"
if [ -n "$BENCH" ]; then
  [ "$BENCH" = crema-d ] || die2 "unknown benchmark $BENCH (crema-d)"
  D="$M/bench/crema-d/AudioWAV"; [ -d "$D" ] || die2 "CREMA-D not under $D"
  for f in "$D"/*.wav; do
    case "$(basename "$f" | cut -d_ -f3)" in
      ANG) e=angry ;; DIS) e=disgusted ;; FEA) e=fearful ;; HAP) e=happy ;; NEU) e=neutral ;; SAD) e=sad ;; *) continue ;;
    esac
    printf '%s\t%s\n' "$f" "$e" >> "$LIST"
  done
else
  for d in "$BUILD"/*/; do
    e="$(basename "$d" | tr '[:upper:]' '[:lower:]')"
    for f in "$d"*.wav; do [ -f "$f" ] && printf '%s\t%s\n' "$f" "$e" >> "$LIST"; done
  done
fi
[ "$LIMIT" -gt 0 ] && { awk -F'\t' -v n="$LIMIT" 'c[$2]++<n' "$LIST" > "$LIST.tmp" && mv "$LIST.tmp" "$LIST"; }
[ -s "$LIST" ] || die2 "no WAV files to score"
echo "files: $(wc -l < "$LIST") ($(cut -f2 "$LIST" | sort | uniq -c | awk '{printf "%s %s, ", $2, $1}'))" | tee -a "$OUT/device.log"

# 5. Run every scorer row.
{
  echo "# Emotion eval ($( [ -n "$BENCH" ] && echo "benchmark $BENCH" || echo "build $BUILD"))"
  echo; echo "$(cat "$OUT/device.log")"; echo
  echo "| scorer | kind | device | files | accuracy | UAR | not covered | wall s |"
  echo "|---|---|---|---:|---:|---:|---|---:|"
} > "$OUT/summary.md"
rc=0
while IFS=$'\t' read -r name kind model head labels map; do
  case "$name" in ''|'#'*) continue ;; esac
  if [ -n "$ONLY" ] && ! printf ',%s,' "$ONLY" | grep -q ",$name,"; then continue; fi
  if [ "$kind" = ort ]; then
    args=(bench --manifest "$LIST" --name "$name" --ser-model "$M/$model" --device "$DEVICE" --threads 0 --label-map "$map" --out "$OUT/$name.json" --summary-md "$OUT/summary.md")
    [ "$head" != - ] && args+=(--ser-head "$M/$head")
    [ "$labels" != - ] && args+=(--ser-labels "$labels")
    "$BIN" "${args[@]}" 2> "$OUT/$name.log" || { echo "scorer $name failed:"; cat "$OUT/$name.log"; rc=1; continue; }
    cat "$OUT/$name.log"
  elif [ "$kind" = harness ]; then
    PY="${EMOTION_EVAL_PYTHON:-}"
    if [ -z "$PY" ] || ! "$PY" -c 'import torch, transformers' 2>/dev/null; then
      echo "| $name | harness | - | - | - | - | skipped: EMOTION_EVAL_PYTHON with torch+transformers not set | - |" >> "$OUT/summary.md"; continue
    fi
    # Renders vs references per emotion: given --reference-dir, or a split of the file list (odd lines render, even lines reference).
    T="$OUT/$name-tree"; mkdir -p "$T/render" "$T/ref"
    if [ -n "$REFS" ]; then RD="$BUILD"; FD="$REFS"; else
      awk -F'\t' '{n[$2]++; print $1"\t"$2"\t"(n[$2]%2)}' "$LIST" | while IFS=$'\t' read -r f e side; do
        dst="$T/$([ "$side" = 1 ] && echo render || echo ref)/$e"; mkdir -p "$dst"; ln -sf "$f" "$dst/"; done
      RD="$T/render"; FD="$T/ref"; fi
    W="$M/emotion/superb-er/hf-441a7599"
    t0=$(date +%s)
    emo=(); for e in $(ls "$RD"); do [ -d "$FD/$e" ] && emo+=(--emotion "$e"); done
    set +e
    EMOTION_V2_SUPERB_WEIGHTS="$W" HF_HUB_OFFLINE=1 TRANSFORMERS_OFFLINE=1 PYTHONPATH="$ROOT/tools/emotion-v2" \
      "$PY" -m harness score --backend "$model" --render-dir "$RD" --reference-dir "$FD" --out-dir "$OUT/$name" "${emo[@]}" > "$OUT/$name.log" 2>&1
    c=$?; set -e
    [ "$c" -le 1 ] || { echo "harness $name failed (exit $c):"; tail -20 "$OUT/$name.log"; rc=1; continue; }
    echo "| $name | harness (gate 0.83, exit $c) | cpu (torch) | $(wc -l < "$LIST") | see $name/summary.md | - | - | $(( $(date +%s) - t0 )) |" >> "$OUT/summary.md"
  fi
done < "$SCORERS"
echo; cat "$OUT/summary.md"
echo "reports: $OUT"
exit $rc
