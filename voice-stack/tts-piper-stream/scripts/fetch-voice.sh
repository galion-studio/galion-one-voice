#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
#
# Download the stock female Piper fallback into this crate's var/ directory.
# Voice: rhasspy/piper-voices en_US-ljspeech-medium (LJ Speech, public domain).
# Runtime: rhasspy/piper 2023.11.14-2 linux x86_64 (MIT), out of process, CPU.
# Pins match scripts/a21/fetch-a21.sh and crates/galion-ml-voice PINNED.
# Does not download a Samantha, Jarvis, or s711 reference. Does not read
# GALION_SAMANTHA_REF. Nothing here is committed (var/ is gitignored).
#
# Usage: scripts/fetch-voice.sh [--check]
#   --check   verify files on disk, no network
# Exit: 0 ok, 1 hash or download failure, 2 unsupported arch or no curl
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VDIR="$ROOT/var/voices"
PDIR="$ROOT/var/piper"
V_ONNX="en_US-ljspeech-medium.onnx"
V_JSON="en_US-ljspeech-medium.onnx.json"
V_ONNX_SHA="6f52a751e2349abe7a76735eb09dc1875298c77ea2342ffd2fef79ff81b87f22"
V_JSON_SHA="141d612cc0a95ed7efc1ca936b845c2364967f2e9217c5dbfcf69fc4d6c65860"
V_BASE="https://huggingface.co/rhasspy/piper-voices/resolve/main/en/en_US/ljspeech/medium"
RT_URL="https://github.com/rhasspy/piper/releases/download/2023.11.14-2/piper_linux_x86_64.tar.gz"
RT_SHA="a50cb45f355b7af1f6d758c1b360717877ba0a398cc8cbe6d2a7a3a26e225992"
CHECK=0
[ "${1:-}" = "--check" ] && CHECK=1
ok() { [ -f "$1" ] && [ "$(sha256sum "$1" | awk '{print $1}')" = "$2" ]; }
get() { # url dest sha
  if ok "$2" "$3"; then echo "ok   $2"; return 0; fi
  if [ "$CHECK" = 1 ]; then echo "MISSING/BAD $2"; return 1; fi
  command -v curl >/dev/null || { echo "fetch-voice: curl required"; exit 2; }
  echo "get  $1"
  curl -fL --retry 4 --retry-delay 2 -A "galion-tts-piper/0.0.1" -o "$2.part" "$1"
  if ok "$2.part" "$3"; then mv "$2.part" "$2"; echo "ok   $2"; else rm -f "$2.part"; echo "fetch-voice: sha256 mismatch for $1"; exit 1; fi
}
mkdir -p "$VDIR" "$ROOT/var"
rc=0
get "$V_BASE/$V_ONNX" "$VDIR/$V_ONNX" "$V_ONNX_SHA" || rc=1
get "$V_BASE/$V_JSON" "$VDIR/$V_JSON" "$V_JSON_SHA" || rc=1
if [ "$rc" = 0 ]; then
  (cd "$VDIR" && sha256sum "$V_ONNX" > "$V_ONNX.sha256" && sha256sum "$V_JSON" > "$V_JSON.sha256")
fi
if [ "$(uname -s)" != Linux ] || [ "$(uname -m)" != x86_64 ]; then
  echo "fetch-voice: pinned runtime is linux x86_64 (got $(uname -s) $(uname -m)). Voice files only. Set GALION_A21_RUNTIME to a local piper."
  [ "$rc" = 0 ] && exit 2 || exit "$rc"
fi
if [ ! -x "$PDIR/piper" ]; then
  if [ "$CHECK" = 1 ]; then echo "MISSING $PDIR/piper"; rc=1
  else
    get "$RT_URL" "$ROOT/var/piper_linux_x86_64.tar.gz" "$RT_SHA"
    tar -xzf "$ROOT/var/piper_linux_x86_64.tar.gz" -C "$ROOT/var"
    rm -f "$ROOT/var/piper_linux_x86_64.tar.gz"
  fi
fi
[ -x "$PDIR/piper" ] && echo "ok   $PDIR/piper" || rc=1
if [ "$rc" = 0 ]; then
  echo "voice=$VDIR/$V_ONNX"
  echo "config=$VDIR/$V_JSON"
  echo "runtime=$PDIR/piper"
fi
exit "$rc"
