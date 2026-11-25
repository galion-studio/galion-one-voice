#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Galion Labs.
#
# The galion-live-demo crate is archived under
# archive/2026-11-25/crates/galion-live-demo. This script does not build it.
#
# The live loop is the HORBM duplex path already on main:
#   cargo test --manifest-path horbm/Cargo.toml -p horbm-duplex
#   bash scripts/horbm-e2e-smoke.sh
#
# Default (including --showcase) runs the duplex package tests. Those tests
# are the virtual 80 ms clock. They do not open a microphone and they do not
# write showcase WAVs. --e2e replaces this process with scripts/horbm-e2e-smoke.sh
# (Linux smoke: loopback, flite or a Piper pack when one is already installed,
# ALSA and Silero when the host has them).
#
# Old flags (--in, --out, --consent, --purpose, --rika-opener, --realtime,
# --mimi, --dot, --quiet, --showcase) are accepted so earlier callers still
# parse. They do not select the archived binary.
# LIVE_DOT, LIVE_REALTIME, and LIVE_EN_CASCADE are ignored.
#
# Last stdout line, except --e2e: GALION_LIVE_DEMO_OUT=archived
# Usage: scripts/run-live-demo.sh [--showcase] [--e2e]
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT="$(pwd)"
MODE=duplex
while [ $# -gt 0 ]; do
  case "$1" in
    --e2e) MODE=e2e; shift ;;
    --showcase|--quiet|--rika-opener|--realtime|--no-play|--dot) shift ;;
    --out|--in|--consent|--purpose|--mimi|--loops|--web|--hold|--reply)
      [ $# -ge 2 ] || { echo "live-demo: $1 needs a value" >&2; exit 64; }
      shift 2 ;;
    -h|--help) sed -n '2,24p' "$0"; exit 0 ;;
    *)
      echo "live-demo: unknown arg: $1" >&2
      echo "live-demo: galion-live-demo is archived. The live loop is:" >&2
      echo "live-demo:   cargo test --manifest-path horbm/Cargo.toml -p horbm-duplex" >&2
      echo "live-demo:   bash scripts/horbm-e2e-smoke.sh" >&2
      exit 64 ;;
  esac
done

if [ "${LIVE_EN_CASCADE:-0}" = 1 ] || [ "${LIVE_DOT:-0}" = 1 ] || [ "${LIVE_REALTIME:-0}" = 1 ]; then
  echo "live-demo: LIVE_EN_CASCADE, LIVE_DOT, and LIVE_REALTIME are ignored. The archived crate is not built."
fi

echo "live-demo: galion-live-demo is archived at archive/2026-11-25/crates/galion-live-demo"
echo "live-demo: Heartbeat::step stays in galion-heartbeat. This script runs the HORBM duplex loop."

if [ "$MODE" = e2e ]; then
  echo "live-demo: exec scripts/horbm-e2e-smoke.sh"
  exec bash "$ROOT/scripts/horbm-e2e-smoke.sh"
fi

echo "live-demo: cargo test --manifest-path horbm/Cargo.toml -p horbm-duplex"
cargo test --manifest-path horbm/Cargo.toml -p horbm-duplex -- --test-threads=1
echo "live-demo: PASS duplex horbm-duplex"
echo "live-demo: full e2e smoke: bash scripts/horbm-e2e-smoke.sh"
echo "GALION_LIVE_DEMO_OUT=archived"
