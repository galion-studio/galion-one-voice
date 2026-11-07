#!/usr/bin/env bash
# PR #87 laptop runbook.
# Runs tools/cloning-v2/fidelity/scripts/eval-laptop.sh for amy, alan, long,
# and long-multi. Checks prerequisites first. Writes one JSON summary to a
# fixed cache path. Does not merge, push, or commit audio.
#
#   tools/laptop-runbooks/pr87-fidelity.sh
#   tools/laptop-runbooks/pr87-fidelity.sh --prereq-only

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib/common.sh
source "$HERE/lib/common.sh"

PR_REF="swarm/symphony-a-fidelity-20261106"
MARKER="tools/cloning-v2/fidelity/scripts/eval-laptop.sh"
PREREQ_ONLY=0

usage() {
  cat <<'EOF'
usage: tools/laptop-runbooks/pr87-fidelity.sh [--prereq-only]

Runs PR #87 eval-laptop.sh (amy, alan, long, long-multi) and writes
${GALION_LAPTOP_OUT:-${XDG_CACHE_HOME:-$HOME/.cache}/galion-laptop-runbooks}/pr87-fidelity.json

The PR checkout is GALION_PR87_ROOT, this repo when the script is already
present, or a detached worktree of origin/swarm/symphony-a-fidelity-20261106.
Nothing is merged and nothing is pushed. CC0 audio stays in the cache.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --prereq-only) PREREQ_ONLY=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown argument: $1" >&2; usage >&2; exit 2 ;;
  esac
done

REPO="$(laptop_repo_root)"
OUT="$(laptop_out_dir "$REPO")"
CACHE="$OUT/pr87-cache"
mkdir -p "$CACHE"

laptop_require_cmd python3
laptop_require_cmd cargo
laptop_require_cmd ffmpeg
laptop_require_cmd curl
laptop_require_cmd git

TREE="$(laptop_resolve_tree "$REPO" "$OUT" GALION_PR87_ROOT "$PR_REF" "$MARKER" pr87)"
SCRIPT="$TREE/$MARKER"
if [[ ! -f "$SCRIPT" ]]; then
  echo "missing $SCRIPT" >&2
  exit 2
fi
if [[ ! -f "$TREE/tools/cloning-v2/fidelity/Cargo.toml" ]]; then
  echo "missing fidelity crate under $TREE" >&2
  exit 2
fi
if [[ ! -f "$TREE/tools/cloning-v2/fidelity/py/crosscheck_fidelity.py" ]]; then
  echo "missing crosscheck_fidelity.py under $TREE" >&2
  exit 2
fi

for needle in 'run_pair amy' 'run_pair alan' 'run_pair long ' 'run_pair long-multi'; do
  if ! grep -q "$needle" "$SCRIPT"; then
    echo "eval-laptop.sh is missing '$needle'; refusing to run a different script" >&2
    exit 2
  fi
done

case "$CACHE" in
  "$REPO"|"$REPO"/*|"$TREE"|"$TREE"/*)
    echo "fidelity cache must be outside the repo and the worktree ($CACHE)" >&2
    exit 2
    ;;
esac

echo "prereq ok"
echo "  pr: 87"
echo "  tree: $TREE"
echo "  script: $MARKER"
echo "  clips: amy alan long long-multi"
echo "  cache: $CACHE"
echo "  json: $OUT/pr87-fidelity.json"

if [[ "$PREREQ_ONLY" -eq 1 ]]; then
  echo "prereq-only: eval-laptop.sh was not started"
  exit 0
fi

BEFORE="$OUT/pr87-audio-before.txt"
AFTER="$OUT/pr87-audio-after.txt"
laptop_audio_snapshot "$REPO" "$BEFORE"

export GALION_FIDELITY_CACHE="$CACHE"
bash "$SCRIPT"

python3 "$HERE/lib/collect_fidelity.py" \
  "$CACHE/out" \
  "$OUT/pr87-fidelity.json" \
  "$CACHE"

laptop_audio_snapshot "$REPO" "$AFTER"
laptop_audio_unchanged "$BEFORE" "$AFTER"

echo "audio stays in $CACHE and is not committed"
echo "json: $OUT/pr87-fidelity.json"
