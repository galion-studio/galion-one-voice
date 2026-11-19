#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Galion Labs.
# Fail if a shipped unit's exposure score is worse than the ceiling measured
# for this change. Not an install. Not a deploy.
#
# Measured with `systemd-analyze security --offline=yes --no-pager`
# on systemd 255 (255.4-1ubuntu8.18):
#   deploy/systemd/templates/galion-voice-stage.service  7.7
#   deploy/systemd/galion-clone-cbx.service              8.1
# A larger number is a worse exposure score.
set -euo pipefail
export LC_ALL=C
cd "$(dirname "$0")/../.."

command -v systemd-analyze >/dev/null || {
  echo "FAIL: systemd-analyze is not installed" >&2
  exit 1
}

check() {
  local unit=$1 ceiling=$2 out score
  out=$(systemd-analyze security --offline=yes --no-pager "$unit" 2>&1) || {
    echo "FAIL: systemd-analyze security failed for $unit" >&2
    printf '%s\n' "$out" >&2
    exit 1
  }
  score=$(printf '%s\n' "$out" | sed -n 's/.*Overall exposure level for .*: \([0-9][0-9]*\(\.[0-9][0-9]*\)\?\).*/\1/p' | head -n 1)
  if [[ -z "$score" ]]; then
    echo "FAIL: no exposure score for $unit" >&2
    printf '%s\n' "$out" >&2
    exit 1
  fi
  if awk -v s="$score" -v c="$ceiling" 'BEGIN { exit !(s > c) }'; then
    echo "FAIL: $unit exposure $score is worse than the measured ceiling $ceiling" >&2
    exit 1
  fi
  echo "OK: $unit exposure $score (ceiling $ceiling)"
}

check deploy/systemd/templates/galion-voice-stage.service 7.7
check deploy/systemd/galion-clone-cbx.service 8.1
