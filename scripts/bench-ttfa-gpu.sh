#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
#
# Cold and warm time-to-first-audio, CPU and CUDA, for the Piper mouth.
# Numbers are written only when a child produced PCM. CUDA is skipped when
# this machine has no CUDA device. WAV and JSON go to GALION_VOICE_TEST_DIR
# (default /mnt/g/galion/logs/voice-test). This script does not download a
# model and does not claim a GPU result.
set -euo pipefail
cd "$(dirname "$0")/.."
export GALION_VOICE_TEST_DIR="${GALION_VOICE_TEST_DIR:-/mnt/g/galion/logs/voice-test}"
exec python3 scripts/bench_ttfa_gpu.py "$@"
