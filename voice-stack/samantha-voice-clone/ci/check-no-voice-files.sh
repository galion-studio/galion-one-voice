#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
# Fail if a voice or weight file is tracked anywhere under voice-stack/.
set -euo pipefail

is_banned() {
  local path
  path="$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]')"
  case "$path" in
    *.wav|*.mp3|*.flac|*.onnx) return 0 ;;
    *) return 1 ;;
  esac
}

if [[ "${1:-}" == "--self-test" ]]; then
  is_banned "voice-stack/a.wav" || exit 1
  is_banned "voice-stack/a.WAV" || exit 1
  is_banned "voice-stack/a.mp3" || exit 1
  is_banned "voice-stack/a.Mp3" || exit 1
  is_banned "voice-stack/a.flac" || exit 1
  is_banned "voice-stack/a.FLAC" || exit 1
  is_banned "voice-stack/a.onnx" || exit 1
  is_banned "voice-stack/a.ONNX" || exit 1
  if is_banned "voice-stack/a.rs"; then
    exit 1
  fi
  if is_banned "voice-stack/samantha-voice-clone/src/lib.rs"; then
    exit 1
  fi
  printf 'self-test ok\n'
  exit 0
fi

root="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$root"

found="$(git ls-files -- voice-stack | grep -Ei '\.(wav|mp3|flac|onnx)$' || true)"
if [[ -n "$found" ]]; then
  printf 'FAIL: tracked voice file under voice-stack/\n%s\n' "$found" >&2
  exit 1
fi

printf 'ok: no wav, mp3, flac, or onnx tracked under voice-stack/\n'
