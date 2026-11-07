#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License
# RG offline Rust gate. Std-only rustc, no cargo fetch, no weights, no microphone.
#   PASS : rustc --test, the test binary, rustc, and the gate binary all exit 0,
#          and the ledger keeps wer/ecapa/mos/latency/ttfa/microphone/weights/network BLOCKED
#   FAIL : any of those commands exits nonzero, or the ledger claims a metric number
# This script does not run a speech, embedding, MOS, latency, or TTFA measurement.
# Those rows stay BLOCKED. GALION_GATES_NO_RG=1 skips it from run-gates.sh.
# Usage: galion-gates/rg-offline.sh
# Exit: 0 PASS, 1 FAIL. Last stdout line: `RG <STATUS> — <note>`.
set -uo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
# shellcheck disable=SC1091
source /home/galion/workspace/galion/env.sh
OUT="$HERE/results"
BUILD="$HERE/.build"
SRC="$HERE/rg/offline_gate.rs"
mkdir -p "$OUT" "$BUILD"
export TMPDIR="$BUILD"
TS="$(date +%Y%m%dT%H%M%S%z)"
JSON="$OUT/rg-$TS.json"

finish() {
  local st="$1" note="$2"
  printf '{"ts":"%s","gate":"RG","status":"%s","note":"%s"}\n' \
    "$TS" "$st" "$(printf '%s' "$note" | sed 's/"/\\"/g')" > "$JSON"
  echo "RG $st — $note"
  if [ "$st" = PASS ]; then exit 0; fi
  exit 1
}

[ -f "$SRC" ] || finish FAIL "rg/offline_gate.rs missing"
command -v rustc >/dev/null 2>&1 || finish FAIL "rustc missing"
if grep -nE 'std::net|TcpStream|UdpSocket|reqwest|hyper' "$SRC" >/dev/null; then
  finish FAIL "network API in rg/offline_gate.rs"
fi

run_rc() {
  local log="$1"
  shift
  set +e
  "$@" >"$log" 2>&1
  local rc=$?
  set -e
  printf '%s' "$rc"
}

set -e
RC_TEST_BUILD="$(run_rc "$BUILD/rg-test-build.log" rustc --edition 2021 --test -O "$SRC" -o "$BUILD/rg_offline_test")"
if [ "$RC_TEST_BUILD" -ne 0 ]; then
  finish FAIL "rustc --test exited $RC_TEST_BUILD (see galion-gates/.build/rg-test-build.log)"
fi
RC_TEST="$(run_rc "$BUILD/rg-test.log" "$BUILD/rg_offline_test" --test-threads=1)"
if [ "$RC_TEST" -ne 0 ]; then
  finish FAIL "rg_offline_test exited $RC_TEST (see galion-gates/.build/rg-test.log)"
fi
RC_BUILD="$(run_rc "$BUILD/rg-build.log" rustc --edition 2021 -O "$SRC" -o "$BUILD/rg_offline")"
if [ "$RC_BUILD" -ne 0 ]; then
  finish FAIL "rustc exited $RC_BUILD (see galion-gates/.build/rg-build.log)"
fi
set +e
LEDGER="$("$BUILD/rg_offline" 2>"$BUILD/rg-run.err")"
RC_RUN=$?
set -e
printf '%s\n' "$LEDGER" > "$BUILD/rg-ledger.json"
if [ "$RC_RUN" -ne 0 ]; then
  finish FAIL "rg_offline exited $RC_RUN ($(tail -n1 "$BUILD/rg-run.err" 2>/dev/null || true))"
fi

set +e
NOTE="$(python3 - "$LEDGER" <<'PY'
import json, sys
raw = sys.argv[1]
try:
    doc = json.loads(raw)
except json.JSONDecodeError as e:
    print(f"ledger json: {e}", file=sys.stderr)
    sys.exit(1)
blocked = ["wer", "ecapa", "mos", "latency", "ttfa", "microphone", "weights", "network"]
rows = {r.get("name"): r for r in doc.get("rows", [])}
if doc.get("gate") != "RG":
    print("gate id", file=sys.stderr)
    sys.exit(1)
if doc.get("used_network") is not False or doc.get("used_weights") is not False or doc.get("used_microphone") is not False:
    print("usage flags", file=sys.stderr)
    sys.exit(1)
if doc.get("fails") != 0:
    print("fails", file=sys.stderr)
    sys.exit(1)
honest = rows.get("honesty-contract") or {}
if honest.get("status") != "PASS" or honest.get("exit") != 0:
    print("honesty-contract", file=sys.stderr)
    sys.exit(1)
for name in blocked:
    row = rows.get(name)
    if not row or row.get("status") != "BLOCKED" or "exit" in row or "value" in row:
        print(f"{name} not BLOCKED", file=sys.stderr)
        sys.exit(1)
banned = ('"wer":', '"ecapa":', '"mos":', '"latency":', '"ttfa":', "wer_mean", "ecapa_mean", "latency_ms", "ttfa_ms")
if any(k in raw for k in banned):
    print("metric number in ledger", file=sys.stderr)
    sys.exit(1)
print("honesty-contract exit 0; wer ecapa mos latency ttfa microphone weights network BLOCKED")
PY
)"
PRC=$?
set -e
if [ "$PRC" -ne 0 ]; then
  finish FAIL "ledger rejected ($(tail -n1 "$BUILD/rg-run.err" 2>/dev/null || true)) $NOTE"
fi

python3 - "$JSON" "$TS" "$NOTE" "$RC_TEST_BUILD" "$RC_TEST" "$RC_BUILD" "$RC_RUN" "$LEDGER" <<'PY'
import json, sys
path, ts, note, c1, c2, c3, c4, ledger = sys.argv[1:9]
doc = {
    "ts": ts,
    "gate": "RG",
    "status": "PASS",
    "note": note,
    "commands": [
        {"cmd": "rustc --edition 2021 --test -O galion-gates/rg/offline_gate.rs", "exit": int(c1)},
        {"cmd": "galion-gates/.build/rg_offline_test --test-threads=1", "exit": int(c2)},
        {"cmd": "rustc --edition 2021 -O galion-gates/rg/offline_gate.rs", "exit": int(c3)},
        {"cmd": "galion-gates/.build/rg_offline", "exit": int(c4)},
    ],
    "ledger": json.loads(ledger),
}
with open(path, "w", encoding="utf-8") as f:
    json.dump(doc, f, ensure_ascii=False)
    f.write("\n")
PY
echo "RG PASS — $NOTE"
exit 0
