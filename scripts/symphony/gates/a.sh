#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
# Section A · Compiler. Exit 0 on PASS. Non-zero leaves the section open.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
exec python3 "$ROOT/scripts/symphony/eval.py" --root "$ROOT" --only A --no-record
