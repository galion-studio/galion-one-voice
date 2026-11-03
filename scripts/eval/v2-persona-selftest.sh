#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
# Local self-test for /health, /dot, persona files, and the five moods.
# Does not fetch a clip. Does not start ngrok.
#   BASE=http://127.0.0.1:7878 scripts/eval/v2-persona-selftest.sh
set -euo pipefail
cd "$(dirname "$0")/../.."
BASE="${BASE:-http://127.0.0.1:7878}"
test -f docs/galion-voice-v2/personas/SAMANTHA.md
test -f docs/galion-voice-v2/personas/JARVIS.md
test -f docs/galion-voice-v2/personas/voice/samantha.toml
test -f docs/galion-voice-v2/personas/voice/jarvis.toml
curl -fsS "$BASE/health" | grep -qx ok
curl -fsS "$BASE/v2" | grep -q 'const MOOD'
curl -fsS "$BASE/dot" | grep -q '"persona"'
say() {
  curl -fsS -X POST -H 'Content-Type: application/json' --data "$2" "$BASE/v2/say"
}
say grief '{"text":"My mother died this morning."}' | grep -q '"mood":"grief"'
say joy '{"text":"I got the job. I can'\''t stop smiling."}' | grep -q '"mood":"joy"'
say vuln '{"text":"I don'\''t think anyone actually likes me."}' | grep -q '"mood":"vulnerability"'
say quiet '{"text":"You don'\''t have to say anything. Just stay."}' | grep -q '"mood":"quiet"'
curl -fsS "$BASE/dot" | grep -q '"mood":"quiet"'
echo "v2 persona self-test OK ($BASE)"
