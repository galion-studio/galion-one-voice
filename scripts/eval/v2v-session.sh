#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
# One text-inject session for Samantha and one for Jarvis.
# WAV stays under /tmp or /var/tmp. This script does not start the clone
# worker, does not set GALION_CLONE_STUB_PCM, and does not claim C5.
set -euo pipefail
cd "$(dirname "$0")/../.."
if [[ "${GALION_CLONE_STUB_PCM:-}" == "1" ]]; then
  echo "v2v-session: GALION_CLONE_STUB_PCM=1 is protocol stub PCM. Refusing." >&2
  exit 2
fi
unset GALION_CLONE_LOCAL_REFS || true
WAV="${GALION_V2V_WAV_DIR:-/tmp/galion-v2v-sessions}"
case "$WAV" in
  /tmp/*|/var/tmp/*) ;;
  *)
    echo "v2v-session: wav dir must be under /tmp or /var/tmp" >&2
    exit 2
    ;;
esac
mkdir -p "$WAV"
export TZ=Europe/Zagreb
echo "v2v-session: wav-dir=$WAV"
if [[ -d weights/asr/vosk-model-small-en-us-0.15 ]]; then
  echo "v2v-session: vosk=present"
else
  echo "v2v-session: vosk=absent"
fi
if [[ -f weights/tts/en_US-ljspeech-medium.onnx ]]; then
  echo "v2v-session: piper_pack=present"
else
  echo "v2v-session: piper_pack=absent"
fi
echo "v2v-session: ref_wavs=$(find docs/galion-voice-v2/personas/refs -iname '*.wav' | wc -l)"
cargo +1.85.0 run -q -p galion-console --features phone-turn --example v2v_session -- --wav-dir "$WAV"
echo "v2v-session: files"
find "$WAV" -type f -printf '%p %s\n' | sort
