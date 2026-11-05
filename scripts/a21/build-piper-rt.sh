#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
#
# lat240 / agent 01 · default A21 Piper runtime (owner approval t613s2992, 2026-11-04):
# galion-piper-rt = the same Piper voice pack and espeak-ng phonemizer as piper 2023.11.14-2,
# run on onnxruntime 1.31.1 (MIT) out of process, with thread/spin/core-pinning control.
#   1. fetch onnxruntime-linux-x64-1.31.1 (sha256-pinned) -> weights/runtime/ort/ (not vendored)
#   2. cargo build --release runtimes/galion-piper-rt (its own cargo workspace; never a member of
#      the main workspace, so `cargo tree --workspace` stays ort-free)
#   3. install -> weights/runtime/piper-rt/galion-piper-rt  (picked by speech::weights::piper_runtime)
#   4. smoke: one line of PCM
# Needs scripts/a21/fetch-a21.sh first (voice pack + espeak-ng from the Piper tarball).
# Legacy runtime stays: GALION_PIPER_LEGACY=1 selects weights/runtime/piper/piper.
# Usage: scripts/a21/build-piper-rt.sh [--check]
set -euo pipefail
cd "$(dirname "$0")/../.."
W="${GALION_WEIGHTS_DIR:-$PWD/weights}"
ORT_VER="1.31.1"
ORT_URL="https://github.com/microsoft/onnxruntime/releases/download/v${ORT_VER}/onnxruntime-linux-x64-${ORT_VER}.tgz"
ORT_SHA="c849e36a1ef3e22d4ff217b9d1ce4a3cf5b9467b975610913ae959ad5b16bdc9"
ORT_LIB="$W/runtime/ort/onnxruntime-linux-x64-${ORT_VER}/lib/libonnxruntime.so.${ORT_VER}"
BIN="$W/runtime/piper-rt/galion-piper-rt"
CHECK=0; [ "${1:-}" = --check ] && CHECK=1
[ "$(uname -m)" = x86_64 ] || { echo "build-piper-rt: pinned onnxruntime is linux x86_64 only"; exit 2; }
[ -x "$W/runtime/piper/piper" ] || { echo "build-piper-rt: run scripts/a21/fetch-a21.sh first"; exit 1; }
mkdir -p "$W/runtime/ort" "$W/runtime/piper-rt"
if [ ! -f "$ORT_LIB" ]; then
  [ "$CHECK" = 1 ] && { echo "MISSING $ORT_LIB"; exit 1; }
  T="$W/runtime/ort/ort.tgz"
  if [ ! -f "$T" ] || [ "$(sha256sum "$T" | cut -d' ' -f1)" != "$ORT_SHA" ]; then
    command -v curl >/dev/null || { echo "build-piper-rt: curl required"; exit 2; }
    curl -fsSL --retry 3 -o "$T.part" "$ORT_URL"
    [ "$(sha256sum "$T.part" | cut -d' ' -f1)" = "$ORT_SHA" ] || { mv "$T.part" "$T.bad"; echo "build-piper-rt: sha256 MISMATCH (kept as $T.bad)"; exit 1; }
    mv "$T.part" "$T"
  fi
  tar xzf "$T" -C "$W/runtime/ort"
fi
echo "ok   $ORT_LIB"
if [ "$CHECK" = 0 ]; then
  # runtimes/galion-piper-rt is its own cargo workspace, so with CARGO_TARGET_DIR unset cargo
  # builds into runtimes/galion-piper-rt/target, not $PWD/target, and the cp below copied a
  # stale (or missing) binary. Pin the target dir for the build and the copy alike.
  TD="${CARGO_TARGET_DIR:-$PWD/target}"
  case "$TD" in /*) ;; *) TD="$PWD/$TD" ;; esac
  CARGO_TARGET_DIR="$TD" cargo build --release --manifest-path runtimes/galion-piper-rt/Cargo.toml
  [ -x "$TD/release/galion-piper-rt" ] || { echo "build-piper-rt: no binary at $TD/release/galion-piper-rt"; exit 1; }
  [ -f "$BIN" ] && mv -f "$BIN" "$BIN.bak"
  cp "$TD/release/galion-piper-rt" "$BIN"
fi
[ -x "$BIN" ] || { echo "MISSING $BIN"; exit 1; }
n=$(echo '{"text":"hello galion"}' | GALION_PIPER_RUNTIME_DIR="$W/runtime" "$BIN" -q --json-input --output_raw \
      --model "$W/tts/en_US-ljspeech-medium.onnx" --noise_scale 0 --noise_w 0 2>/dev/null | wc -c)
echo "smoke: $n B raw PCM"; [ "$n" -gt 1000 ] || exit 1
"$BIN" --version
