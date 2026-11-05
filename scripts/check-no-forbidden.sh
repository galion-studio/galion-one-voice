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
# Owner exception t613s2992 (2026-11-04, LOCKS formats.exceptions.a21_piper): the single
# `ort = { ... "load-dynamic" ... }` line of runtimes/galion-piper-rt/Cargo.toml (its own
# cargo workspace, out of process, excluded from the main workspace) is allowed. Nothing else.
PIPER_RT="runtimes/galion-piper-rt/Cargo.toml"
HITS="$(grep -RInE --include=Cargo.toml -e '^\s*(candle[-a-z]*|ort|tract[-a-z]*|burn[-a-z]*|llama[-a-z_]*|ggml[-a-z]*)\s*=' "$SCOPE" Cargo.toml || true)"
if [ -f "$PIPER_RT" ] && grep -qE '^\[workspace\]' "$PIPER_RT" && grep -qE '^exclude = \[.*"runtimes/galion-piper-rt"' Cargo.toml; then
  HITS="$(echo "$HITS" | grep -vE "^(\./)?$PIPER_RT:[0-9]+:ort = \{.*\"load-dynamic\"" || true)"
fi
if [ -n "$HITS" ]; then
  echo "$HITS"; echo "FAIL: forbidden runtime dependency in Cargo.toml"; fail=1
fi
[ $fail -eq 0 ] && echo "OK: forbidden-format + forbidden-runtime gates pass"
exit $fail
