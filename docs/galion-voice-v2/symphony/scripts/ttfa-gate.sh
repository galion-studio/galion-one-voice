#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
#
# TTFA / streaming latency gate. Frame bar (80 ms) plus presence onset,
# barge-in, and e_to_a. Default run is the mock stub. A wall-clock ttfa_ms
# is read only from GALION_TTFA_MEASURED. Does not write LOCKS.toml.
#
#   docs/galion-voice-v2/symphony/scripts/ttfa-gate.sh
#   docs/galion-voice-v2/symphony/scripts/ttfa-gate.sh --self-check
#   docs/galion-voice-v2/symphony/scripts/ttfa-gate.sh --write-eval
#   GALION_TTFA_MEASURED=file.json docs/galion-voice-v2/symphony/scripts/ttfa-gate.sh
#
# Exit 0 pass, 1 gate red, 2 usage or IO. Stdout is one JSON object.
set -euo pipefail
cd "$(dirname "$0")/../../../.."
if [ "${1:-}" = "--self-check" ]; then
  exec python3 docs/galion-voice-v2/symphony/scripts/ttfa_check.py self-check
fi
exec python3 docs/galion-voice-v2/symphony/scripts/ttfa_check.py gate "$@"
