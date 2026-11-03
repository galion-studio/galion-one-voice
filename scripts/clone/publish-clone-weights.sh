#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
#
# One-shot publish of the five pinned Chatterbox safetensors as GitHub
# Release assets on tag clone-weights-v1. Run this on the admin box that
# already has weights/clone/. A CloudAgent checkout does not have the bytes.
#
# Default is verify and print. --upload calls gh. Neither mode commits.
# The asset list is the pin allowlist. Reference audio is refused.
#
#   GALION_CLONE_PUBLISH=1 scripts/clone/publish-clone-weights.sh
#   GALION_CLONE_PUBLISH=1 scripts/clone/publish-clone-weights.sh --upload
#
# Exit: 0 printed or uploaded · 1 pin mismatch · 2 refused or BLOCKED
set -euo pipefail
cd "$(dirname "$0")/../.."
# shellcheck source=weights-release.sh
source "$PWD/scripts/clone/weights-release.sh"

if [ "${GALION_CLONE_PUBLISH:-}" != 1 ]; then
  echo "publish-clone-weights: refusing; set GALION_CLONE_PUBLISH=1 on the admin box. Not run in CI." >&2
  exit 2
fi

UPLOAD=0
if [ "$#" -gt 1 ]; then
  echo "publish-clone-weights: refusing extra arguments" >&2
  exit 2
fi
case "${1:-}" in
  "") UPLOAD=0 ;;
  --upload) UPLOAD=1 ;;
  *)
    echo "publish-clone-weights: refusing extra arguments" >&2
    exit 2
    ;;
esac

if [ "$UPLOAD" = 1 ]; then
  if [ -n "${GALION_CLONE_PINS:-}" ] || [ -n "${GALION_CLONE_WEIGHTS:-}" ] || [ -n "${GALION_CLONE_RELEASE_BASE:-}" ] || [ -n "${GALION_CLONE_WEIGHTS_SOURCE:-}" ] || [ -n "${GALION_CLONE_HF_BASE:-}" ]; then
    echo "publish-clone-weights: --upload refuses path overrides. Use the repo pins and weights/clone." >&2
    exit 2
  fi
fi

PINS="${GALION_CLONE_PINS:-$PWD/scripts/clone/PINS.sha256}"
W="${GALION_CLONE_WEIGHTS:-$PWD/weights/clone}"
FETCH="$PWD/scripts/clone/fetch-clone-weights.sh"

if [ ! -d "$W" ]; then
  echo "publish-clone-weights: BLOCKED. $W is absent. The safetensors are gitignored and are not in this checkout. Run this on the admin box that already holds weights/clone/. See docs/galion-voice-v2/CLONE-WEIGHTS.md" >&2
  exit 2
fi

missing=0
while read -r rel; do
  if [ ! -f "$W/$rel" ]; then
    echo "publish-clone-weights: BLOCKED missing $W/$rel" >&2
    missing=1
  fi
done < <(clone_weight_rels)
if [ "$missing" = 1 ]; then
  exit 2
fi

while IFS= read -r path; do
  rel=${path#"$W"/}
  if [ -L "$path" ]; then
    echo "publish-clone-weights: refusing symlink $rel" >&2
    exit 2
  fi
  if [ -d "$path" ]; then
    case "$rel" in
      shared|nano|turbo) continue ;;
      *)
        echo "publish-clone-weights: refusing directory $rel" >&2
        exit 2
        ;;
    esac
  fi
  case "$rel" in
    *.pt|*.wav|*.wave|*.mp3|*.flac|*.ogg|*.opus|*.m4a|*.aac|*.mp4|*.mkv|*.mov|*.webm|*.part)
      echo "publish-clone-weights: refusing $rel" >&2
      exit 2
      ;;
  esac
  low=$(printf '%s' "$rel" | tr '[:upper:]' '[:lower:]')
  case "$low" in
    *samantha*|*jarvis*)
      echo "publish-clone-weights: refusing persona reference name $rel" >&2
      exit 2
      ;;
  esac
  case "$rel" in
    PINS.sha256) continue ;;
  esac
  if ! clone_weight_asset_for "$rel" >/dev/null; then
    echo "publish-clone-weights: refusing unexpected file $rel" >&2
    exit 2
  fi
