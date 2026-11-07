#!/usr/bin/env bash
# Fix (a): build a reference from the base recording's most expressive moments.
# Simple version (worker B): energy-peak window from original.wav → cbx voice add
# as own-<p>-v2-check-a_ref_expressive. Worker A/C may replace via VOICECHECK_FIX_A.
set -euo pipefail
: "${VOICECHECK_OUT_VOICE:?}" "${VOICECHECK_BASE_WAV:?}" "${VOICECHECK_WORK:?}"
DRY="${VOICECHECK_DRY_RUN:-0}"
CBX="${VOICECHECK_CLONING_V2:-/workspace/swarm/voice-v2-sprint/cloning-v2}/pipeline/cbx-v2-timbre/target/release/cbx"
OUT_CLIP="$VOICECHECK_WORK/expressive_ref.wav"

if [[ "$DRY" == "1" ]]; then
  python3 - <<'PY'
import json, os
print(json.dumps({
  "status": "skipped",
  "out_voice": os.environ["VOICECHECK_OUT_VOICE"],
  "detail": "dry-run: would extract high-energy 8s window from base and cbx voice add (no TTS, no voice write)"
}))
PY
  exit 0
fi

python3 - <<'PY'
import json, os, wave, struct, math
base = os.environ["VOICECHECK_BASE_WAV"]
out = os.environ["VOICECHECK_WORK"] + "/expressive_ref.wav"
with wave.open(base, "rb") as w:
    ch, sw, sr, nframes, _, _ = w.getparams()
    raw = w.readframes(nframes)
if sw == 2:
    samples = list(struct.unpack("<" + "h"*(len(raw)//2), raw))
elif sw == 4:
    samples = list(struct.unpack("<" + "i"*(len(raw)//4), raw))
else:
    raise SystemExit(f"unsupported sample width {sw}")
# mono mean
if ch > 1:
    mono = []
    for i in range(0, len(samples), ch):
        mono.append(sum(samples[i:i+ch]) / ch)
else:
    mono = [float(x) for x in samples]
win = int(sr * 8)  # 8 s target (clone-friendly)
hop = int(sr * 0.25)
if len(mono) <= win:
    best_i = 0
    win = len(mono)
else:
    best_i, best_e = 0, -1.0
    for i in range(0, len(mono) - win, hop):
        seg = mono[i:i+win]
        e = math.sqrt(sum(x*x for x in seg) / len(seg))
        if e > best_e:
            best_e, best_i = e, i
start = best_i
# write 16-bit mono
clipped = []
for x in mono[start:start+win]:
    v = int(max(-32767, min(32767, round(x if sw != 2 else x))))
    # if original was int16 already, x is already in that scale
    if sw == 2:
        v = int(max(-32767, min(32767, round(x))))
    else:
        # int32-ish → scale down roughly
        v = int(max(-32767, min(32767, round(x / 65536.0))))
    clipped.append(v)
with wave.open(out, "wb") as w:
    w.setnchannels(1); w.setsampwidth(2); w.setframerate(sr)
    w.writeframes(struct.pack("<" + "h"*len(clipped), *clipped))
print(json.dumps({"start_s": start/sr, "dur_s": win/sr, "sr": sr}), flush=True)
PY

# Protect: never write own-samantha / own-jarvis
case "$VOICECHECK_OUT_VOICE" in
  own-samantha|own-jarvis) echo '{"status":"error","detail":"refusing protected voice"}'; exit 1;;
esac

nice -n 10 "$CBX" --intra-threads 2 voice add --dtype fp32 --name "$VOICECHECK_OUT_VOICE" --voice-wav "$OUT_CLIP" >/dev/null
python3 - <<PY
import json, os
print(json.dumps({
  "status": "ok",
  "out_voice": os.environ["VOICECHECK_OUT_VOICE"],
  "detail": f"expressive 8s ref from {os.environ['VOICECHECK_BASE_WAV']} → voice add"
}))
PY
