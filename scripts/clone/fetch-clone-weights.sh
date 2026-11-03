#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
#
# V2-S14 · fetch pinned Chatterbox safetensors into weights/clone/ (not vendored).
# weights/.gitignore ignores everything under weights/. CI must not run this.
#
# Default source is the GitHub Release tag clone-weights-v1. Each asset is
# checked against scripts/clone/PINS.sha256 (content sha256 and byte size).
# A mismatch deletes the partial. .pt, audio suffixes, and Samantha or Jarvis
# filenames are refused. Reference clips are never a pin row.
#
#   GALION_CLONE_FETCH=1 scripts/clone/fetch-clone-weights.sh --nano
#   GALION_CLONE_FETCH=1 scripts/clone/fetch-clone-weights.sh
#   GALION_CLONE_FETCH=1 scripts/clone/fetch-clone-weights.sh --check --nano
#
# --nano fetches the four Nano files (shared + t3_nano). The default also
# fetches Turbo. GALION_CLONE_WEIGHTS_SOURCE=hf uses the pinned Hugging Face
# URLs for the same hashes.
#
# Until tag clone-weights-v1 exists, a release fetch exits 1 and prints
# BLOCKED. The upload is docs/galion-voice-v2/CLONE-WEIGHTS.md.
#
# Exit: 0 ok · 1 sha mismatch, missing on --check, or release absent · 2 refused
set -euo pipefail
cd "$(dirname "$0")/../.."
# shellcheck source=weights-release.sh
source "$PWD/scripts/clone/weights-release.sh"

if [ "${GALION_CLONE_FETCH:-}" != 1 ]; then
  echo "fetch-clone-weights: refusing; set GALION_CLONE_FETCH=1 to fetch pinned safetensors. Not run in CI." >&2
  exit 2
fi

CHECK=0
NANO=0
if [ "$#" -gt 2 ]; then
  echo "fetch-clone-weights: refusing extra arguments" >&2
  exit 2
fi
for arg in "$@"; do
  case "$arg" in
    --check) CHECK=1 ;;
    --nano) NANO=1 ;;
    *)
      echo "fetch-clone-weights: refusing extra arguments" >&2
      exit 2
      ;;
  esac
done

PINS="${GALION_CLONE_PINS:-$PWD/scripts/clone/PINS.sha256}"
[ -f "$PINS" ] || { echo "fetch-clone-weights: pins missing" >&2; exit 2; }
W="${GALION_CLONE_WEIGHTS:-$PWD/weights/clone}"
command -v sha256sum >/dev/null || { echo "fetch-clone-weights: sha256sum required" >&2; exit 2; }

SOURCE_KIND="${GALION_CLONE_WEIGHTS_SOURCE:-release}"
case "$SOURCE_KIND" in
  release|hf) ;;
  *)
    echo "fetch-clone-weights: GALION_CLONE_WEIGHTS_SOURCE must be release or hf" >&2
    exit 2
    ;;
esac

TAG="${GALION_CLONE_RELEASE_TAG:-$CLONE_RELEASE_TAG}"
case "$TAG" in
  clone-weights-v[0-9]|clone-weights-v[0-9][0-9]) ;;
  *)
    echo "fetch-clone-weights: refusing tag $TAG" >&2
    exit 2
    ;;
esac

RELEASE_BASE="${GALION_CLONE_RELEASE_BASE:-$CLONE_RELEASE_BASE_DEFAULT}"
RELEASE_BASE="${RELEASE_BASE%/}"
HF_BASE="${GALION_CLONE_HF_BASE:-https://huggingface.co}"
HF_BASE="${HF_BASE%/}"
case "$RELEASE_BASE" in
  http://*|https://*) ;;
  *)
    echo "fetch-clone-weights: release base must be http(s)" >&2
    exit 2
    ;;
esac
case "$HF_BASE" in
  http://*|https://*) ;;
  *)
    echo "fetch-clone-weights: hf base must be http(s)" >&2
    exit 2
    ;;
esac
case "$RELEASE_BASE$HF_BASE" in
  *[[:space:]]*|*'?'*|*'#'*)
    echo "fetch-clone-weights: refusing base url" >&2
    exit 2
    ;;
esac

ok() { [ -f "$1" ] && [ "$(sha256sum "$1" | cut -d' ' -f1)" = "$2" ]; }

refuse_suffix() {
  case "$1" in
    *.pt|*.wav|*.wave|*.mp3|*.flac|*.ogg|*.opus|*.m4a|*.aac|*.mp4|*.mkv|*.mov|*.webm)
      echo "fetch-clone-weights: refusing $1" >&2
      exit 2
      ;;
  esac
}

recorded="$W/PINS.sha256"
mkdir -p "$W"
: > "$recorded"
rc=0
all_rows=0
nano_rows=0

