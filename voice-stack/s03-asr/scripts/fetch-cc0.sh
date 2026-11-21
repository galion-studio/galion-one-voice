#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
#
# Download the 12 LJ Speech clips named in eval/cc0-ljspeech.sha256.
# Audio is not committed. A sha256 mismatch deletes the file and exits 1.
set -euo pipefail
cd "$(dirname "$0")/.."
W="$PWD/weights/cc0"
LIST="$PWD/eval/cc0-ljspeech.sha256"
BASE="https://huggingface.co/datasets/flexthink/ljspeech/resolve/main/wavs"
mkdir -p "$W"
command -v curl >/dev/null || { echo "fetch-cc0: curl required"; exit 2; }
sha() { sha256sum "$1" | awk '{print $1}'; }
while read -r sum name; do
  [ -z "${sum:-}" ] && continue
  case "$sum" in \#*) continue ;; esac
  dest="$W/$name"
  if [ -f "$dest" ] && [ "$(sha "$dest")" = "$sum" ]; then
    echo "ok   $dest"
    continue
  fi
  id="${name%.wav}"
  url="$BASE/${id}.wav?download=true"
  echo "get  $url"
  curl -fL --retry 3 -A "galion-s03-asr" -o "$dest.part" "$url"
  if [ "$(sha "$dest.part")" = "$sum" ]; then
    mv "$dest.part" "$dest"
    echo "ok   $dest"
  else
    rm -f "$dest.part"
    echo "fetch-cc0: sha256 mismatch for $name"
    exit 1
  fi
done < "$LIST"
echo "fetch-cc0: done"
