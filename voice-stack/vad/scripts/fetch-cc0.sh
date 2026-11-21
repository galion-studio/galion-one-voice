#!/bin/sh
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
# Download the CC0 speech clips and the MIT Silero VAD model. Nothing is committed.
set -eu
cd "$(dirname "$0")/.."
src="scripts/sources.tsv"
ua="galion-sam-vad/0.1 (local VAD research; CC0 speech)"
while IFS="$(printf '\t')" read -r dest url sha lic note; do
    case "$dest" in
        ""|\#*) continue ;;
    esac
    mkdir -p "$(dirname "$dest")"
    if [ -f "$dest" ]; then
        got="$(sha256sum "$dest" | awk '{print $1}')"
        if [ "$got" = "$sha" ]; then
            echo "ok  $dest"
            continue
        fi
        echo "redo $dest (sha256 mismatch)"
    fi
    echo "get $dest"
    curl -fsSL -A "$ua" -o "$dest" "$url"
    got="$(sha256sum "$dest" | awk '{print $1}')"
    if [ "$got" != "$sha" ]; then
        echo "sha256 mismatch for $dest" >&2
        echo "  expected $sha" >&2
        echo "  got      $got" >&2
        exit 1
    fi
    echo "ok  $dest  $lic"
done < "$src"
echo "ready. audio stays in samples/ and the model in models/. Both are gitignored."