while read -r rel sha size hf_repo hf_path asset extra; do
  case "$rel" in
    '#'*) continue ;;
    "") continue ;;
  esac
  if [ -z "${sha:-}" ] || [ -z "${size:-}" ] || [ -z "${hf_repo:-}" ] || [ -z "${hf_path:-}" ] || [ -z "${asset:-}" ] || [ -n "${extra:-}" ]; then
    echo "fetch-clone-weights: pin row must be rel sha size hf_repo hf_path release_asset ($rel)" >&2
    exit 2
  fi
  refuse_suffix "$rel"
  refuse_suffix "$hf_path"
  refuse_suffix "$asset"
  low=$(printf '%s' "$rel $hf_path $asset" | tr '[:upper:]' '[:lower:]')
  case "$low" in
    *samantha*|*jarvis*)
      echo "fetch-clone-weights: refusing persona reference name in $rel" >&2
      exit 2
      ;;
  esac
  if ! printf '%s' "$sha" | grep -Eq '^[0-9a-f]{64}$'; then
    echo "fetch-clone-weights: bad sha for $rel" >&2
    exit 2
  fi
  case "$size" in
    ''|*[!0-9]*)
      echo "fetch-clone-weights: bad size for $rel" >&2
      exit 2
      ;;
  esac
  if [ "$size" -ge "$CLONE_ASSET_MAX_BYTES" ]; then
    echo "fetch-clone-weights: $rel is $size bytes. A GitHub release asset must be under 2 GiB. Upload the five safetensors as separate assets." >&2
    exit 2
  fi
  if ! expected=$(clone_weight_asset_for "$rel"); then
    echo "fetch-clone-weights: refusing $rel" >&2
    exit 2
  fi
  if [ "$asset" != "$expected" ]; then
    echo "fetch-clone-weights: asset $asset does not match $expected for $rel" >&2
    exit 2
  fi
  case "$hf_repo" in
    ResembleAI/chatterbox-nano|ResembleAI/chatterbox-turbo) ;;
    *)
      echo "fetch-clone-weights: refusing repo $hf_repo" >&2
      exit 2
      ;;
  esac
  base=${rel##*/}
  if [ "$hf_path" != "$base" ]; then
    echo "fetch-clone-weights: refusing hf path $hf_path for $rel" >&2
    exit 2
  fi

  all_rows=$((all_rows + 1))
  if clone_weight_is_nano_rel "$rel"; then
    nano_rows=$((nano_rows + 1))
  fi
  if [ "$NANO" = 1 ] && ! clone_weight_is_nano_rel "$rel"; then
    echo "skip $rel (--nano)"
    continue
  fi

  dest="$W/$rel"
  mkdir -p "$(dirname "$dest")"
  if ok "$dest" "$sha"; then
    echo "ok   $dest"
  elif [ "$CHECK" = 1 ]; then
    echo "MISSING/BAD $dest" >&2
    rc=1
    continue
  else
    command -v curl >/dev/null || { echo "fetch-clone-weights: curl required" >&2; exit 2; }
    case "$SOURCE_KIND" in
      release) url="${RELEASE_BASE}/releases/download/${TAG}/${asset}" ;;
      hf) url="${HF_BASE}/${hf_repo}/resolve/main/${hf_path}" ;;
    esac
    echo "get  $url"
    part="$dest.part"
    http=$(curl -L --retry 3 --retry-delay 2 -o "$part" -w "%{http_code}" "$url" || true)
    http=${http:-000}
    if [ "$http" != "200" ]; then
      rm -f "$part"
      if [ -f "$dest" ] && ! ok "$dest" "$sha"; then
        rm -f "$dest"
      fi
      if [ "$SOURCE_KIND" = release ] && { [ "$http" = "404" ] || [ "$http" = "000" ]; }; then
        echo "fetch-clone-weights: BLOCKED $asset HTTP $http. Tag ${TAG} is not published. One-shot upload: docs/galion-voice-v2/CLONE-WEIGHTS.md" >&2
      else
        echo "fetch-clone-weights: download failed HTTP $http for $url" >&2
      fi
      exit 1
    fi
    if ok "$part" "$sha"; then
      mv "$part" "$dest"
      echo "ok   $dest"
    else
      rm -f "$part"
      if [ -f "$dest" ] && ! ok "$dest" "$sha"; then
        rm -f "$dest"
      fi
      echo "fetch-clone-weights: sha256 MISMATCH for $url" >&2
      exit 1
    fi
  fi
  if [ -f "$dest" ]; then
    got=$(stat -c %s "$dest")
    if [ "$got" != "$size" ]; then
      echo "fetch-clone-weights: size $got want $size for $rel" >&2
      rm -f "$dest"
      exit 1
    fi
    sha256sum "$dest" | awk -v r="$rel" '{print r, $1}' >> "$recorded"
  fi
done < "$PINS"

if [ "$all_rows" -ne 5 ] || [ "$nano_rows" -ne 4 ]; then
  echo "fetch-clone-weights: expected 5 pin rows (4 nano), got $all_rows rows and $nano_rows nano" >&2
  exit 2
fi

if [ "$rc" = 0 ]; then
  if [ "$NANO" = 1 ]; then
    echo "CLONE=PRESENT (nano safetensors only, not vendored). Generate is not run by this script."
  else
    echo "CLONE=PRESENT (safetensors only, not vendored). Generate is not run by this script."
  fi
else
  echo "CLONE=ABSENT (check incomplete)" >&2
fi
exit "$rc"
