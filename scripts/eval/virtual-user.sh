#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
# Virtual-user presence eval (T0–T2). Offline. Loopback only. No SSH.
#
# Injects text or a fixture WAV into PhoneLane, synthesizes with CannedLlm and
# Piper or flite when one is installed, captures the reply WAV, and writes JSON.
# TTFA is filled only when a reply WAV comes back. PSTN is not run.
#
# Report: galion-gates/results/presence-eval/<UTC stamp>.json
#         galion-gates/results/presence-eval/latest.json
# WAVs:   galion-gates/results/presence-eval/<UTC stamp>/
# Exit: 0 green, 1 a scored case failed, 2 usage or IO.
# Doc: docs/eval/PRESENCE-EVAL.md
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
OUT="$ROOT/galion-gates/results/presence-eval"
mkdir -p "$OUT"
STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
COMMIT="$(git -C "$ROOT" rev-parse --short HEAD 2>/dev/null || echo unknown)"
WAV="$OUT/$STAMP"
mkdir -p "$WAV"
cd "$ROOT"
cargo run --offline -q -p galion-presence-eval -- \
  --out "$OUT/$STAMP.json" \
  --wav-dir "$WAV" \
  --commit "$COMMIT"
cp "$OUT/$STAMP.json" "$OUT/latest.json"
echo "presence-eval report: $OUT/$STAMP.json"
