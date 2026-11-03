#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
#
# Chatterbox Nano stream gate. Fails if the worker can only return PCM
# with the finished utterance. Does not download weights and does not
# write a WAV. A protocol-stub clock is not a model TTFA.
#
#   scripts/eval/clone-stream-ttfa.sh
set -euo pipefail
cd "$(dirname "$0")/../.."
exec python3 scripts/eval/clone_stream_ttfa.py
