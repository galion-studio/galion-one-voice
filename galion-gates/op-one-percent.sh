#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
# OP "one-percent" CPU gate (V6 engine account inside symphony).
#   P1  non-inference CPU ≤ 1% of the 80 ms tick (800 µs)
#   P3  p99 of that tick work < 800 µs; p3_much_less records p99 < 80 µs
#   late frames are a pause (catch_up_steps stays 0)
# Stub durations and the local process clock are separate series.
# product_claim is false. This is not a hosted-SKU measurement.
# LOCKS.toml is not a target of this gate.
#   PASS : symphony tests pass and the example JSON holds the bars above
#   FAIL : tests fail, JSON is missing a series, or a bar breaks
#   SKIP : crates/galion-symphony is missing
# Usage: galion-gates/op-one-percent.sh
# Exit: 0 PASS, 1 FAIL, 3 SKIP.
# Last stdout line: `OP <STATUS> — <note>` (parsed by run-gates.sh).
set -uo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
OUT="$HERE/results"; mkdir -p "$OUT" "$HERE/.build"
TS="$(date +%Y%m%dT%H%M%S%z)"
JSON="$OUT/op-$TS.json"
LOGF="$HERE/.build/op-one-percent.log"
CRATE="$ROOT/crates/galion-symphony"
finish(){
  # $1 status, $2 note, $3 optional report JSON (one line)
  python3 - "$JSON" "$TS" "$1" "$2" "${3:-}" <<'PY'
import json, sys
path, ts, status, note = sys.argv[1:5]
report = sys.argv[5] if len(sys.argv) > 5 else ""
doc = {"ts": ts, "gate": "OP", "status": status, "note": note}
if report:
    try:
        doc["report"] = json.loads(report)
    except json.JSONDecodeError:
        doc["report_raw"] = report[:500]
with open(path, "w", encoding="utf-8") as f:
    json.dump(doc, f, ensure_ascii=False)
    f.write("\n")
PY
  echo "OP $1 — $2"
  case "$1" in PASS) exit 0;; SKIP) exit 3;; *) exit 1;; esac
}
[ -d "$CRATE" ] || finish SKIP "galion-symphony not landed"
command -v cargo >/dev/null || finish FAIL "cargo missing"
command -v python3 >/dev/null || finish FAIL "python3 missing"

( cd "$ROOT" && cargo test -p galion-symphony --offline ) > "$LOGF" 2>&1
RC=$?
if [ $RC -ne 0 ]; then
  ( cd "$ROOT" && cargo test -p galion-symphony ) > "$LOGF" 2>&1
  RC=$?
fi
SUM="$(grep -E '^test result:' "$LOGF" | awk '{p+=$4; f+=$6} END{printf "%d passed, %d failed", p, f}')"
[ $RC = 0 ] || finish FAIL "galion-symphony tests failed ($SUM); see galion-gates/.build/op-one-percent.log"

# Release: the measured series is the engine, not debug checks.
LINE="$( cd "$ROOT" && cargo run -p galion-symphony --example one-percent --release --offline --quiet 2>>"$LOGF" | grep -E '^\{' | tail -n1 )"
if [ -z "$LINE" ]; then
  LINE="$( cd "$ROOT" && cargo run -p galion-symphony --example one-percent --release --quiet 2>>"$LOGF" | grep -E '^\{' | tail -n1 )"
fi
[ -n "$LINE" ] || finish FAIL "one-percent example printed no JSON; see galion-gates/.build/op-one-percent.log"

NOTE="$(python3 - "$LINE" <<'PY'
import json, sys
try:
    r = json.loads(sys.argv[1])
except json.JSONDecodeError as e:
    print(f"json: {e}")
    sys.exit(2)
errs = []
if r.get("product_claim") is not False:
    errs.append("product_claim")
if r.get("engine") != "v6-one-percent":
    errs.append("engine")
if r.get("tick_ns") != 80_000_000 or r.get("p1_budget_ns") != 800_000 or r.get("p3_ll_ns") != 80_000:
    errs.append("budgets")
if r.get("late_is_pause") is not True:
    errs.append("late_is_pause")
if r.get("catch_up_steps") != 0:
    errs.append("catch_up")
stub, meas = r.get("stub") or {}, r.get("measured") or {}
if stub.get("kind") != "stub" or meas.get("kind") != "measured":
    errs.append("kind")
if not stub.get("n") or not meas.get("n"):
    errs.append("empty-series")
if stub.get("p1_pass") is not True or stub.get("p1_instrument") != "injected":
    errs.append("stub-p1")
if stub.get("pauses", 0) < 1:
    errs.append("stub-late-pause")
if stub.get("p3_under_budget") is not True:
    errs.append("stub-p3")
if meas.get("p1_pass") is not True:
    errs.append(f"measured-p1:{meas.get('p1_note')}")
if meas.get("p1_instrument") not in ("schedstat", "stat"):
    errs.append(f"measured-cpu:{meas.get('p1_instrument')}")
if meas.get("p3_instrument") != "instant":
    errs.append("p3-instrument")
p99 = meas.get("p99_ns")
if not isinstance(p99, int) or p99 >= 800_000:
    errs.append(f"p3-p99:{p99}")
if meas.get("p3_under_budget") is not True:
    errs.append("measured-p3")
ll = "yes" if meas.get("p3_much_less") is True else "no"
ratio = meas.get("p3_ratio")
if errs:
    print("; ".join(errs))
    sys.exit(2)
print(f"P1≤1% ({meas.get('p1_note')}) P3 p99={p99}ns ratio={ratio} much_less={ll} cpu={meas.get('cpu_source')} stub={stub.get('n')} measured={meas.get('n')} product_claim=false")
PY
)"
PYRC=$?
if [ $PYRC -ne 0 ]; then
  finish FAIL "${NOTE:-example JSON rejected}" "$LINE"
fi
finish PASS "$NOTE" "$LINE"
