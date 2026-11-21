#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
# Shellcheck the WSL install scripts. No network.
# source= directives in this tree are resolved from the working directory
# (shellcheck 0.9), so each directory is checked from its own path.

set -euo pipefail
ROOT=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
command -v shellcheck >/dev/null 2>&1 || {
  echo "FAIL: shellcheck is not installed" >&2
  exit 1
}

(
  cd "$ROOT"
  shellcheck -x \
    install.sh \
    share/galion-one-voice \
    share/wsl-audio.sh \
    ci/shellcheck.sh \
    ci/verify-unit.sh \
    ci/container.sh \
    tests/install_test.sh
)

(
  cd "$ROOT/lib"
  shellcheck -x audio.sh silero.sh
)

echo "OK: wsl shellcheck"
