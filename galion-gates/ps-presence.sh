#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License
# PS "presence replay" CPU gate (FINAL-BUILD-SPEC S8). Owner: agent A13 (soft gate in run-gates.sh, A16 runner).
# Runs the galion-presence harness on the deterministic Mock backend (no weights, offline):
#   specs/presence.json      frame-scripted P1–P6 (golden digest 9f657f360c565dbf, checked by cargo test)
#   specs/presence-wav.json  WAV-driven P7–P8 (PCM fixtures in specs/fixtures/), if present
#   PASS : every spec file is green (exit 0)
#   FAIL : any file red (exit 1) or invalid / IO error (exit 2), or the harness does not build
#   SKIP : crates/galion-presence or specs/presence.json missing
# Reports: galion-gates/results/<ts>.presence.json and <ts>.presence-wav.json (+ results/ps-<ts>.json).
# Usage: galion-gates/ps-presence.sh   Exit: 0 PASS, 1 FAIL, 3 SKIP. Last stdout line: `PS <STATUS> — <note>`.
set -uo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
OUT="$HERE/results"; mkdir -p "$OUT" "$HERE/.build"
TS="${GALION_GATES_TS:-$(date +%Y%m%dT%H%M%S%z)}"; JSON="$OUT/ps-$TS.json"; LOGF="$HERE/.build/ps.log"
finish(){
  printf '{"ts":"%s","gate":"PS","status":"%s","note":"%s"}\n' "$TS" "$1" "$(echo "$2" | sed 's/"/\\"/g')" > "$JSON"
  echo "PS $1 — $2"
  case "$1" in PASS) exit 0;; SKIP) exit 3;; *) exit 1;; esac
}
[ -d "$ROOT/crates/galion-presence" ] || finish SKIP "crates/galion-presence missing"
[ -f "$ROOT/specs/presence.json" ] || finish SKIP "specs/presence.json missing"
command -v cargo >/dev/null || finish FAIL "cargo missing"
( cd "$ROOT" && cargo build --offline -q -p galion-presence ) >"$LOGF" 2>&1 \
  || finish FAIL "galion-presence does not build: $(grep -m1 -E '^error' "$LOGF" || tail -n1 "$LOGF")"
NOTE=""; BAD=0
for f in presence presence-wav; do
  [ -f "$ROOT/specs/$f.json" ] || { NOTE+="$f: absent; "; continue; }
  REP="$OUT/$TS.$f.json"
  SUMM="$( cd "$ROOT" && cargo run --offline -q -p galion-presence -- "specs/$f.json" --out "$REP" 2>&1 )"; RC=$?
  echo "$SUMM" >> "$LOGF"
  NS="$(echo "$SUMM" | sed -nE 's/.* · ([0-9]+) specs · .*/\1/p' | head -n1)"
  DG="$(echo "$SUMM" | sed -nE 's/^digest_fnv1a64=([0-9a-f]+)$/\1/p' | tail -n1)"
  case $RC in
    0) NOTE+="$f: green (${NS:-?} specs, digest ${DG:-?}); ";;
    1) BAD=1; NOTE+="$f: RED (see $(basename "$REP")); ";;
    *) BAD=1; NOTE+="$f: invalid rc=$RC ($(echo "$SUMM" | tail -n1)); ";;
  esac
done
[ $BAD = 0 ] && finish PASS "mock backend: ${NOTE%; }" || finish FAIL "${NOTE%; }"
