#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
#
# Hard loop for Galion Voice V2. Replaces a stub SELF-EVAL as the claim.
# scripts/eval/v2-persona-selftest.sh and scripts/eval/flash-ttfa.sh stay
# the stub tools. A green stub line is STUB-PASS. It is not HARD-PASS.
#
#   scripts/eval/v2-hard-loop.sh
#   scripts/eval/v2-hard-loop.sh --self-check
#   GALION_HARD_REQUIRE=1 scripts/eval/v2-hard-loop.sh
#   GALION_CLONE_LOCAL_REFS=1 GALION_CLIP_STORE=/abs/outside/repo \
#       scripts/eval/v2-hard-loop.sh
#   GALION_HARD_HTTP=1 scripts/eval/v2-hard-loop.sh
#
# HARD-PASS is printed only when reference hashes, pinned Nano weights, a
# model generate, and streaming TTFA all met, and the live gates met too.
# This script does not download weights, does not commit WAV or safetensors,
# and does not read or edit LOCKS.toml.
set -euo pipefail
cd "$(dirname "$0")/../.."
exec python3 scripts/eval/v2_hard_loop.py "$@"
