#!/usr/bin/env bash
# Fix (c): combined — expressive ref (a) then light prosody sidecar (b).
set -euo pipefail
: "${VOICECHECK_OUT_VOICE:?}" "${VOICECHECK_AUTOCHECK:?}"
DRY="${VOICECHECK_DRY_RUN:-0}"
HERE="$VOICECHECK_AUTOCHECK/fixes"

case "$VOICECHECK_OUT_VOICE" in
  own-samantha|own-jarvis) echo '{"status":"error","detail":"refusing protected voice"}'; exit 1;;
esac

if [[ "$DRY" == "1" ]]; then
  python3 - <<'PY'
import json, os
print(json.dumps({
  "status": "skipped",
  "out_voice": os.environ["VOICECHECK_OUT_VOICE"],
  "detail": "dry-run: would run fix-a then fix-b into combined check voice"
}))
PY
  exit 0
fi

# Run A into a temp voice, then B from that temp into OUT.
TMP_VOICE="own-${VOICECHECK_PERSONA}-v2-check-c_tmp_ref"
export VOICECHECK_OUT_VOICE_SAVE="$VOICECHECK_OUT_VOICE"
export VOICECHECK_OUT_VOICE="$TMP_VOICE"
bash "$HERE/fix-a-ref-expressive.sh" >/tmp/vc-fix-a-out.json
export VOICECHECK_VOICE="$TMP_VOICE"
export VOICECHECK_OUT_VOICE="$VOICECHECK_OUT_VOICE_SAVE"
bash "$HERE/fix-b-prosody-light.sh"
# fix-b already printed JSON; done.
