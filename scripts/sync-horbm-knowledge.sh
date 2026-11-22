#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
#
# Copy the galion-knowledge core card pack into a local folder.
# Public HTTPS git clone. No tokens, no cards committed.
#
# Usage: scripts/sync-horbm-knowledge.sh [TARGET]
#   TARGET defaults to $GALION_HORBM_KNOWLEDGE, or <repo>/var/knowledge.
# Point GALION_HORBM_KNOWLEDGE at TARGET before starting the 7881 server.
# A missing directory falls back to the example kettle card.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TARGET="${1:-${GALION_HORBM_KNOWLEDGE:-$ROOT/var/knowledge}}"
REPO="https://github.com/galion-studio/galion-knowledge.git"
TMP="$(mktemp -d)"
cleanup() { rm -rf "$TMP"; }
trap cleanup EXIT

command -v git >/dev/null || { echo "sync-horbm-knowledge: git is required" >&2; exit 2; }

# Disable autocrlf so card-bundle sha256 matches the manifest on Windows.
echo "clone $REPO"
git clone -c core.autocrlf=false -c core.eol=lf --depth 1 --filter=blob:none --sparse "$REPO" "$TMP/src"
git -c core.autocrlf=false -c core.eol=lf -C "$TMP/src" sparse-checkout set --skip-checks v2/data/cards v2/shards/manifest.json

mkdir -p "$TARGET"
for name in horbm world diary; do
  rm -rf "$TARGET/$name"
  if [ -d "$TMP/src/v2/data/cards/$name" ]; then
    cp -a "$TMP/src/v2/data/cards/$name" "$TARGET/$name"
  fi
done
cp -f "$TMP/src/v2/shards/manifest.json" "$TARGET/manifest.json"

count="$(find "$TARGET" -name '*.json' ! -name 'manifest.json' | wc -l | tr -d ' ')"
echo "cards=$count"
echo "target=$TARGET"
echo "GALION_HORBM_KNOWLEDGE=$TARGET"
