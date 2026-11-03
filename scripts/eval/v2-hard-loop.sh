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
# HARD-PASS is printed only when every real gate is met: Chatterbox Nano
# PCM (engine chatterbox-nano or nano), 24 kHz / 80 ms sampling with
# allocs=0 and late=pause, a real model (Hermes or student .gq), a thought
# trace, reference clips plus live emotion and Jarvis hub texture, and the
# measured Vosk → floor → Nano → stream → EmotionClock → playback chain.
# GALION_CLONE_STUB_PCM and GALION_CLONE_STUB_FRAMES never yield HARD-PASS.
# GALION_CLONE_WEIGHTS_SOURCE=hf stays the Hugging Face fetch path. This
# script does not call it.
# This script does not download weights, does not commit WAV or safetensors,
# and does not read or edit LOCKS.toml.
set -euo pipefail
cd "$(dirname "$0")/../.."
exec python3 scripts/eval/v2_hard_loop.py "$@"
