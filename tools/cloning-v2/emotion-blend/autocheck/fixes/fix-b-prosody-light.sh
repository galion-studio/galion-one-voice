#!/usr/bin/env bash
# Fix (b): light prosody layer — simple version writes a sidecar JSON of mild
# pace/pitch hints and clones the current voice profile bytes to a check name.
# Worker A global-fix / C candidates may replace via VOICECHECK_FIX_B.
set -euo pipefail
: "${VOICECHECK_OUT_VOICE:?}" "${VOICECHECK_VOICE:?}" "${VOICECHECK_WORK:?}"
DRY="${VOICECHECK_DRY_RUN:-0}"
VOICES="${VOICECHECK_VOICES_DIR:-$HOME/.cache/huggingface/cbx/voices}"

case "$VOICECHECK_OUT_VOICE" in
  own-samantha|own-jarvis) echo '{"status":"error","detail":"refusing protected voice"}'; exit 1;;
esac

if [[ "$DRY" == "1" ]]; then
  python3 - <<'PY'
import json, os
print(json.dumps({
  "status": "skipped",
  "out_voice": os.environ["VOICECHECK_OUT_VOICE"],
  "detail": "dry-run: would copy voice profile + write prosody sidecar (pace=1.03, pitch=+0.4st)"
}))
PY
  exit 0
fi

SRC="$VOICES/${VOICECHECK_VOICE}.cbxvoice"
DST="$VOICES/${VOICECHECK_OUT_VOICE}.cbxvoice"
if [[ ! -f "$SRC" ]]; then
  echo "{\"status\":\"error\",\"detail\":\"missing source voice $SRC\"}"
  exit 1
fi
# Byte-copy profile only; never touch the source.
cp -n "$SRC" "$DST" 2>/dev/null || cp "$SRC" "$DST"
cat > "$VOICECHECK_WORK/prosody-light.json" <<JSON
{
  "schema": "galion-voicecheck-prosody-sidecar/1",
  "pace": 1.03,
  "pitch_semitones": 0.4,
  "note": "apply via cbx speak --pace / --pitch-semitones; production prosody engine stays OFF"
}
JSON
python3 - <<PY
import json, os
print(json.dumps({
  "status": "ok",
  "out_voice": os.environ["VOICECHECK_OUT_VOICE"],
  "detail": "copied cbxvoice + prosody-light sidecar (pace 1.03, +0.4 st)"
}))
PY
