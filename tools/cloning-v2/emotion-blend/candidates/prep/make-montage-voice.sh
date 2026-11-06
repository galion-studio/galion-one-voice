#!/usr/bin/env bash
set -euo pipefail
BASE=/workspace/swarm/voice-v2-sprint/cloning-v2
CAND=$BASE/emotion-blend/candidates
LOCK1=$BASE/emotion-blend/render.lock
LOG=$CAND/logs/montage-voice.log
WORK=$1
touch "$LOCK1"
exec 9>"$LOCK1"
echo "[$(date '+%F %T %Z')] flock emotion-blend/render.lock" | tee -a "$LOG"
flock 9
echo "[$(date '+%F %T %Z')] lock held; waiting for free encode binaries" | tee -a "$LOG"
for i in $(seq 1 60); do
  avail=$(awk '/MemAvailable/{print int($2/1024)}' /proc/meminfo)
  nbin=0
  for p in /proc/[0-9]*/exe; do
    t=$(readlink "$p" 2>/dev/null || true)
    case "$t" in
      */release/cbx|*/release/cbx-blend) nbin=$((nbin+1));;
    esac
  done
  if [[ ${avail:-0} -ge 4000 && $nbin -eq 0 ]]; then break; fi
  echo "[$(date '+%F %T %Z')] WAITMEM avail=$avail nbin=$nbin" >>"$LOG"
  sleep 5
done
if [[ -e "$HOME/.cache/huggingface/cbx/voices/own-samantha-v2-blend-montage.cbxvoice" ]]; then
  echo "already exists" | tee -a "$LOG"
  exit 0
fi
echo "[$(date '+%F %T %Z')] RUN cbx-add-voice --emotion-blend work=$WORK" | tee -a "$LOG"
# wrapper acquires /tmp/cbx-render.lock itself — do not nest flock on it
nice -n 10 "$BASE/pipeline/bin/cbx-add-voice" --emotion-blend \
  --blend-work-dir "$WORK" \
  --name own-samantha-v2-blend-montage \
  --voice-wav /workspace/swarm/voice-v2-sprint/clip-store-own/samantha/original.wav \
  --dtype fp32 2>&1 | tee -a "$LOG"
echo "own-samantha=$(sha256sum "$HOME/.cache/huggingface/cbx/voices/own-samantha.cbxvoice")" | tee -a "$LOG"
ls -la "$HOME/.cache/huggingface/cbx/voices/own-samantha-v2-blend-montage.cbxvoice" | tee -a "$LOG"
# show conditioning meta
ls -la "$WORK"/ | tee -a "$LOG"
python3 -c "import json;print(json.dumps(json.load(open('$WORK/conditioning.json')),indent=2)[:2000])" 2>/dev/null | tee -a "$LOG" || true
echo "[$(date '+%F %T %Z')] DONE montage voice" | tee -a "$LOG"
