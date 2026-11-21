#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
#
# Download Vosk small-en and libvosk into weights/ (not committed).
# Pass --whisper to also download whisper.cpp ggml-tiny.en and ggml-base.en.
# CPU only. A sha256 mismatch deletes the partial file and exits 1.
#
#   scripts/fetch-models.sh
#   scripts/fetch-models.sh --whisper
set -euo pipefail
cd "$(dirname "$0")/.."
W="$PWD/weights"
MODEL_URL="https://alphacephei.com/vosk/models/vosk-model-small-en-us-0.15.zip"
MODEL_SHA="30f26242c4eb449f948e42cb302dd7a686cb29a3423a8367f99ff41780942498"
LINUX_URL="https://github.com/alphacep/vosk-api/releases/download/v0.3.45/vosk-linux-x86_64-0.3.45.zip"
LINUX_SHA="bbdc8ed85c43979f6443142889770ea95cbfbc56cffb5c5dcd73afa875c5fbb2"
WIN_URL="https://github.com/alphacep/vosk-api/releases/download/v0.3.45/vosk-win64-0.3.45.zip"
WIN_SHA="f1dcc9cca460630f81ea8f71794f69c80bed6556d2a4e6237b5785e1d2dff34b"
TINY_URL="https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.en.bin?download=true"
TINY_SHA="921e4cf8686fdd993dcd081a5da5b6c365bfde1162e72b08d75ac75289920b1f"
BASE_URL="https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin?download=true"
BASE_SHA="a03779c86df3323075f5e796cb2ce5029f00ec8869eee3fdfb897afe36c6d002"
MODEL_DIR="$W/asr/vosk-model-small-en-us-0.15"
LIB_DIR="$W/asr/libvosk"
WHISPER=0
for arg in "$@"; do
  case "$arg" in
    --whisper) WHISPER=1 ;;
    -h|--help)
      echo "usage: scripts/fetch-models.sh [--whisper]"
      exit 0
      ;;
    *) echo "fetch-models: unknown argument $arg"; exit 2 ;;
  esac
done

sha() { sha256sum "$1" | awk '{print $1}'; }
ok() { [ -f "$1" ] && [ "$(sha "$1")" = "$2" ]; }
model_ready() { [ -f "$MODEL_DIR/am/final.mdl" ] && [ -f "$MODEL_DIR/conf/model.conf" ]; }
get() { # url dest sha
  ok "$2" "$3" && { echo "ok   $2"; return 0; }
  command -v curl >/dev/null || { echo "fetch-models: curl required"; exit 2; }
  echo "get  $1"
  curl -fL --retry 3 -A "galion-sam-asr" -o "$2.part" "$1"
  if ok "$2.part" "$3"; then mv "$2.part" "$2"; echo "ok   $2"; else rm -f "$2.part"; echo "fetch-models: sha256 mismatch for $1"; exit 1; fi
}

mkdir -p "$W/asr" "$W/whisper"
[ -f "$W/.gitignore" ] || printf '*\n!.gitignore\n' > "$W/.gitignore"

if ! model_ready; then
  command -v unzip >/dev/null || { echo "fetch-models: unzip required"; exit 2; }
  get "$MODEL_URL" "$W/asr/vosk-model-small-en-us-0.15.zip" "$MODEL_SHA"
  unzip -q -o "$W/asr/vosk-model-small-en-us-0.15.zip" -d "$W/asr"
  rm -f "$W/asr/vosk-model-small-en-us-0.15.zip"
fi
model_ready && echo "ok   $MODEL_DIR" || { echo "fetch-models: model directory incomplete"; exit 1; }

os=$(uname -s)
arch=$(uname -m)
case "$os" in
  Linux)
    if [ "$arch" != "x86_64" ]; then
      echo "fetch-models: pinned libvosk zip is linux x86_64 (got $arch). Set GALION_VOSK_LIB."
    elif [ ! -f "$LIB_DIR/libvosk.so" ]; then
      command -v unzip >/dev/null || { echo "fetch-models: unzip required"; exit 2; }
      get "$LINUX_URL" "$W/asr/vosk-linux-x86_64-0.3.45.zip" "$LINUX_SHA"
      rm -rf "$W/asr/vosk-linux-x86_64-0.3.45" "$LIB_DIR"
      unzip -q -o "$W/asr/vosk-linux-x86_64-0.3.45.zip" -d "$W/asr"
      mkdir -p "$LIB_DIR"
      cp -a "$W/asr/vosk-linux-x86_64-0.3.45/." "$LIB_DIR/"
      rm -rf "$W/asr/vosk-linux-x86_64-0.3.45" "$W/asr/vosk-linux-x86_64-0.3.45.zip"
    fi
    [ -f "$LIB_DIR/libvosk.so" ] && echo "ok   $LIB_DIR/libvosk.so"
    ;;
  MINGW*|MSYS*|CYGWIN*)
    if [ ! -f "$LIB_DIR/libvosk.dll" ]; then
      command -v unzip >/dev/null || { echo "fetch-models: unzip required"; exit 2; }
      get "$WIN_URL" "$W/asr/vosk-win64-0.3.45.zip" "$WIN_SHA"
      rm -rf "$W/asr/vosk-win64-0.3.45" "$LIB_DIR"
      unzip -q -o "$W/asr/vosk-win64-0.3.45.zip" -d "$W/asr"
      mkdir -p "$LIB_DIR"
      cp -a "$W/asr/vosk-win64-0.3.45/." "$LIB_DIR/"
      rm -rf "$W/asr/vosk-win64-0.3.45" "$W/asr/vosk-win64-0.3.45.zip"
    fi
    [ -f "$LIB_DIR/libvosk.dll" ] && echo "ok   $LIB_DIR/libvosk.dll"
    ;;
  *)
    echo "fetch-models: no pinned libvosk zip for $os $arch. Install libvosk and set GALION_VOSK_LIB."
    ;;
esac

if [ "$WHISPER" = 1 ]; then
  get "$TINY_URL" "$W/whisper/ggml-tiny.en.bin" "$TINY_SHA"
  get "$BASE_URL" "$W/whisper/ggml-base.en.bin" "$BASE_SHA"
fi
echo "fetch-models: done"
