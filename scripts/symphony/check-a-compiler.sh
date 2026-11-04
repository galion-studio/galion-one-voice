#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
# Section A. Cites the synthetic .gq compiler gate. Does not convert Chatterbox weights.
# Real-weights G5 stays PENDING, as galion-gates/g5-gq-synthetic.sh already says.
set -euo pipefail
cd "$(dirname "$0")/../.."
G5="galion-gates/g5-gq-synthetic.sh"
TEST="crates/galion-tensor/tests/gq_synthetic_g5.rs"
WEIGHTS="crates/galion-ml-voice/src/speech/weights.rs"
fail() { echo "A FAIL — $*"; exit 1; }
[ -f "$G5" ] || fail "missing $G5"
[ -f "$TEST" ] || fail "missing $TEST"
[ -f "$WEIGHTS" ] || fail "missing $WEIGHTS"
grep -q 'SYNTHETIC-PASS' "$G5" || fail "G5 script has no SYNTHETIC-PASS contract"
grep -q 'gq_synthetic_g5' "$G5" || fail "G5 script does not name the round-trip test"
grep -q 'real_g5' "$G5" || fail "G5 script does not keep real G5 pending"
grep -q 'synthetic.gq' "$TEST" || fail "synthetic .gq fixture missing"
grep -q 'no foreign runtime' "$WEIGHTS" || fail "weight loader does not refuse a foreign runtime"
python3 - <<'PY'
import tomllib
with open("LOCKS.toml", "rb") as f:
    locks = tomllib.load(f)
fmts = locks["formats"]
assert fmts["foreign_runtime_allowed"] is False
assert fmts["ggml_container_allowed"] is False
wf = {str(x).lower() for x in fmts["weight_formats"]}
assert wf and wf <= {"safetensors", "gq"}
ex = fmts["exceptions"]["v2s14_chatterbox"]
assert ex["in_process"] is False
assert ex["in_tree_ort"] is False
assert ex["in_tree_ggml"] is False
PY
echo "A PASS — cited galion-gates/g5-gq-synthetic.sh (SYNTHETIC-PASS, real_g5 PENDING) and crates/galion-tensor/tests/gq_synthetic_g5.rs; cargo G5 not run; Chatterbox stays out of process and is not the heartbeat runtime"
