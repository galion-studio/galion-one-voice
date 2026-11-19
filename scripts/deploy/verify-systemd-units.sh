#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Galion Labs.
# Check the systemd units shipped in this tree. Not an install. Not a deploy.
# The ExecStart binaries are absent on a CI runner. That one diagnostic is expected.
# Any other systemd-analyze verify line fails the check.
set -euo pipefail
cd "$(dirname "$0")/../.."

units=(
  deploy/systemd/templates/galion-voice-stage.service
  deploy/systemd/galion-clone-cbx.service
)

command -v systemd-analyze >/dev/null || {
  echo "FAIL: systemd-analyze is not installed" >&2
  exit 1
}

for unit in "${units[@]}"; do
  [[ -f "$unit" ]] || { echo "FAIL: missing $unit" >&2; exit 1; }
  for key in \
    '^NoNewPrivileges=yes$' \
    '^ProtectSystem=strict$' \
    '^ProtectHome=yes$' \
    '^PrivateTmp=yes$' \
    '^RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6$' \
    '^ReadWritePaths=/opt/galion/voice(/.*)?$'
  do
    if ! grep -Eq "$key" "$unit"; then
      echo "FAIL: $unit is missing $key" >&2
      exit 1
    fi
  done
done

out=$(mktemp)
trap 'rm -f "$out"' EXIT
set +e
systemd-analyze verify "${units[@]}" >"$out" 2>&1
rc=$?
set -e

allowed='^(galion-voice-stage\.service: Command /opt/galion/voice/release/current/dot_console_demo is not executable: No such file or directory|galion-clone-cbx\.service: Command /opt/galion/voice/cbx/galion-clone-cbx is not executable: No such file or directory)$'
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
echo "OK: systemd units sandbox and verify"
