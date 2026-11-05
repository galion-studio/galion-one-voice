#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
# Section G. 80 ms tick, 1% non-inference ceiling, EPYC / CX23 / CX43 cited as a plan.
# Heartbeat::step is measured on the stub path. No other latency number is invented.
set -euo pipefail
cd "$(dirname "$0")/../.."
fail() { echo "G FAIL — $*"; exit 1; }
DOC="docs/galion-voice-v2/symphony"
for n in "EPYC 9655P" "CX23" "CX43" "800" "80 ms" "1%" "no GPU" "not measured"; do
  grep -q -F "$n" "$DOC/CPU.md" || fail "CPU.md missing $n"
  grep -q -F "$n" "$DOC/LATENCY.md" || fail "LATENCY.md missing $n"
done
python3 - <<'PY'
import re, tomllib
def const(path, expr):
    text = open(path, encoding="utf-8").read()
    if expr not in text:
        raise SystemExit(f"{path} missing {expr}")
const("crates/galion-heartbeat/src/lib.rs", "pub const FRAME_MS: u32 = 80;")
const("crates/galion-mimi/src/config.rs", "pub const FRAME_MS: u32 = 80;")
const("galion-render/src/dot.rs", "pub const FRAME_MS: u64 = 80;")
const("crates/galion-console/src/heartbeat.rs", "pub const FRAME_MS: u64 = 80;")
with open("LOCKS.toml", "rb") as f:
    locks = tomllib.load(f)
frame = locks["clock"]["frame_ms"]
budget = locks["clock"]["frame_budget_p95_ms"]
if frame != 80 or budget > 80:
    raise SystemExit(f"LOCKS clock frame_ms={frame} frame_budget_p95_ms={budget}")
if locks["clock"]["configurable"] is not False:
    raise SystemExit("frame clock is configurable")
PY

LOG="$(mktemp)"
trap 'rm -f "$LOG"' EXIT
# Heartbeat's rust-version is 1.85. Use that toolchain when the default rustc is older.
CARGO=(cargo)
if ! python3 - <<'PY'
import subprocess
ver = subprocess.check_output(["rustc", "--version"], text=True).split()[1]
parts = [int(x) for x in ver.split(".")[:2]]
raise SystemExit(0 if parts >= [1, 85] else 1)
PY
then
  if rustup run 1.85.0 rustc --version >/dev/null 2>&1; then
    CARGO=(cargo +1.85.0)
  else
    fail "rustc >= 1.85 required to measure galion-heartbeat"
  fi
fi
run_cargo() {
  local profile="$1"
  local flag=()
  [ "$profile" = release ] && flag=(--release)
  if ! "${CARGO[@]}" test -p galion-heartbeat "${flag[@]}" --offline --test frame_budget -- --nocapture >"$LOG" 2>&1; then
    if grep -q '^test result: FAILED' "$LOG"; then
      cat "$LOG" >&2
      return 1
    fi
    "${CARGO[@]}" test -p galion-heartbeat "${flag[@]}" --test frame_budget -- --nocapture >"$LOG" 2>&1 || { cat "$LOG" >&2; return 1; }
  fi
  "${CARGO[@]}" test -p galion-heartbeat "${flag[@]}" --offline --test zero_alloc -- --nocapture >>"$LOG" 2>&1 \
    || "${CARGO[@]}" test -p galion-heartbeat "${flag[@]}" --test zero_alloc -- --nocapture >>"$LOG" 2>&1 \
    || { cat "$LOG" >&2; return 1; }
  "${CARGO[@]}" test -p galion-heartbeat "${flag[@]}" --offline --test heartbeat -- late_frame_is_a_pause -- --nocapture >>"$LOG" 2>&1 \
    || "${CARGO[@]}" test -p galion-heartbeat "${flag[@]}" --test heartbeat -- late_frame_is_a_pause -- --nocapture >>"$LOG" 2>&1 \
    || { cat "$LOG" >&2; return 1; }
  "${CARGO[@]}" test -p galion-heartbeat "${flag[@]}" --offline --test heartbeat -- frame_constants_match_locks -- --nocapture >>"$LOG" 2>&1 \
    || "${CARGO[@]}" test -p galion-heartbeat "${flag[@]}" --test heartbeat -- frame_constants_match_locks -- --nocapture >>"$LOG" 2>&1 \
    || { cat "$LOG" >&2; return 1; }
}

run_cargo debug || fail "galion-heartbeat stub measurement failed"
LINE="$(grep 'GALION_FRAME_BUDGET ' "$LOG" | tail -n 1 || true)"
[ -n "$LINE" ] || fail "no GALION_FRAME_BUDGET line"
eval_line() {
  python3 - "$1" <<'PY'
import sys
line = sys.argv[1]
want = {
    "kind": "measured-stub",
    "frame_ms": "80",
    "budget_us": "800",
    "late_pause": "1",
}
got = {}
for part in line.split()[1:]:
    if "=" not in part:
        continue
    k, v = part.split("=", 1)
    got[k] = v
for k, v in want.items():
    if got.get(k) != v:
        raise SystemExit(f"{k}={got.get(k)!r} want {v}")
p99 = int(got["p99_us"])
max_us = int(got["max_us"])
ppm = int(got["step_duty_ppm"])
profile = got.get("profile", "")
if profile not in ("debug", "release"):
    raise SystemExit(f"profile {profile}")
if max_us >= 80_000:
    raise SystemExit(f"max_us {max_us} past the frame")
# 10_000 ppm is 1% of the frame.
over = p99 > 800 or ppm > 10_000
print(f"{profile} {p99} {max_us} {ppm} {int(over)}")
PY
}
PARSED="$(eval_line "$LINE")" || fail "budget line rejected: $LINE"
read -r PROFILE P99 MAXU PPM OVER <<<"$PARSED"
if [ "$OVER" = 1 ]; then
  run_cargo release || fail "release remeasure failed after debug p99_us=$P99"
  LINE="$(grep 'GALION_FRAME_BUDGET ' "$LOG" | tail -n 1)"
  PARSED="$(eval_line "$LINE")" || fail "release budget line rejected: $LINE"
  read -r PROFILE P99 MAXU PPM OVER <<<"$PARSED"
  [ "$OVER" = 0 ] || fail "release p99_us=$P99 still over 800"
fi
grep -q 'test result: ok' "$LOG" || fail "heartbeat tests did not report ok"
echo "$LINE"
echo "G PASS — measured-stub profile=$PROFILE p99_us=$P99 max_us=$MAXU step_duty_ppm=$PPM budget_us=800 frame_ms=80 late=pause; host CPU and EPYC/CX23/CX43 capacity not measured"
