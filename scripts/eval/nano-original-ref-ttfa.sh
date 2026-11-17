#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Galion Labs.
#
# Original-ref Nano first-PCM and 80 ms first-playable-frame harness.
# Stubs are FAIL. Piper is not this clock. No real clone PCM => BLOCKED.
# Does not edit LOCKS.toml or REFS.sha256. Does not call a paid model.
#
# Exit 0: a real original-ref Nano capture was classified. Read claim_80ms.
# Exit 1: FAIL or HARD-FAIL.
# Exit 2: BLOCKED. Not a product TTFA. 80 ms is not claimed.
set -euo pipefail
cd "$(dirname "$0")/../.."

files=(LOCKS.toml scripts/clone/REFS.sha256)
for extra in SOUL.md config.yaml; do
  if [ -f "$extra" ]; then
    files+=("$extra")
  fi
done
before=$(sha256sum "${files[@]}")
set +e
out=$(python3 scripts/eval/nano_original_ref_ttfa.py)
code=$?
set -e
printf '%s\n' "$out"
after=$(sha256sum "${files[@]}")
if [ "$before" != "$after" ]; then
  echo "FAIL: a locked file hashed by this harness changed" >&2
  exit 1
fi
REPORT=$out CODE=$code python3 - << 'PY'
import importlib.util
import json
import os
import sys

code = int(os.environ["CODE"])
doc = json.loads(os.environ["REPORT"])
spec = importlib.util.spec_from_file_location(
    "nano_original_ref_ttfa", "scripts/eval/nano_original_ref_ttfa.py"
)
mod = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mod)
mod.validate_public(doc)
expect = {"BLOCKED": 2, "FAIL": 1, "HARD-FAIL": 1, "MEASURED": 0}
if expect.get(doc.get("gate")) != code:
    sys.stderr.write("exit %s does not match gate %s\n" % (code, doc.get("gate")))
    sys.exit(1)
if doc.get("gate") != "MEASURED" and doc.get("measurement") == "measured":
    sys.stderr.write("measurement flag set without MEASURED\n")
    sys.exit(1)
PY
exit "$code"
