#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
# 80 ms heartbeat harness. Prints verdict=PASS, verdict=FAIL, or verdict=BLOCKED.
# Does not fetch weights. Does not edit LOCKS.toml.
set -euo pipefail
cd "$(dirname "$0")/../.."

if ! command -v cargo >/dev/null 2>&1; then
  echo "verdict=BLOCKED reason=cargo-missing"
  exit 2
fi

if cargo +1.85.0 --version >/dev/null 2>&1; then
  CARGO=(cargo +1.85.0)
else
  CARGO=(cargo)
fi

run() {
  "${CARGO[@]}" "$@"
}

set +e
log=$(run test -p galion-heartbeat --test symphony_wire --test heartbeat --test zero_alloc -- --test-threads=1 --nocapture 2>&1)
status=$?
set -e
printf '%s\n' "$log"
if [[ $status -ne 0 ]]; then
  echo "verdict=FAIL cargo_status=$status"
  exit 1
fi
if ! printf '%s\n' "$log" | grep -q 'verdict=PASS'; then
  echo "verdict=FAIL reason=gate-line-missing"
  exit 1
fi

set +e
run test -p galion-ml-voice --features speech --lib frame_matches_heartbeat -- --test-threads=1
c1=$?
run test -p galion-console --features dot-viz --lib sectors_match_heartbeat_frame -- --test-threads=1
c2=$?
run test -p galion-render --lib empathy_switch_needs_eight_unpinned_frames -- --test-threads=1
c3=$?
run test -p galion-console --features phone-turn --lib drain_takes_one_heartbeat_frame -- --test-threads=1
c4=$?
run test -p galion-live-demo --features dot-viz --test live -- --test-threads=1
c5=$?
run test -p galion-ml-voice --features speech --lib nano_pcm_drains_one_heartbeat_frame -- --test-threads=1
c6=$?
set -e
if [[ $c1 -ne 0 || $c2 -ne 0 || $c3 -ne 0 || $c4 -ne 0 || $c5 -ne 0 || $c6 -ne 0 ]]; then
  echo "verdict=FAIL clone=$c1 sectors=$c2 persona=$c3 drain=$c4 live=$c5 nano=$c6"
  exit 1
fi
echo "verdict=PASS"
exit 0
