#!/usr/bin/env bash
# Galion Voice probe runner — Perpetual Open MIT License. Read-only; writes only ./probe-<host>-<ts>.json
# Local:   ./run-probe.sh
# VPS:     DO NOT hand-roll; see ./remote-r0-dryrun.sh (prints commands, never executes)
set -euo pipefail
cd "$(dirname "$0")"
out="probe-$(hostname)-$(date +%Y%m%dT%H%M%S).json"
if command -v cargo >/dev/null; then
  RUSTFLAGS="${RUSTFLAGS:--C target-cpu=native}" cargo build --release --offline -q
  mkdir -p results; out="results/$out"
  ./target/release/galion-probe "$@" | tee "$out"
else
  echo "cargo not found; shell fallback (no benchmarks)" >&2
  { echo "== lscpu =="; (lscpu 2>/dev/null || cat /proc/cpuinfo | head -30)
    echo "== meminfo =="; head -5 /proc/meminfo; echo "== os =="; head -2 /etc/os-release; uname -srm; nproc; } | tee "${out%.json}.txt"
fi
