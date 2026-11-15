#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
#
# A1 prep on the machine that will speak. Stages a CPU venv and the five
# pinned tokenizer text files. Does not fetch safetensors, does not edit
# REFS.sha256 or LOCKS.toml, does not write PCM or WAV, and does not call
# generate. CI must not set GALION_CLONE_PREP.
#
#   scripts/clone/stage-clone-prep.sh --check
#   GALION_CLONE_TOKENIZER_SRC=/path/to/ckpt-nano scripts/clone/stage-clone-prep.sh --tokenizer
#   GALION_CLONE_PREP=1 scripts/clone/stage-clone-prep.sh --venv
#
# --check prints venv and tokenizer status and does not download.
# Exit 0 when both are ready. Exit 1 when either is BLOCKED (absent).
# Exit 2 when a present file fails its pin, or the invocation is refused.
#
# --tokenizer copies vocab.json, merges.txt, added_tokens.json,
# special_tokens_map.json, and tokenizer_config.json into
# weights/clone/tokenizer when each source file matches TOKENIZER.sha256.
# A missing source directory or file is BLOCKED (exit 1). A hash or size
# miss leaves the destination unchanged and exits 2. Audio, .pt, and
# safetensors are never copied.
#
# The default source is the operator tree named in the Swarm 10 A1 brief:
# /workspace/galion-symphony-v2/clone-env/ckpt-nano
# GALION_CLONE_TOKENIZER_SRC overrides it. Files may sit in that directory
# or in a tokenizer/ child. This script does not download them.
#
# --venv creates .venv-clone with the CPU torch index, then the chatterbox
# git install from scripts/clone/requirements.txt. It needs GALION_CLONE_PREP=1.
set -euo pipefail
cd "$(dirname "$0")/../.."

PINS="${GALION_CLONE_TOKENIZER_PINS:-$PWD/scripts/clone/TOKENIZER.sha256}"
WEIGHTS="${GALION_CLONE_WEIGHTS:-$PWD/weights/clone}"
DEST="${WEIGHTS}/tokenizer"
VENV="${GALION_CLONE_VENV:-$PWD/.venv-clone}"
DEFAULT_SRC="/workspace/galion-symphony-v2/clone-env/ckpt-nano"
SRC="${GALION_CLONE_TOKENIZER_SRC:-$DEFAULT_SRC}"
REFS_FILE="$PWD/scripts/clone/REFS.sha256"
LOCKS_FILE="$PWD/LOCKS.toml"

usage() {
  echo "stage-clone-prep: use one of --check, --tokenizer, --venv" >&2
  exit 2
}

if [ "$#" -ne 1 ]; then
  usage
fi
MODE="$1"
case "$MODE" in
  --check|--tokenizer|--venv) ;;
  *) usage ;;
esac

[ -f "$PINS" ] || { echo "stage-clone-prep: tokenizer pins missing" >&2; exit 2; }
command -v sha256sum >/dev/null || { echo "stage-clone-prep: sha256sum required" >&2; exit 2; }

lock_digest() {
  if [ -f "$1" ]; then
    sha256sum "$1" | awk '{print $1}'
  else
    echo missing
  fi
}

REFS_BEFORE="$(lock_digest "$REFS_FILE")"
LOCKS_BEFORE="$(lock_digest "$LOCKS_FILE")"

guard_locks() {
  local refs_after locks_after
  refs_after="$(lock_digest "$REFS_FILE")"
  locks_after="$(lock_digest "$LOCKS_FILE")"
  if [ "$refs_after" != "$REFS_BEFORE" ] || [ "$locks_after" != "$LOCKS_BEFORE" ]; then
    trap - EXIT
    echo "stage-clone-prep: refusing; REFS.sha256 or LOCKS.toml changed during staging" >&2
    exit 2
  fi
}
trap guard_locks EXIT

# rel sha size, comments and blank lines skipped. Extra columns are ignored.
mapfile -t PIN_ROWS < <(awk 'NF && $1 !~ /^#/ { print $1, $2, $3 }' "$PINS")
if [ "${#PIN_ROWS[@]}" -ne 5 ]; then
  echo "stage-clone-prep: TOKENIZER.sha256 must list exactly five files (got ${#PIN_ROWS[@]})" >&2
  exit 2
fi

