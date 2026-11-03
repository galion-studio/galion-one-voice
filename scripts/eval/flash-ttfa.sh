#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
#
# V2-S14 Flash eval harness. CPU time-to-first-audio, only when a worker
# returns PCM. The default path prints measured:false and no ttfa_ms.
# It does not download weights and it does not call Chatterbox generate.
#
#   scripts/eval/flash-ttfa.sh
#   GALION_FLASH_EVAL=1 GALION_CLONE_STUB_PCM=1 scripts/eval/flash-ttfa.sh
#
# A protocol-stub clock is not a Flash result. product_claim stays false
# until an engine labelled model is recorded in LEDGER.md. This script
# does not record one. STUB-PASS is not HARD-PASS. The hard loop is
# scripts/eval/v2-hard-loop.sh.
set -euo pipefail
cd "$(dirname "$0")/../.."
out=$(cargo run -q -p galion-ml-voice --features speech --example flash_ttfa)
printf '%s\n' "$out"
if [ "${GALION_FLASH_EVAL:-}" != 1 ]; then
  printf '%s\n' "$out" | grep -q '"measured":false'
  if printf '%s\n' "$out" | grep -q 'ttfa_ms'; then
    echo "flash-ttfa: a number was printed without GALION_FLASH_EVAL=1" >&2
    exit 1
  fi
fi
