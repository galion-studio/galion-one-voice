#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
# Eureka symphony gates A–G. Exit 0 only when every section PASSes.
# OPEN is incomplete and exits 1. LOCKS.toml is hashed before and after and must not change.
#   scripts/symphony/self-eval.sh
#   scripts/symphony/self-eval.sh --only G
#   scripts/symphony/gates/a.sh
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
LOCKS="$ROOT/LOCKS.toml"
before="$(sha256sum "$LOCKS")"
record=(--record)
for arg in "$@"; do
  case "$arg" in
    --only|--no-record) record=() ;;
  esac
done
set +e
if ((${#record[@]})); then
  python3 "$ROOT/scripts/symphony/eval.py" --root "$ROOT" "${record[@]}" "$@"
else
  python3 "$ROOT/scripts/symphony/eval.py" --root "$ROOT" "$@"
fi
rc=$?
set -e
after="$(sha256sum "$LOCKS")"
if [[ "$before" != "$after" ]]; then
  echo "OPEN locks — LOCKS.toml changed during self-eval" >&2
  exit 2
fi
exit "$rc"
