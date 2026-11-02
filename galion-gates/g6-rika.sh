#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License
# G6 "presence / Rika" CPU gate (D20 §6.3). Owner: agent A10 (slot created by A16; contract kept).
# Fixtures R1–R9 are Rust tests in galion-emotion named `rika::tests::g6_r1_*` … `g6_r9_*`,
# run on the RMS/ZCR stub path (synthetic, seeded 13, no model).
#   PASS : structural checks pass AND all nine g6_rN tests exist and pass
#   FAIL : any structural check fails, any g6_ test fails, or some of R1–R9 have no test
#   SKIP : galion-emotion/src/rika.rs missing, or zero g6_ tests (an empty G6 is never PASS)
# Structural checks (presence, not behaviour):
#   - rika.rs declared in lib.rs and Rika re-exported
#   - RAM only: no std::fs / std::net / File:: / OpenOptions in non-test rika.rs code
#   - Cue has no lexical variant (is_lexical() is const false); hysteresis clamped to >= 2
#   - SENSE + ADAPT gate and Workplace/Education denial present
# Measured latency (VPS R-gates) stays PENDING: this gate is local CPU only.
# Usage: galion-gates/g6-rika.sh      Exit: 0 PASS, 1 FAIL, 3 SKIP.
# Last stdout line: `G6 <STATUS> — <note>` (parsed by run-gates.sh).
set -uo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
OUT="$HERE/results"; mkdir -p "$OUT" "$HERE/.build"
TS="$(date +%Y%m%dT%H%M%S%z)"; JSON="$OUT/g6-$TS.json"; LOGF="$HERE/.build/g6.log"
RIKA="$ROOT/galion-emotion/src/rika.rs"; LIB="$ROOT/galion-emotion/src/lib.rs"
finish(){
  printf '{"ts":"%s","gate":"G6","status":"%s","note":"%s"}\n' "$TS" "$1" "$(echo "$2" | sed 's/"/\\"/g')" > "$JSON"
  echo "G6 $1 — $2"
  case "$1" in PASS) exit 0;; SKIP) exit 3;; *) exit 1;; esac
}
[ -f "$RIKA" ] || finish SKIP "A10 not landed (galion-emotion/src/rika.rs missing)"
command -v cargo >/dev/null || finish FAIL "cargo missing"

# ---- structural checks ----
CODE="$(awk '/^#\[cfg\(test\)\]/{exit} {print}' "$RIKA")"
BAD=""
grep -qE '^pub mod rika;' "$LIB" || BAD+="lib.rs lacks 'pub mod rika'; "
grep -qE 'pub use rika::.*\bRika\b' "$LIB" || BAD+="lib.rs does not re-export Rika; "
echo "$CODE" | grep -qE 'std::fs|std::net|File::|OpenOptions' && BAD+="rika.rs not RAM-only (fs/net use); "
echo "$CODE" | grep -qE 'const fn is_lexical\(self\) -> bool \{[[:space:]]*$' || BAD+="Cue::is_lexical missing; "
echo "$CODE" | grep -A2 'const fn is_lexical' | grep -qE '^[[:space:]]*false[[:space:]]*$' || BAD+="Cue::is_lexical not const false; "
echo "$CODE" | grep -q 'hysteresis.max(2)' || BAD+="hysteresis not clamped >= 2; "
echo "$CODE" | grep -q 'consent_bits::SENSE' || BAD+="no SENSE guard; "
echo "$CODE" | grep -q 'consent_bits::ADAPT' || BAD+="no ADAPT guard; "
grep -q 'Workplace | Purpose::Education' "$ROOT/galion-emotion/src/consent.rs" || BAD+="consent gate lacks Workplace/Education denial; "
[ -z "$BAD" ] || finish FAIL "structural: ${BAD%; }"

# ---- fixtures R1–R9 ----
LIST="$( cd "$ROOT" && cargo test -p galion-emotion --offline -q -- --list 2>"$LOGF" )" \
  || finish FAIL "galion-emotion does not build: $(grep -m1 -E '^error' "$LOGF" || tail -n1 "$LOGF")"
G6T="$(echo "$LIST" | grep -E ': test$' | grep -iE 'g6_' || true)"
[ -n "$G6T" ] || finish SKIP "rika.rs present but no g6_ tests yet (expected g6_r1..g6_r9)"
MISSING=""; for n in 1 2 3 4 5 6 7 8 9; do echo "$G6T" | grep -qiE "g6_r${n}([^0-9]|$)" || MISSING+="R$n "; done
( cd "$ROOT" && cargo test -p galion-emotion --offline -- g6_ ) > "$LOGF" 2>&1; RC=$?
SUM="$(grep -E '^test result:' "$LOGF" | awk '{p+=$4; f+=$6} END{printf "%d passed, %d failed", p, f}')"
NPASS="$(grep -E '^test result:' "$LOGF" | awk '{p+=$4} END{print p+0}')"
[ $RC = 0 ] || finish FAIL "g6_ tests failed ($SUM); see galion-gates/.build/g6.log"
[ "${NPASS:-0}" -gt 0 ] || finish FAIL "g6_ selected 0 tests (empty G6 = fail)"
[ -z "$MISSING" ] || finish FAIL "g6_ tests pass ($SUM) but fixtures missing: ${MISSING% }"
# ---- heartbeat wiring (A10 W2): Rika inside the real 80 ms step(), zero-alloc, P6 lean_in ----
HB="$ROOT/crates/galion-heartbeat"; HBNOTE="heartbeat not landed"
if [ -d "$HB" ]; then
  ( cd "$ROOT" && cargo test -p galion-heartbeat --offline ) > "$HERE/.build/g6-heartbeat.log" 2>&1 \
    || finish FAIL "R1–R9 pass ($SUM) but galion-heartbeat tests fail; see galion-gates/.build/g6-heartbeat.log"
  HSUM="$(grep -E '^test result:' "$HERE/.build/g6-heartbeat.log" | awk '{p+=$4} END{print p+0}')"
  HBNOTE="heartbeat step() wired ($HSUM tests, zero-alloc, P6 lean_in_ms=160)"
fi
finish PASS "R1–R9 pass ($SUM); structural ok (RAM-only, non-lexical, SENSE+ADAPT, >=2-frame hysteresis); $HBNOTE; VPS latency PENDING"
