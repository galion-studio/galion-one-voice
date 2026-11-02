#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
# Lock gate (agent A1): validates LOCKS.toml is present and every Eureka lock holds.
# Usage: scripts/check-locks.sh [path/to/LOCKS.toml]   exit 0 = OK, 1 = lock broken, 2 = missing/unparseable.
set -euo pipefail
if [ -n "${1:-}" ]; then
  if d="$(cd "$(dirname "$1")" 2>/dev/null && pwd)"; then LOCKS="$d/$(basename "$1")"; else LOCKS="$1"; fi
fi
cd "$(dirname "$0")/.."
LOCKS="${LOCKS:-LOCKS.toml}"
[ -f "$LOCKS" ] || { echo "FAIL: $LOCKS not found"; exit 2; }
command -v python3 >/dev/null || { echo "FAIL: python3 (>=3.11, tomllib) required"; exit 2; }
exec python3 - "$LOCKS" <<'PY'
import sys, re
try:
    import tomllib
except ImportError:
    print("FAIL: python3 >= 3.11 (tomllib) required"); sys.exit(2)
path = sys.argv[1]
try:
    with open(path, "rb") as f: L = tomllib.load(f)
except Exception as e:
    print(f"FAIL: cannot parse {path}: {e}"); sys.exit(2)

fails = []
def get(dotted):
    cur = L
    for k in dotted.split("."):
        if not isinstance(cur, dict) or k not in cur: return KeyError
        cur = cur[k]
    return cur
def eq(key, want):
    got = get(key)
    if got is KeyError: fails.append(f"missing {key}")
    elif isinstance(want, str) and isinstance(got, str) and want.startswith("#"):
        if got.upper() != want.upper(): fails.append(f"{key} = {got!r}, want {want!r}")
    elif got != want or type(got) is not type(want) and not (isinstance(got,(int,float)) and isinstance(want,(int,float)) and not isinstance(got,bool)):
        fails.append(f"{key} = {got!r}, want {want!r}")
def le(key, cap):
    got = get(key)
    if got is KeyError: fails.append(f"missing {key}")
    elif isinstance(got, bool) or not isinstance(got, (int, float)) or got > cap:
        fails.append(f"{key} = {got!r}, must be number <= {cap}")

# clock
eq("clock.frame_ms", 80); eq("clock.frame_hz", 12.5); eq("clock.configurable", False)
le("clock.frame_budget_p95_ms", 80)
# bands
eq("bands.names", ["you", "me", "feeling"]); eq("bands.count", 3)
eq("bands.rika_reads_transcript", False)
# gates (thresholds may only be tightened, never loosened)
le("gates.barge_in_max_ms", 200); le("gates.onset_p50_max_ms", 300)
le("gates.affect_to_voice_max_ms", 160); le("gates.affect_to_voice_max_frames", 2)
le("gates.rika_yield_max_frames", 1); le("gates.affect_overhead_max_pct", 3)
le("gates.present_max_ms", 1); le("gates.probe_cv_max_pct", 5)
eq("gates.blocking", True)
fm = get("clock.frame_ms"); a2v = get("gates.affect_to_voice_max_ms"); a2f = get("gates.affect_to_voice_max_frames")
if all(isinstance(x, int) for x in (fm, a2v, a2f)) and a2v > fm * a2f:
    fails.append(f"gates.affect_to_voice_max_ms {a2v} > frame_ms*frames {fm*a2f}")
# consent
eq("consent.default", "deny")
for b in ("sense", "adapt", "remember", "share"): eq(f"consent.{b}", "deny")
eq("consent.prohibited_overridable", False)
pp = get("consent.prohibited_purposes")
if pp is KeyError or not {"workplace", "education"} <= set(pp): fails.append("consent.prohibited_purposes must include workplace+education")
eq("consent.withdraw_wipes_ram_history", True)
# formats
eq("formats.ggml_container_allowed", False); eq("formats.foreign_runtime_allowed", False)
eq("formats.weights_vendored", False)
wf = get("formats.weight_formats")
# allowlist (stricter than a denylist): only safetensors / galion .gq; any GGML-family container fails
if wf is KeyError or not wf or any(str(x).lower() not in ("safetensors", "gq") for x in wf): fails.append("formats.weight_formats must exist and be a subset of {safetensors, gq}")
# ui: one dot
eq("ui.rule", "one-dot"); eq("ui.dot_count", 1); eq("ui.dot_fill", "solid")
eq("ui.dot_color", "#F5F5F7"); eq("ui.background", "#0A0A0C")
eq("ui.gradient_from", "#F5A524"); eq("ui.gradient_to", "#7C3AED")
eq("ui.gradient_scope", "background"); eq("ui.chrome_allowed", False)
eq("ui.dot_states", ["idle", "listening", "speaking"])
# licence
eq("licence.code", "Perpetual Open MIT"); eq("licence.paid_tiers", False)

# every locked section must cite Q-* sources
qid = re.compile(r"^Q-[A-Z]+\.Q\d+$")
for sec in ("clock", "bands", "gates", "consent", "formats", "ui", "licence"):
    src = get(f"{sec}.sources")
    if src is KeyError or not src: fails.append(f"{sec}.sources missing"); continue
    if not any(qid.match(s) for s in src): fails.append(f"{sec}.sources has no Q-* id")
    for s in src:
        if s.startswith("Q-") and not qid.match(s): fails.append(f"{sec}.sources bad id {s!r}")

if fails:
    print(f"FAIL: {len(fails)} lock(s) broken in {path}")
    for x in fails: print("  -", x)
    sys.exit(1)
print(f"OK: all Eureka locks hold ({path})")
PY
