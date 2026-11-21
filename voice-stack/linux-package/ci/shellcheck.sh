#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
# Shellcheck the user-install scripts. No network.

set -euo pipefail
ROOT=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"
command -v shellcheck >/dev/null 2>&1 || {
  echo "FAIL: shellcheck is not installed" >&2
  exit 1
}
shellcheck -x \
  "$ROOT/install.sh" \
  "$ROOT/uninstall.sh" \
  "$ROOT/lib/common.sh" \
  "$ROOT/lib/models.sh" \
  "$ROOT/ci/verify-unit.sh" \
  "$ROOT/ci/shellcheck.sh" \
  "$ROOT/tests/install_test.sh"
echo "OK: shellcheck"
