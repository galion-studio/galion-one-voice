#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
# Section E. One page at /. Mic VAD posts /turn. /v2 redirects. No second panel.
# Live browser session is not started. The contract test is this script.
set -euo pipefail
cd "$(dirname "$0")/../.."
fail() { echo "E FAIL — $*"; exit 1; }
PAGE="crates/galion-console/assets/v1-screen.html"
V2="crates/galion-console/assets/v2-screen.html"
[ -f "$PAGE" ] || fail "missing $PAGE"
[ -f "$V2" ] || fail "missing $V2"
[ -f scripts/run-v1-screen.sh ] || fail "missing run-v1-screen.sh"
[ -f scripts/one-ui.sh ] || fail "missing one-ui.sh"
python3 - "$PAGE" "$V2" <<'PY'
import sys, tomllib
page, v2 = sys.argv[1:]
html = open(page, encoding="utf-8").read()
redir = open(v2, encoding="utf-8").read()
need = (
    "function vadFrame",
    "function endUtterance",
    "function postTurn",
    'fetch("turn"',
    "POST /turn",
    "AudioWorklet",
)
for needle in need:
    if needle not in html:
        raise SystemExit(f"one-dot page missing {needle}")
# The hook order is the workflow: VAD ends the utterance, which posts the turn.
if html.find("function vadFrame") > html.find("function endUtterance"):
    raise SystemExit("vadFrame is not before endUtterance")
if html.find("endUtterance()") < 0 or html.find("postTurn(frames,barge)") < 0:
    raise SystemExit("workflow call missing")
if 'location.replace("/")' not in redir:
    raise SystemExit("/v2 does not redirect home")
if "createRadialGradient" in redir:
    raise SystemExit("/v2 still paints a second circle")
with open("LOCKS.toml", "rb") as f:
    ui = tomllib.load(f)["ui"]
if ui["rule"] != "one-dot" or ui["dot_count"] != 1 or ui["chrome_allowed"] is not False:
    raise SystemExit(f"ui lock is not one dot: {ui['rule']!r} count={ui['dot_count']!r}")
print("ONE_DOT_WORKFLOW PASS")
PY
echo "E PASS — cited scripts/run-v1-screen.sh and crates/galion-console/assets/v1-screen.html; ONE_DOT_WORKFLOW fired on the VAD to /turn contract; live browser not run"
