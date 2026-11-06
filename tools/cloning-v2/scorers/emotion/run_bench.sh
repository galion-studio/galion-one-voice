#!/usr/bin/env bash
# Rust timing: fp32 and int8 over the same 26 files (single process, 2 ORT threads, model load included).
set -euo pipefail
cd "$(dirname "$0")"
TIMEFORMAT='real %R user %U sys %S'
for mode in fp32 int8; do
  extra=""; [ $mode = int8 ] && extra="--int8"
  { time nice -n 10 target/release/galion-emotion score $extra $(cat results/files.txt) --out results/rust_all_$mode.json ; } 2> logs/rust_all_$mode.time
  echo "$mode $(cat logs/rust_all_$mode.time)"
done
