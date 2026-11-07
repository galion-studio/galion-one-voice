#!/usr/bin/env bash
# PR #86 laptop runbook: Samantha warmth/love, one file.
# One CLAP pass on the RTX 4060, then a CPU emotion2vec+ posterior.
# The two results are written as one blend JSON. Does not merge, push,
# download weights, or commit audio.
#
#   tools/laptop-runbooks/pr86-samantha-warmth.sh \
#     --wav /absolute/path/outside-the-repo.wav \
#     --label samantha-love-window \
#     --role reference

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib/common.sh
source "$HERE/lib/common.sh"

PR_REF="swarm/symphony-b-expression-20261106"
MARKER="tools/cloning-v2/pipeline/expression/Cargo.toml"
PREREQ_ONLY=0
WAV=""
LABEL=""
ROLE="unspecified"

usage() {
  cat <<'EOF'
usage: tools/laptop-runbooks/pr86-samantha-warmth.sh --wav PATH [--label NAME] [--role reference|clone|unspecified] [--prereq-only]

One wav, outside the git repo. CLAP runs once on the RTX 4060 (8 GB).
emotion2vec+ then runs on CPUExecutionProvider. The blend is written to
${GALION_LAPTOP_OUT:-${XDG_CACHE_HOME:-$HOME/.cache}/galion-laptop-runbooks}/pr86-samantha-warmth.json

The PR checkout is GALION_PR86_ROOT, this repo when the expression crate is
already present, or a detached worktree of origin/swarm/symphony-b-expression-20261106.
Nothing is merged and nothing is pushed. Weights are not downloaded.
The committed love-gate fixture is only read.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --prereq-only) PREREQ_ONLY=1; shift ;;
    --wav) WAV="${2:-}"; shift 2 ;;
    --label) LABEL="${2:-}"; shift 2 ;;
    --role) ROLE="${2:-}"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown argument: $1" >&2; usage >&2; exit 2 ;;
  esac
done

case "$ROLE" in
  reference|clone|unspecified) ;;
  *) echo "--role must be reference, clone, or unspecified" >&2; exit 2 ;;
esac

REPO="$(laptop_repo_root)"
OUT="$(laptop_out_dir "$REPO")"

laptop_require_cmd python3
laptop_require_cmd cargo
laptop_require_cmd git
laptop_require_cmd nvidia-smi

if [[ -z "$WAV" ]]; then
  echo "--wav is required (a local file outside the repo)" >&2
  usage >&2
  exit 2
fi
WAV="$(laptop_abs_outside_repo "$REPO" "$WAV" "wav")"
case "$WAV" in
  *.wav) ;;
  *) echo "wav must end in .wav: $WAV" >&2; exit 2 ;;
esac
if [[ -z "$LABEL" ]]; then
  LABEL="$(basename "$WAV" .wav)"
fi
if [[ ! "$LABEL" =~ ^[A-Za-z0-9._-]+$ ]]; then
  echo "--label must be letters, digits, dot, underscore, or hyphen" >&2
  exit 2
fi

TREE="$(laptop_resolve_tree "$REPO" "$OUT" GALION_PR86_ROOT "$PR_REF" "$MARKER" pr86)"
CLONE="$TREE/tools/cloning-v2"
EXPR="$CLONE/pipeline/expression"
PERSONA="$CLONE/pipeline/personas/own-samantha.toml"
FIXTURE="$EXPR/fixtures/samantha_love_gate.json"
CLAP_DIR="$CLONE/scorers/universal/clap/laion-clap-htsat-fused"
E2V_ONNX="$CLONE/scorers/universal/emotion2vec/emotion2vec_plus_base.onnx"
E2V_HEAD="$CLONE/scorers/universal/emotion2vec/emotion2vec_head.json"
BIN="$EXPR/target/release/galion-expression"

for path in "$PERSONA" "$FIXTURE" "$EXPR/py/eval_obs.py" "$CLONE/scorers/persona/py/score_persona.py"; do
  if [[ ! -f "$path" ]]; then
    echo "missing $path" >&2
    exit 2
  fi
done
if [[ ! -s "$E2V_ONNX" || ! -s "$E2V_HEAD" ]]; then
  echo "emotion2vec+ ONNX weights are not local at $E2V_ONNX" >&2
  echo "this script does not download weights" >&2
  exit 2
fi
if [[ ! -f "$CLAP_DIR/config.json" && ! -f "$CLAP_DIR/preprocessor_config.json" ]]; then
  echo "CLAP weights are not local at $CLAP_DIR" >&2
  echo "this script does not download weights" >&2
  exit 2
