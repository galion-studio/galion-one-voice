#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
#
# Jarvis hub texture self-eval.
# The layer toggles on and off. Off is the clean-speech asset, not a
# denoise. The band check fails if click, alarm, or ambience energy is
# missing above that baseline, or if off is not the asset.
#
#   scripts/eval/jarvis-texture.sh
#
# Exit 0 prints one JSON line. A failed test exits 1 and prints no PASS.
# The owner wav is not opened. That measurement stays BLOCKED.
set -euo pipefail
cd "$(dirname "$0")/../.."
cargo test -q -p galion-ml-voice --lib jarvis_texture -- --test-threads=8
printf '%s\n' '{"gate":"jarvis-texture","result":"PASS","wav":"BLOCKED","keep_ratio":0.70,"off":"asset","on":"asset+layer"}'
