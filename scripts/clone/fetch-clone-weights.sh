#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
#
# V2-S14 · fetch pinned Chatterbox safetensors into weights/clone/ (not vendored).
# weights/.gitignore ignores everything under weights/. CI must not run this.
#
# Refuses unless GALION_CLONE_FETCH=1. That includes --check. No reference
# audio, no .pt, no film clip. A sha256 mismatch deletes the partial file.
#
#   GALION_CLONE_FETCH=1 scripts/clone/fetch-clone-weights.sh
#   GALION_CLONE_FETCH=1 scripts/clone/fetch-clone-weights.sh --check
#
# Exit: 0 ok · 1 sha mismatch or missing on --check · 2 refused
set -euo pipefail
cd "$(dirname "$0")/../.."
if [ "${GALION_CLONE_FETCH:-}" != 1 ]; then
  echo "fetch-clone-weights: refusing; set GALION_CLONE_FETCH=1 to fetch pinned safetensors. Not run in CI." >&2
  exit 2
fi
if [ "$#" -gt 1 ] || { [ "$#" -eq 1 ] && [ "${1:-}" != "--check" ]; }; then
  echo "fetch-clone-weights: refusing extra arguments" >&2
  exit 2
fi
CHECK=0
[ "${1:-}" = "--check" ] && CHECK=1
PINS="$PWD/scripts/clone/PINS.sha256"
[ -f "$PINS" ] || { echo "fetch-clone-weights: pins missing" >&2; exit 2; }
W="$PWD/weights/clone"
command -v sha256sum >/dev/null || { echo "fetch-clone-weights: sha256sum required" >&2; exit 2; }
ok() { [ -f "$1" ] && [ "$(sha256sum "$1" | cut -d' ' -f1)" = "$2" ]; }
recorded="$W/PINS.sha256"
mkdir -p "$W"
: > "$recorded"
rc=0
while read -r rel sha size repo remote; do
  case "$rel" in
    '#'*) continue ;;
    "") continue ;;
  esac
  case "$rel" in
    *.pt|*.wav|*.mp3|*.flac|*.mp4|*.mkv|*.mov) echo "fetch-clone-weights: refusing $rel" >&2; exit 2 ;;
  esac
  case "$remote" in
    *.pt|*.wav|*.mp3|*.flac|*.mp4|*.mkv|*.mov) echo "fetch-clone-weights: refusing $remote" >&2; exit 2 ;;
  esac
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
    url="https://huggingface.co/${repo}/resolve/main/${remote}"
    echo "get  $url"
    curl -fsSL --retry 3 -o "$dest.part" "$url"
    if ok "$dest.part" "$sha"; then
      mv "$dest.part" "$dest"
      echo "ok   $dest"
    else
      rm -f "$dest.part"
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
if [ "$rc" = 0 ]; then
  echo "CLONE=PRESENT (safetensors only, not vendored). Generate is not run by this script."
else
  echo "CLONE=ABSENT (check incomplete)" >&2
fi
exit "$rc"
