#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
#
# Presence gates for the live-sector clock: onset p50, barge-in, e_to_a.
# Runs galion-presence --json on specs/presence.json and specs/presence-wav.json
# (same offline mock as galion-gates/ps-presence.sh). The result is kind "stub".
# There is no ttfa_ms on this path.
#
#   docs/galion-voice-v2/symphony/scripts/ttfa-presence.sh
# Exit 0 pass, 1 gate red, 2 usage or IO. Stdout is one JSON object.
set -euo pipefail
cd "$(dirname "$0")/../../../.."
exec python3 docs/galion-voice-v2/symphony/scripts/ttfa_check.py presence