refuse_name() {
  local name="$1"
  case "$name" in
    *.wav|*.wave|*.mp3|*.flac|*.ogg|*.opus|*.m4a|*.aac|*.pt|*.ckpt|*.safetensors|*.bin|*.onnx|*.gguf)
      echo "stage-clone-prep: refusing $name" >&2
      exit 2
      ;;
  esac
  case "$(printf '%s' "$name" | tr '[:upper:]' '[:lower:]')" in
    *samantha*|*jarvis*)
      echo "stage-clone-prep: refusing persona audio name $name" >&2
      exit 2
      ;;
  esac
}

file_sha() {
  sha256sum "$1" | awk '{print $1}'
}

file_size() {
  stat -c %s "$1"
}

# Resolve one pin basename inside the operator source. Flat snapshot or tokenizer/ child.
resolve_src() {
  local base="$1"
  local flat="$SRC/$base"
  local nested="$SRC/tokenizer/$base"
  if [ -f "$flat" ] && [ -f "$nested" ]; then
    if [ "$(file_sha "$flat")" != "$(file_sha "$nested")" ]; then
      echo "stage-clone-prep: $base disagrees between $flat and $nested" >&2
      return 2
    fi
  fi
  if [ -f "$nested" ]; then
    printf '%s\n' "$nested"
    return 0
  fi
  if [ -f "$flat" ]; then
    printf '%s\n' "$flat"
    return 0
  fi
  return 1
}

tokenizer_dest_status() {
  local ready=0
  local missing=0
  local bad=0
  local row rel sha size base path
  for row in "${PIN_ROWS[@]}"; do
    rel="${row%% *}"
    sha="$(printf '%s\n' "$row" | awk '{print $2}')"
    size="$(printf '%s\n' "$row" | awk '{print $3}')"
    base="$(basename "$rel")"
    refuse_name "$base"
    if ! printf '%s' "$sha" | grep -Eq '^[0-9a-f]{64}$'; then
      echo "stage-clone-prep: bad sha for $rel" >&2
      exit 2
    fi
    case "$size" in
      ''|*[!0-9]*)
        echo "stage-clone-prep: bad size for $rel" >&2
        exit 2
        ;;
    esac
    path="$DEST/$base"
    if [ ! -f "$path" ]; then
      missing=$((missing + 1))
      continue
    fi
    if [ "$(file_size "$path")" = "$size" ] && [ "$(file_sha "$path")" = "$sha" ]; then
      ready=$((ready + 1))
    else
      bad=$((bad + 1))
    fi
  done
  if [ "$bad" -gt 0 ]; then
    echo "mismatch"
  elif [ "$ready" -eq 5 ]; then
    echo "ready"
  else
    echo "absent"
  fi
}

venv_status() {
  local py="$VENV/bin/python"
  if [ ! -x "$py" ]; then
    echo "absent"
    return 0
  fi
  local out rc=0
  set +e
  out="$("$py" - 2>&1 <<'PY'
import inspect, sys
try:
    import torch
except Exception as exc:
    print(f"torch-import-failed {type(exc).__name__}")
    sys.exit(1)
cuda = getattr(torch.version, "cuda", None)
print(f"torch={torch.__version__} cuda_build={cuda}")
if cuda not in (None, ""):
    sys.exit(2)
try:
    from chatterbox.tts_turbo import ChatterboxTurboTTS
except Exception as exc:
    print(f"chatterbox-import-failed {type(exc).__name__}")
    sys.exit(1)
try:
    params = inspect.signature(ChatterboxTurboTTS.from_local).parameters
except (TypeError, ValueError):
    params = {}
print("from_local=" + ",".join(params))
if "nano" not in params:
    sys.exit(3)
sys.exit(0)
PY
)"
  rc=$?
  set -e
  printf '%s\n' "$out" >&2
  case "$rc" in
    0) echo "ready" ;;
    2) echo "not-cpu" ;;
    3) echo "not-nano" ;;
    *) echo "absent" ;;
  esac
}

cmd_check() {
  local tok venv
  tok="$(tokenizer_dest_status)"
  venv="$(venv_status)"
  echo "stage-clone-prep: tokenizer=$tok dest=$DEST"
  echo "stage-clone-prep: venv=$venv path=$VENV"
  echo "stage-clone-prep: pcm=not-run"
  echo "stage-clone-prep: refs_sha256=$REFS_BEFORE locks_sha256=$LOCKS_BEFORE"
  if [ "$tok" = "mismatch" ] || [ "$venv" = "not-cpu" ] || [ "$venv" = "not-nano" ]; then
    exit 2
  fi
  if [ "$tok" = "ready" ] && [ "$venv" = "ready" ]; then
    exit 0
  fi
  echo "stage-clone-prep: BLOCKED" >&2
  exit 1
}

