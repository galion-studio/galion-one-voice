#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License
# G5 gate SKETCH (W20-14) — synthetic tensors only. NOT wired into run-gates.sh.
# Real G5 = pack(real safetensors) → open → verify_all → bit-exact/quant-tol compare; it stays
# PENDING until A21 counsel + HF URIs + sha256 clear and the owner places a weight locally.
# This script only proves the gate *mechanics* on synthetic data:
#   cargo test -p galion-tensor --test gq_synthetic_g5   (8 tests: writer layout, determinism,
#   order, I8 tolerance, on-disk pack→mmap→verify→compare, SHA256SUMS, corruption/drift negatives)
# Last stdout line: `G5 <STATUS> — <note>`. STATUS is SYNTHETIC-PASS / FAIL — never plain PASS,
# so a runner cannot mistake it for the real weights gate. Exit: 0 synthetic pass, 1 fail.
set -uo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"; ROOT="$(cd "$HERE/.." && pwd)"
OUT="$HERE/results"; mkdir -p "$OUT" "$HERE/.build"
TS="$(date +%Y%m%dT%H%M%S%z)"; JSON="$OUT/g5-syn-$TS.json"; LOGF="$HERE/.build/g5-syn.log"
finish(){
  printf '{"ts":"%s","gate":"G5","mode":"synthetic","status":"%s","real_g5":"PENDING","note":"%s"}\n' \
    "$TS" "$1" "$(echo "$2" | sed 's/"/\\"/g')" > "$JSON"
  echo "G5 $1 — $2"; [ "$1" = SYNTHETIC-PASS ] && exit 0 || exit 1
}
command -v cargo >/dev/null || finish FAIL "cargo missing"
( cd "$ROOT" && cargo test -p galion-tensor --offline --test gq_synthetic_g5 ) >"$LOGF" 2>&1; RC=$?
SUM="$(grep -E '^test result:' "$LOGF" | tail -1)"
N="$(echo "$SUM" | awk '{print $4+0}')"
[ $RC -eq 0 ] && [ "${N:-0}" -ge 8 ] || finish FAIL "synthetic G5 tests failed or missing (rc=$RC; $SUM)"
finish SYNTHETIC-PASS "$N synthetic round-trip tests ok; real-weights G5 PENDING (A21/weights path not clear)"
