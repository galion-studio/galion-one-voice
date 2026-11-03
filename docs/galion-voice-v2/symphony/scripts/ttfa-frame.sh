#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
#
# Live-sector frame bar. Reads LOCKS.toml, both presence specs, the heartbeat
# constant, and live-sectors.json. Asserts frame_ms = 80. Does not write LOCKS.
#
#   docs/galion-voice-v2/symphony/scripts/ttfa-frame.sh
# Exit 0 pass, 1 gate red, 2 usage or IO. Stdout is one JSON object.
set -euo pipefail
cd "$(dirname "$0")"
exec python3 ./ttfa_check.py frame