fi

python3 - <<'PY'
import importlib
import sys
missing = []
for name in ("numpy", "soundfile", "onnxruntime", "torch", "transformers"):
    try:
        importlib.import_module(name)
    except Exception:
        missing.append(name)
if missing:
    print("missing python modules: " + ", ".join(missing), file=sys.stderr)
    print("install them in the laptop environment; this script does not pip install", file=sys.stderr)
    sys.exit(2)
PY

GPU_LINE="$(
  python3 - <<'PY'
import os, subprocess, sys
raw = subprocess.check_output(
    ["nvidia-smi", "--query-gpu=index,name,memory.total", "--format=csv,noheader,nounits"],
    text=True,
)
allow = os.environ.get("GALION_ALLOW_OTHER_GPU") == "1"
chosen = None
for line in raw.splitlines():
    if not line.strip():
        continue
    idx, name, mem = [part.strip() for part in line.split(",", 2)]
    mem_i = int(float(mem))
    if "4060" in name and mem_i >= 7000:
        chosen = (idx, name, mem_i)
        break
    if allow and mem_i >= 7000 and chosen is None:
        chosen = (idx, name, mem_i)
if chosen is None:
    print("no RTX 4060 with at least 7000 MiB reported", file=sys.stderr)
    print(raw, file=sys.stderr)
    print("GALION_ALLOW_OTHER_GPU=1 accepts another card of at least 7000 MiB", file=sys.stderr)
    sys.exit(2)
print(f"{chosen[0]}\t{chosen[1]}\t{chosen[2]}")
PY
)"
IFS=$'\t' read -r GPU_INDEX GPU_NAME GPU_MIB <<<"$GPU_LINE"

WORK="$OUT/pr86/work/$LABEL"
RESULT="$OUT/pr86-samantha-warmth.json"
mkdir -p "$WORK"
case "$WORK" in
  "$REPO"|"$REPO"/*|"$TREE"|"$TREE"/*)
    echo "work directory must stay outside the repo ($WORK)" >&2
    exit 2
    ;;
esac

echo "prereq ok"
echo "  pr: 86"
echo "  tree: $TREE"
echo "  wav_basename: $(basename "$WAV")"
echo "  label: $LABEL"
echo "  role: $ROLE"
echo "  gpu: index=$GPU_INDEX name=$GPU_NAME memory_mib=$GPU_MIB"
echo "  clap: one file, device=cuda, gpu_mem_gb=8, model=laion/clap-htsat-fused"
echo "  emotion2vec: CPUExecutionProvider, emotion2vec_plus_base"
echo "  json: $RESULT"
echo "  fixture is read-only: pipeline/expression/fixtures/samantha_love_gate.json"

if [[ "$PREREQ_ONLY" -eq 1 ]]; then
  echo "prereq-only: CLAP and emotion2vec+ were not started"
  exit 0
fi

BEFORE="$OUT/pr86-audio-before.txt"
AFTER="$OUT/pr86-audio-after.txt"
laptop_audio_snapshot "$REPO" "$BEFORE"

( cd "$EXPR" && cargo build --release --locked )

(
  cd "$CLONE"
  CUDA_VISIBLE_DEVICES="$GPU_INDEX" \
  GALION_CLAP_DEVICE=cuda \
  "$BIN" eval \
    --wav "$WAV" \
    --voice own-samantha \
    --emotion love \
    --persona "$PERSONA" \
    --case "$FIXTURE" \
    --out-dir "$WORK" \
    --score --love-warmth \
    --device cuda --gpu-mem-gb 8
)

# CLAP has exited. emotion2vec+ is a second process and stays on CPU.
(
  cd "$CLONE"
  env -u GALION_CLAP_DEVICE -u CUDA_VISIBLE_DEVICES \
    python3 scorers/persona/py/score_persona.py \
      --persona "$PERSONA" \
      --skip-clap --skip-efficientat \
      --threads 2 \
      --out "$WORK/e2v-cpu.json" \
      "$WAV"
)

python3 "$HERE/lib/blend_warmth.py" \
  --wav "$WAV" \
  --label "$LABEL" \
  --role "$ROLE" \
  --clap "$WORK/clap_observation.json" \
  --profile "$WORK/profile.json" \
  --e2v "$WORK/e2v-cpu.json" \
  --diagnostic "$WORK/diagnostic.json" \
  --out "$RESULT"

laptop_audio_snapshot "$REPO" "$AFTER"
laptop_audio_unchanged "$BEFORE" "$AFTER"

echo "wav was not copied into the repo"
echo "json: $RESULT"
