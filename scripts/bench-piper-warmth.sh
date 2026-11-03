#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
#
# V2-S6 · cold vs warm Piper first-clause PCM.
#
# One JSON object on stdout. No PCM file is written (warmth is a live process,
# not a cache). When the A21 pack or the Piper runtime is missing, the object
# has "measured": false and does not invent a latency number.
#
# What the two numbers mean:
#   cold_first_pcm_ms  fresh worker, first speak, until the first clause's PCM
#                      (process start + model load + "Hi!")
#   warm_first_pcm_ms  second speak, same process, until that first clause's PCM
# PhoneLane::new pays the cold start itself (a discarded "Hello.") so the first
# caller turn is the warm path. This bench does not pre-warm, so both numbers
# are visible.
#
# The worker is out of process: stdin JSON lines, stdout raw s16le, framed by
# Piper's "Real-time factor:" line on stderr (rhasspy/piper 2023.11.14-2).
# Flite is unchanged when the pack is absent. GALION_SPEECH=off is not this path.
#
# Usage: scripts/bench-piper-warmth.sh
set -euo pipefail
cd "$(dirname "$0")/.."
exec cargo run -q -p galion-ml-voice --features speech --example piper_warmth
