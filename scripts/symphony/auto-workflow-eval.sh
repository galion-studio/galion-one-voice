#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
# Gate for the one-dot auto-record workflow. Exit 0 = PASS.
# Does not edit LOCKS.toml. Does not start a second UI.
set -euo pipefail
cd "$(dirname "$0")/../.."

fail() { echo "FAIL: $*"; exit 1; }

doc=docs/galion-voice-v2/symphony/AUTO-WORKFLOW.md
self=docs/galion-voice-v2/symphony/SELF-EVAL.md
page=crates/galion-console/assets/v1-screen.html
hook=crates/galion-console/src/auto_workflow.rs

[[ -f "$doc" ]] || fail "missing $doc"
[[ -f "$self" ]] || fail "missing $self"
[[ -f "$hook" ]] || fail "missing $hook"
[[ -f "$page" ]] || fail "missing $page"

grep -q "Maciej Grajczyk" "$doc" || fail "doc is not signed"
grep -q "auto-record" "$doc" || fail "doc does not name auto-record"
grep -q "POST /turn" "$doc" || fail "doc does not keep the /turn mic path"
grep -q "X-Galion-Proceed" "$doc" || fail "doc does not name the proceed header"
grep -q "no second AI screen" "$doc" || fail "doc does not refuse a second AI screen"
grep -q "LOCKS.toml" "$doc" || fail "doc does not record that LOCKS stay untouched"
grep -q "AUTO-WORKFLOW" "$self" || fail "self-eval does not name this section"
grep -q "auto-workflow-eval.sh" "$self" || fail "self-eval does not name this gate"

grep -q "function proceedTurn" "$page" || fail "page has no proceedTurn"
grep -q 'if(ok) proceedTurn(frames,barge);' "$page" || fail "end of take does not proceed"
grep -q "X-Galion-Proceed" "$page" || fail "page does not send the proceed header"
grep -q 'fetch("turn"' "$page" || fail "page left the /turn mic path"
if grep -q 'id="workflow"\|id="chat"\|id="skills"\|id="agent"' "$page"; then
  fail "page grew a second surface"
fi

if grep -q '"/workflow"\|"/skill"\|"/agent"' galion-render/src/backends/web.rs; then
  fail "web surface grew a second route"
fi
grep -q "X-Galion-Proceed" galion-render/src/backends/web.rs || fail "CORS allow list dropped the proceed header"

if [[ -n "$(git status --porcelain -- LOCKS.toml)" ]]; then
  fail "LOCKS.toml is dirty"
fi

rustc_minor="$(rustc --version | sed -n 's/.* 1\.\([0-9][0-9]*\).*/\1/p')"
if [[ -n "${rustc_minor}" && "${rustc_minor}" -ge 85 ]]; then
  cargo=(cargo)
elif cargo +1.85.0 version >/dev/null 2>&1; then
  cargo=(cargo +1.85.0)
else
  cargo=(cargo)
fi
"${cargo[@]}" test -p galion-console --lib auto_workflow:: -- --test-threads=8
echo "PASS: one-dot auto-record workflow"
