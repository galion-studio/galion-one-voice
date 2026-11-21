#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
# Run the WSL installer inside ubuntu:24.04.
# The image has no sound device and no WSLg socket. The audio check must
# skip and the install must still succeed. No model download. No deploy.

set -euo pipefail

ROOT=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
REPO=$(CDPATH='' cd -- "$ROOT/../../.." && pwd)

if [[ -f /.dockerenv && "$(id -u)" -eq 0 ]]; then
  export DEBIAN_FRONTEND=noninteractive
  apt-get update
  apt-get install -y --no-install-recommends \
    bash \
    ca-certificates \
    coreutils \
    curl \
    findutils \
    grep \
    python3 \
    shellcheck \
    systemd \
    tar \
    unzip
fi

if [[ -S /mnt/wslg/PulseServer || -S /mnt/wslg/runtime-dir/pulse/native ]]; then
  echo "FAIL: WSLg PulseAudio socket is present; this run needs a host with no audio device" >&2
  exit 1
fi

log=$(mktemp)
prefix=$(mktemp -d)
models=$(mktemp -d)
cleanup() {
  rm -rf "$prefix" "$models"
  rm -f "$log"
}
trap cleanup EXIT

env -u PULSE_SERVER -u XDG_RUNTIME_DIR -u GALION_WSLG_PULSE \
  GALION_VOICE_VAR="$models" \
  "$ROOT/install.sh" --prefix "$prefix" --skip-build --skip-models >"$log"
grep -q 'WSL_AUDIO=SKIPPED' "$log"
grep -q 'wsl-audio: no WSLg PulseAudio socket; audio check skipped' "$log"
grep -q 'GALION_WSL_INSTALL=OK' "$log"
unit="$prefix/.config/systemd/user/galion-one-voice.service"
grep -q '^Environment=GALION_SAMANTHA_WS=127\.0\.0\.1:7881$' "$unit"
if grep -q '0\.0\.0\.0' "$unit" "$prefix/.config/galion/one-voice.env"; then
  echo "FAIL: install wrote a public bind" >&2
  exit 1
fi
if [[ -e "$prefix/.asoundrc" ]]; then
  echo "FAIL: install replaced ~/.asoundrc" >&2
  exit 1
fi

log2=$(mktemp)
env -u PULSE_SERVER -u XDG_RUNTIME_DIR -u GALION_WSLG_PULSE \
  GALION_VOICE_VAR="$models" \
  "$ROOT/install.sh" --prefix "$prefix" --skip-build --skip-models >"$log2"
grep -q 'GALION_WSL_INSTALL=OK' "$log2"
grep -q "unchanged $unit" "$log2"
rm -f "$log2"

audio=$(find "$ROOT" "$prefix" "$models" -type f \
  \( -name '*.wav' -o -name '*.wave' -o -name '*.mp3' -o -name '*.flac' -o -name '*.ogg' -o -name '*.opus' \) \
  | wc -l | tr -d ' ')
if [[ "$audio" != "0" ]]; then
  echo "FAIL: audio files were written" >&2
  exit 1
fi

"$ROOT/ci/shellcheck.sh"
"$ROOT/ci/verify-unit.sh"
"$ROOT/tests/install_test.sh"
echo "CONTAINER_WSL=OK"
echo "repo=$REPO"