cmd_tokenizer() {
  if [ ! -d "$SRC" ]; then
    echo "stage-clone-prep: BLOCKED tokenizer source absent: $SRC" >&2
    echo "stage-clone-prep: pcm=not-run"
    exit 1
  fi
  local stage row rel sha size base src path
  stage="$(mktemp -d)"
  for row in "${PIN_ROWS[@]}"; do
    rel="${row%% *}"
    sha="$(printf '%s\n' "$row" | awk '{print $2}')"
    size="$(printf '%s\n' "$row" | awk '{print $3}')"
    base="$(basename "$rel")"
    refuse_name "$base"
    local src_rc=0
    set +e
    src="$(resolve_src "$base")"
    src_rc=$?
    set -e
    if [ "$src_rc" -eq 2 ]; then
      rm -rf "$stage"
      exit 2
    fi
    if [ "$src_rc" -ne 0 ] || [ -z "$src" ]; then
      rm -rf "$stage"
      echo "stage-clone-prep: BLOCKED tokenizer file absent: $base (source $SRC)" >&2
      echo "stage-clone-prep: pcm=not-run"
      exit 1
    fi
    refuse_name "$(basename "$src")"
    if [ "$(file_size "$src")" != "$size" ] || [ "$(file_sha "$src")" != "$sha" ]; then
      rm -rf "$stage"
      echo "stage-clone-prep: pin mismatch for $base (destination unchanged)" >&2
      exit 2
    fi
    cp -f "$src" "$stage/$base"
    if [ "$(file_size "$stage/$base")" != "$size" ] || [ "$(file_sha "$stage/$base")" != "$sha" ]; then
      rm -rf "$stage"
      echo "stage-clone-prep: copy mismatch for $base (destination unchanged)" >&2
      exit 2
    fi
  done
  mkdir -p "$DEST"
  for row in "${PIN_ROWS[@]}"; do
    rel="${row%% *}"
    base="$(basename "$rel")"
    # Install via a temp name in DEST so a crash cannot leave a truncated pin file.
    cp -f "$stage/$base" "$DEST/$base.staging"
    mv -f "$DEST/$base.staging" "$DEST/$base"
  done
  rm -rf "$stage"
  if [ "$(tokenizer_dest_status)" != "ready" ]; then
    echo "stage-clone-prep: destination failed pin re-check" >&2
    exit 2
  fi
  echo "stage-clone-prep: tokenizer=ready dest=$DEST files=5"
  echo "stage-clone-prep: pcm=not-run"
  echo "stage-clone-prep: refs_sha256=$REFS_BEFORE locks_sha256=$LOCKS_BEFORE"
}

cmd_venv() {
  if [ "${GALION_CLONE_PREP:-}" != 1 ]; then
    echo "stage-clone-prep: refusing venv; set GALION_CLONE_PREP=1. Not run in CI." >&2
    exit 2
  fi
  if [ -e "$VENV" ] && [ ! -d "$VENV" ]; then
    echo "stage-clone-prep: $VENV exists and is not a directory" >&2
    exit 2
  fi
  if ! python3 -c 'import ensurepip' >/dev/null 2>&1; then
    echo "stage-clone-prep: BLOCKED python3 ensurepip is missing. Install the python3-venv package. PCM was not run." >&2
    exit 1
  fi
  python3 -m venv "$VENV"
  "$VENV/bin/python" -m pip install --upgrade pip
  "$VENV/bin/python" -m pip install torch torchaudio --index-url https://download.pytorch.org/whl/cpu
  # Install chatterbox without letting pip replace the CPU torch wheel.
  "$VENV/bin/python" -m pip install 'chatterbox-tts @ git+https://github.com/resemble-ai/chatterbox.git' \
    --extra-index-url https://download.pytorch.org/whl/cpu
  local status
  status="$(venv_status)"
  echo "stage-clone-prep: venv=$status path=$VENV"
  echo "stage-clone-prep: pcm=not-run"
  if [ "$status" != "ready" ]; then
    exit 2
  fi
}

case "$MODE" in
  --check) cmd_check ;;
  --tokenizer) cmd_tokenizer ;;
  --venv) cmd_venv ;;
esac
