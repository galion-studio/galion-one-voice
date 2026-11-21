#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
# Check the WSL user unit. Not an install. Not a deploy.
# The ExecStart launcher is absent in CI. That one diagnostic is expected.
#
# Exposure measured with:
#   systemd-analyze security --offline=yes --no-pager
# Offline analysis treats this user unit as a system unit because it has
# no User= line. A user unit must not set User=. The "runs as root" row
# is that analyzer artifact. A larger score is worse.

set -euo pipefail
export LC_ALL=C

ROOT=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
unit="$ROOT/share/systemd/user/galion-one-voice.service"
ceiling=5.0

[[ -f "$unit" ]] || {
  echo "FAIL: missing $unit" >&2
  exit 1
}

if grep -Eq '^User=' "$unit"; then
  echo "FAIL: user unit must not set User=" >&2
  exit 1
fi

for key in \
  '^NoNewPrivileges=yes$' \
  '^ProtectSystem=strict$' \
  '^ProtectHome=yes$' \
  '^PrivateTmp=yes$' \
  '^RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6$' \
  '^IPAddressDeny=any$' \
  '^IPAddressAllow=localhost$' \
  '^ProtectKernelTunables=yes$' \
  '^ProtectKernelModules=yes$' \
  '^ProtectControlGroups=yes$' \
  '^MemoryMax=2G$' \
  '^Environment=GALION_SAMANTHA_WS=127\.0\.0\.1:7881$' \
  '^Environment=GALION_CPU=1$' \
  '^ExecStart=%h/\.local/bin/galion-one-voice$'
do
  if ! grep -Eq "$key" "$unit"; then
    echo "FAIL: $unit is missing $key" >&2
    exit 1
  fi
done

if grep -Eq '0\.0\.0\.0|GALION_LLM|GALION_SAMANTHA_REF=' "$unit"; then
  echo "FAIL: unit binds a public address, sets an LLM, or stores a reference path" >&2
  exit 1
fi
if grep -Eq '^Environment=.*7879' "$unit"; then
  echo "FAIL: unit binds 7879, which is GALION_HARD_ADDR" >&2
  exit 1
fi

command -v systemd-analyze >/dev/null 2>&1 || {
  echo "FAIL: systemd-analyze is not installed" >&2
  exit 1
}

out=$(mktemp)
trap 'rm -f "$out"' EXIT
set +e
systemd-analyze verify "$unit" >"$out" 2>&1
rc=$?
set -e

allowed='^galion-one-voice\.service: Command .+/\.local/bin/galion-one-voice is not executable: No such file or directory$'
unexpected=0
while IFS= read -r line || [[ -n "$line" ]]; do
  [[ -z "$line" ]] && continue
  if [[ "$line" =~ $allowed ]]; then
    continue
  fi
  echo "FAIL: $line" >&2
  unexpected=1
done <"$out"

if [[ "$unexpected" -ne 0 ]]; then
  exit 1
fi
if [[ "$rc" -ne 0 && ! -s "$out" ]]; then
  echo "FAIL: systemd-analyze verify exited $rc with no diagnostic" >&2
  exit 1
fi

sec=$(systemd-analyze security --offline=yes --no-pager "$unit" 2>&1) || {
  echo "FAIL: systemd-analyze security failed" >&2
  printf '%s\n' "$sec" >&2
  exit 1
}
score=$(printf '%s\n' "$sec" | sed -n 's/.*Overall exposure level for .*: \([0-9][0-9]*\(\.[0-9][0-9]*\)\?\).*/\1/p' | head -n 1)
if [[ -z "$score" ]]; then
  echo "FAIL: no exposure score" >&2
  exit 1
fi
if awk -v s="$score" -v c="$ceiling" 'BEGIN { exit !(s > c) }'; then
  echo "FAIL: exposure $score is worse than the measured ceiling $ceiling" >&2
  exit 1
fi
echo "OK: galion-one-voice.service sandbox and exposure $score (ceiling $ceiling)"
