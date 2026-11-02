#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
#
# FP-03 · fetch the A21 neural TTS pack (CPU only, MIT) into <repo>/weights/ (NOT vendored:
# weights/.gitignore ignores everything). The ONLY network step of the A21 path; the demo
# itself never downloads. Every file is sha256-pinned; a mismatch deletes it and exits 1.
#
#   runtime : rhasspy/piper 2023.11.14-2 piper_linux_x86_64 (MIT; bundles onnxruntime 1.14.1 MIT,
#             espeak-ng 1.52 GPL-3 for phonemes) -> weights/runtime/piper/piper  (out of process, CPU)
#   voice   : rhasspy/piper-voices en_US-ljspeech-medium (repo MIT; LJSpeech data public domain;
#             VITS, 22.05 kHz, 63.5 MB) -> weights/tts/en_US-ljspeech-medium.onnx{,.json}
#
# Usage: scripts/a21/fetch-a21.sh [--check]   (--check = verify what is on disk, no network)
# Exit: 0 ok · 1 sha mismatch / missing · 2 unsupported arch / no curl
set -euo pipefail
cd "$(dirname "$0")/../.."
W="$PWD/weights"
RT_URL="https://github.com/rhasspy/piper/releases/download/2023.11.14-2/piper_linux_x86_64.tar.gz"
RT_SHA="a50cb45f355b7af1f6d758c1b360717877ba0a398cc8cbe6d2a7a3a26e225992"
V_BASE="https://huggingface.co/rhasspy/piper-voices/resolve/main/en/en_US/ljspeech/medium"
V_ONNX="en_US-ljspeech-medium.onnx"
V_ONNX_SHA="6f52a751e2349abe7a76735eb09dc1875298c77ea2342ffd2fef79ff81b87f22"
V_JSON_SHA="141d612cc0a95ed7efc1ca936b845c2364967f2e9217c5dbfcf69fc4d6c65860"
CHECK=0; [ "${1:-}" = --check ] && CHECK=1
ok() { [ -f "$1" ] && [ "$(sha256sum "$1" | cut -d' ' -f1)" = "$2" ]; }
get() {  # url dest sha
  ok "$2" "$3" && { echo "ok   $2"; return 0; }
  [ "$CHECK" = 1 ] && { echo "MISSING/BAD $2"; return 1; }
  command -v curl >/dev/null || { echo "fetch-a21: curl required"; exit 2; }
  echo "get  $1"; curl -fsSL --retry 3 -o "$2.part" "$1"
  if ok "$2.part" "$3"; then mv "$2.part" "$2"; echo "ok   $2"; else rm -f "$2.part"; echo "fetch-a21: sha256 MISMATCH for $1"; exit 1; fi
}
[ "$(uname -m)" = x86_64 ] || { echo "fetch-a21: pinned runtime is linux x86_64 only (got $(uname -m))"; exit 2; }
mkdir -p "$W/tts" "$W/runtime"
[ -f "$W/.gitignore" ] || printf '*\n!.gitignore\n' > "$W/.gitignore"
rc=0
get "$V_BASE/$V_ONNX" "$W/tts/$V_ONNX" "$V_ONNX_SHA" || rc=1
get "$V_BASE/$V_ONNX.json" "$W/tts/$V_ONNX.json" "$V_JSON_SHA" || rc=1
(cd "$W/tts" && for f in "$V_ONNX" "$V_ONNX.json"; do [ -f "$f" ] && sha256sum "$f" > "$f.sha256"; done) || true
if [ ! -x "$W/runtime/piper/piper" ]; then
  if [ "$CHECK" = 1 ]; then echo "MISSING $W/runtime/piper/piper"; rc=1
  else get "$RT_URL" "$W/runtime/piper_linux_x86_64.tar.gz" "$RT_SHA"; tar xzf "$W/runtime/piper_linux_x86_64.tar.gz" -C "$W/runtime"; rm -f "$W/runtime/piper_linux_x86_64.tar.gz"; fi
fi
[ -x "$W/runtime/piper/piper" ] && echo "ok   $W/runtime/piper/piper"
if [ "$rc" = 0 ] && [ -x "$W/runtime/piper/piper" ]; then
  t0=$(date +%s%N)
  echo "hello galion" | "$W/runtime/piper/piper" --quiet --model "$W/tts/$V_ONNX" --noise_scale 0 --noise_w 0 --output_raw > /tmp/fetch-a21-smoke.raw 2>/dev/null || rc=1
  n=$(stat -c %s /tmp/fetch-a21-smoke.raw 2>/dev/null || echo 0); rm -f /tmp/fetch-a21-smoke.raw
  echo "smoke: ${n} B raw PCM in $(( ($(date +%s%N)-t0)/1000000 )) ms (CPU)"; [ "$n" -gt 1000 ] || rc=1
fi
[ "$rc" = 0 ] && echo "A21=PRESENT (Piper VITS en_US-ljspeech-medium, CPU)" || echo "A21=ABSENT (fetch incomplete)"
exit $rc
