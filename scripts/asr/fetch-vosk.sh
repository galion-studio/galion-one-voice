#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
#
# Letter A · fetch Vosk small-en and libvosk into <repo>/weights/ (NOT vendored:
# weights/.gitignore ignores everything). CPU only. No GPU. The demo does not
# download on its own. A sha mismatch deletes the partial file and exits 1.
#
#   model : vosk-model-small-en-us-0.15 (~40 MB, Apache-2.0)
#           -> weights/asr/vosk-model-small-en-us-0.15/
#   lib   : libvosk 0.3.45 linux x86_64 (Apache-2.0), dlopen'd by galion-asr-vosk
#           -> weights/asr/libvosk/libvosk.so
#
# Usage: scripts/asr/fetch-vosk.sh [--check]   (--check = verify, no network)
# Exit: 0 present · 1 missing or sha mismatch · 2 no curl
set -euo pipefail
cd "$(dirname "$0")/../.."
W="$PWD/weights"
MODEL_URL="https://alphacephei.com/vosk/models/vosk-model-small-en-us-0.15.zip"
MODEL_SHA="30f26242c4eb449f948e42cb302dd7a686cb29a3423a8367f99ff41780942498"
LIB_URL="https://github.com/alphacep/vosk-api/releases/download/v0.3.45/vosk-linux-x86_64-0.3.45.zip"
LIB_SHA="bbdc8ed85c43979f6443142889770ea95cbfbc56cffb5c5dcd73afa875c5fbb2"
MODEL_DIR="$W/asr/vosk-model-small-en-us-0.15"
LIB_DIR="$W/asr/libvosk"
CHECK=0
[ "${1:-}" = "--check" ] && CHECK=1

ok() { [ -f "$1" ] && [ "$(sha256sum "$1" | awk '{print $1}')" = "$2" ]; }
model_ready() { [ -f "$MODEL_DIR/am/final.mdl" ] && [ -f "$MODEL_DIR/conf/model.conf" ]; }
get() { # url dest sha
  ok "$2" "$3" && { echo "ok   $2"; return 0; }
  [ "$CHECK" = 1 ] && { echo "MISSING/BAD $2"; return 1; }
  command -v curl >/dev/null || { echo "fetch-vosk: curl required"; exit 2; }
  command -v unzip >/dev/null || { echo "fetch-vosk: unzip required"; exit 2; }
  echo "get  $1"
  curl -fL --retry 3 -o "$2.part" "$1"
  if ok "$2.part" "$3"; then mv "$2.part" "$2"; echo "ok   $2"; else rm -f "$2.part"; echo "fetch-vosk: sha256 MISMATCH for $1"; exit 1; fi
}

mkdir -p "$W/asr"
[ -f "$W/.gitignore" ] || printf '*\n!.gitignore\n' > "$W/.gitignore"
rc=0
if ! model_ready; then
  if [ "$CHECK" = 1 ]; then echo "MISSING $MODEL_DIR"; rc=1
  else
    get "$MODEL_URL" "$W/asr/vosk-model-small-en-us-0.15.zip" "$MODEL_SHA"
    unzip -q -o "$W/asr/vosk-model-small-en-us-0.15.zip" -d "$W/asr"
    rm -f "$W/asr/vosk-model-small-en-us-0.15.zip"
  fi
fi
model_ready && echo "ok   $MODEL_DIR" || rc=1

if [ "$(uname -m)" != "x86_64" ]; then
  echo "fetch-vosk: pinned libvosk zip is linux x86_64 (got $(uname -m)). Set GALION_VOSK_LIB."
  [ "$rc" = 0 ] && echo "VOSK=MODEL_ONLY" || echo "VOSK=ABSENT"
  exit "$rc"
fi

if [ ! -f "$LIB_DIR/libvosk.so" ]; then
  if [ "$CHECK" = 1 ]; then echo "MISSING $LIB_DIR/libvosk.so"; rc=1
  else
    get "$LIB_URL" "$W/asr/vosk-linux-x86_64-0.3.45.zip" "$LIB_SHA"
    rm -rf "$W/asr/vosk-linux-x86_64-0.3.45" "$LIB_DIR"
    unzip -q -o "$W/asr/vosk-linux-x86_64-0.3.45.zip" -d "$W/asr"
    mkdir -p "$LIB_DIR"
    cp -a "$W/asr/vosk-linux-x86_64-0.3.45/." "$LIB_DIR/"
    rm -rf "$W/asr/vosk-linux-x86_64-0.3.45" "$W/asr/vosk-linux-x86_64-0.3.45.zip"
  fi
fi
[ -f "$LIB_DIR/libvosk.so" ] && echo "ok   $LIB_DIR/libvosk.so" || rc=1
if [ "$rc" = 0 ]; then echo "VOSK=PRESENT (small-en-us-0.15, libvosk 0.3.45, CPU)"; else echo "VOSK=ABSENT"; fi
exit "$rc"
