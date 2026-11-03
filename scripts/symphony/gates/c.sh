#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
# Section C · Jarvis texture toggle. Exit 0 on PASS. Non-zero leaves the section open.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
exec python3 "$ROOT/scripts/symphony/eval.py" --root "$ROOT" --only C --no-record
