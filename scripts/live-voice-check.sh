#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
# Live voice check against a stack that is already listening on loopback.
# Does not start the server. Does not commit a WAV. CPU only. 127.0.0.1 or ::1.
#
#   scripts/live-voice-check.sh
#   GALION_VOICE_WS=127.0.0.1:7881 scripts/live-voice-check.sh
#
# Exit 0 is result=PASS. Exit 1 is result=FAIL. Exit 2 is a refused bind.
# A clock that was not observed is the word unmeasured.
# How to read the report: docs/horbm/README.md section "Live voice check".

set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"

rustc_ok() {
  local ver
  ver=$("$1" --version 2>/dev/null | awk '{print $2}') || return 1
  local major minor
  major=${ver%%.*}
  minor=${ver#*.}
  minor=${minor%%.*}
  [[ "$major" -gt 1 || ( "$major" -eq 1 && "$minor" -ge 85 ) ]]
}

if rustup run 1.85.0 rustc --version >/dev/null 2>&1; then
  rustc=(rustup run 1.85.0 rustc)
elif rustc_ok rustc; then
  rustc=(rustc)
else
  echo "live-voice-check needs rustc 1.85 or newer" >&2
  exit 1
fi

work=$(mktemp -d "${TMPDIR:-/tmp}/live-voice-check.XXXXXX")
trap 'rm -rf "$work"' EXIT

"${rustc[@]}" --edition 2021 -D warnings --test scripts/live_voice_check.rs -o "$work/test"
"$work/test" --test-threads=1

"${rustc[@]}" --edition 2021 -D warnings -O scripts/live_voice_check.rs -o "$work/check"
set +e
"$work/check"
rc=$?
set -e

audio=$(git -C "$root" status --porcelain | grep -Ei '\.(wav|flac|mp3|ogg|opus)$' || true)
if [[ -n "${audio}" ]]; then
  echo "FAIL: voice audio would be committed" >&2
  echo "${audio}" >&2
  exit 1
fi
exit "$rc"
