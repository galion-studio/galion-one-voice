#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
#
# Check one reference clip the human already placed. Does not fetch, does not
# record, does not trim, does not write the wav, and does not edit LOCKS.toml.
#
#   GALION_CLIP_STORE=/absolute/path/outside/the/repo scripts/clone/ingest-clip.sh samantha
#   GALION_CLIP_STORE=/absolute/path/outside/the/repo scripts/clone/ingest-clip.sh jarvis
#
# The wav is mono 16-bit PCM, about 10 seconds (8–12). consent.txt beside it
# carries every field from the rights checklist. This script prints duration
# and sha256 so those two fields can be copied from the file. It does not
# invent them.
#
# Exit: 0 ready for the worker (the lock must also be true) · 1 bad clip · 2 refused
set -euo pipefail
cd "$(dirname "$0")/../.."
# Rust consent gate (crates/galion-consent). consent.py is retired to
# tools/legacy/consent.py.bak; no Python runs here.
consent() { cargo run --quiet --offline -p galion-consent -- "$@"; }
persona="${1:-}"
case "$persona" in
  samantha|jarvis) ;;
  *)
    echo "usage: GALION_CLIP_STORE=/absolute/outside/repo $0 samantha|jarvis" >&2
    echo "required consent fields:" >&2
    consent fields >&2
    exit 2
    ;;
esac
if [ -z "${GALION_CLIP_STORE:-}" ]; then
  echo "ingest: GALION_CLIP_STORE is unset. There is no default path and this script does not fetch a wav." >&2
  echo "Place one mono 16-bit PCM wav of about 10 seconds (8–12 s) at \$GALION_CLIP_STORE/$persona/reference.wav" >&2
  echo "required consent fields:" >&2
  consent fields >&2
  exit 2
fi
consent measure --persona "$persona"
echo "--- check ---"
consent check --persona "$persona"
