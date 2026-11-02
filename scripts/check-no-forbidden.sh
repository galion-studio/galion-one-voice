#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License
# CI gate: zero legacy-container-format refs and no third-party inference runtimes in primary crates.
set -euo pipefail
cd "$(dirname "$0")/.."
# Scope: this script gates galion-tensor (Agent B6). Workspace-wide gates live in galion-gates/run-gates.sh.
SCOPE="${1:-crates/galion-tensor}"
fail=0
# Format gate (case-insensitive), excluding this script and target/.
if grep -RIni --exclude-dir=target --exclude-dir=.git --exclude=check-no-forbidden.sh -e 'gguf' "$SCOPE" Cargo.toml ; then
  echo "FAIL: forbidden format reference found"; fail=1
fi
# Runtime-dep gate on Cargo manifests.
if grep -RInE --include=Cargo.toml -e '^\s*(candle[-a-z]*|ort|tract[-a-z]*|burn[-a-z]*|llama[-a-z_]*|ggml[-a-z]*)\s*=' "$SCOPE" Cargo.toml ; then
  echo "FAIL: forbidden runtime dependency in Cargo.toml"; fail=1
fi
[ $fail -eq 0 ] && echo "OK: forbidden-format + forbidden-runtime gates pass"
exit $fail