done < <(find "$W" -mindepth 1 -print | sort)

set +e
GALION_CLONE_FETCH=1 "$FETCH" --check >&2
check_rc=$?
set -e
if [ "$check_rc" != 0 ]; then
  echo "publish-clone-weights: pin check failed (exit $check_rc). Not uploading." >&2
  exit "$check_rc"
fi

echo "gh release create ${CLONE_RELEASE_TAG} \\"
echo "  --repo ${CLONE_RELEASE_REPO} \\"
echo "  --target main \\"
echo "  --latest=false \\"
echo "  --title 'Clone weights v1' \\"
echo "  --notes-file <notes> \\"
files=()
while read -r rel; do
  asset=$(clone_weight_asset_for "$rel")
  files+=("${W}/${rel}#${asset}")
done < <(clone_weight_rels)
n=${#files[@]}
i=0
for f in "${files[@]}"; do
  i=$((i + 1))
  if [ "$i" -eq "$n" ]; then
    printf "  '%s'\n" "$f"
  else
    printf "  '%s' \\\\\n" "$f"
  fi
done
echo

if [ "$UPLOAD" != 1 ]; then
  echo "publish-clone-weights: verified. Re-run with --upload to publish ${CLONE_RELEASE_TAG}. Weights stay gitignored."
  exit 0
fi

command -v gh >/dev/null || { echo "publish-clone-weights: gh required for --upload" >&2; exit 2; }
if GH_PROMPT_DISABLED=1 gh release view "$CLONE_RELEASE_TAG" --repo "$CLONE_RELEASE_REPO" >/dev/null 2>&1; then
  echo "publish-clone-weights: ${CLONE_RELEASE_TAG} already exists. Do not replace assets. Cut clone-weights-v2." >&2
  exit 2
fi

notes=$(mktemp)
stage=$(mktemp -d)
trap 'rm -rf "$stage"; rm -f "$notes"' EXIT
cat > "$notes" <<EOF
Galion Voice clone weights v1.

Five Chatterbox safetensors. Content sha256 matches scripts/clone/PINS.sha256.
Upstream trees: ResembleAI/chatterbox-nano 71ccd1d0081b430592cea481f4307e764e07bc64 and ResembleAI/chatterbox-turbo 749d1c1a46eb10492095d68fbcf55691ccf137cd.

Reference audio is absent from this release. Samantha and Jarvis clips stay on the admin box, gitignored.

Fetch (CI does not set the flag):

    GALION_CLONE_FETCH=1 scripts/clone/fetch-clone-weights.sh --nano

The tag is immutable. A new hash is clone-weights-v2. This tag is a weight drop. The product release remains v2.0.0.

Signed Maciej Grajczyk.
EOF
{
  echo
  echo "Pins:"
  echo
  grep -E '^[a-z].*\.safetensors ' "$PINS"
} >> "$notes"

args=()
while read -r rel; do
  asset=$(clone_weight_asset_for "$rel")
  if ! ln "$W/$rel" "$stage/$asset" 2>/dev/null; then
    cp "$W/$rel" "$stage/$asset"
  fi
  args+=("$stage/$asset")
done < <(clone_weight_rels)

GH_PROMPT_DISABLED=1 gh release create "$CLONE_RELEASE_TAG" \
  --repo "$CLONE_RELEASE_REPO" \
  --target main \
  --latest=false \
  --title "Clone weights v1" \
  --notes-file "$notes" \
  "${args[@]}"

echo "publish-clone-weights: published ${CLONE_RELEASE_TAG}. The git tag does not contain the blobs. Agents fetch the assets."
