#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
# One clip -> articulation card. Runs each brick as-is, then the Rust glue.
# usage: run.sh IN.wav OUT_DIR   (env: VENV, RHUBARB, BIN, DEVICE)
# Audio never leaves the machine and is never written into the repo.
set -euo pipefail
in="$1"; out="$2"; mkdir -p "$out"
here="$(cd "$(dirname "$0")" && pwd)"
py="${VENV:-$here/.venv}/bin/python"
bin="${BIN:-$here/target/release/galion-articulation}"
stem="$(basename "${in%.*}")"
args=()
if [ -x "$py" ]; then
  "$py" "$here/workers/sparc_worker.py" "$in" "$out/$stem.sparc.json" en "${DEVICE:-cuda:0}" && args+=(--sparc "$out/$stem.sparc.json") || echo "sparc worker failed" >&2
  "$py" "$here/workers/disvoice_worker.py" "$in" "$out/$stem.disvoice.json" && args+=(--disvoice "$out/$stem.disvoice.json") || echo "disvoice worker failed" >&2
fi
if [ -x "${RHUBARB:-}" ]; then
  "$RHUBARB" -q -r phonetic -f json -o "$out/$stem.rhubarb.json" "$in" >/dev/null 2>"$out/$stem.rhubarb.log" && args+=(--rhubarb "$out/$stem.rhubarb.json") || echo "rhubarb failed" >&2
fi
"$bin" analyze --wav "$in" "${args[@]}" --out "$out/$stem.articulation.json"
echo "$out/$stem.articulation.json"
