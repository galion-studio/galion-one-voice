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
# HARD-PASS is printed only when every real CPU gate on this box is met.
# The mouth is Piper (en_US-ljspeech-medium), not a clone stub, not flite,
# and not a cloud voice. A missing component is HARD-FAIL and is named.
# GALION_CLONE_STUB_PCM and GALION_CLONE_STUB_FRAMES never yield HARD-PASS.
# This script does not download weights and does not call a remote model.
# This script does not download weights, does not commit WAV or safetensors,
# and does not read or edit LOCKS.toml.
set -euo pipefail
cd "$(dirname "$0")/../.."
exec python3 scripts/eval/v2_hard_loop.py "$@"
