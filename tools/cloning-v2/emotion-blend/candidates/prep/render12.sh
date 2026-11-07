#!/usr/bin/env bash
# Hold BOTH emotion-blend/render.lock and /tmp/cbx-render.lock per line.
# Never pgrep for cbx-blend/cbx string (matches this script). Wait via flock only.
set -euo pipefail
B=/workspace/swarm/voice-v2-sprint/cloning-v2
C=$B/emotion-blend/candidates
SPEAK=$B/pipeline/bin/cbx-speak-voice
LOCK1=$B/emotion-blend/render.lock
LOCK2=/tmp/cbx-render.lock
voice=$1; outdir=$2
mkdir -p "$outdir" "$C/logs"
MAN=$B/emotion-fix/test12.tsv
touch "$LOCK1" "$LOCK2"
export CBX_INTRA_THREADS=2
while IFS=$'\t' read -r id text; do
  [[ -z "${id:-}" || "$id" =~ ^# ]] && continue
  out="$outdir/${id}.wav"
  if [[ -f "$out" && -s "$out" ]]; then
    echo "[$(date '+%F %T %Z')] SKIP $voice $id" | tee -a "$C/logs/render.log"
    continue
  fi
  echo "[$(date '+%F %T %Z')] WAITLOCK $voice $id (owners: $(fuser "$LOCK1" 2>/dev/null | tr -s ' ' || echo none))" | tee -a "$C/logs/render.log"
  # Blocking flock on both locks — no pgrep waiter
  flock "$LOCK1" flock "$LOCK2" bash -c '
    set -euo pipefail
    id="$1"; text="$2"; out="$3"; voice="$4"; SPEAK="$5"; C="$6"
    for i in $(seq 1 60); do
      avail=$(awk "/MemAvailable/{print int(\$2/1024)}" /proc/meminfo)
      if [[ ${avail:-0} -ge 2800 ]]; then break; fi
      echo "[$(date "+%F %T %Z")] WAITRAM $voice $id avail=${avail}MB" >>"$C/logs/render.log"
      sleep 5
    done
    echo "[$(date "+%F %T %Z")] RENDER $voice $id" | tee -a "$C/logs/render.log"
    nice -n 10 "$SPEAK" --voice "$voice" --text "$text" --out-wav "$out" --seed 0 --temperature 0 --dtype fp32
    echo "[$(date "+%F %T %Z")] OK $voice $id ($(stat -c%s "$out") bytes)" | tee -a "$C/logs/render.log"
  ' bash "$id" "$text" "$out" "$voice" "$SPEAK" "$C"
done < "$MAN"
echo "[$(date '+%F %T %Z')] DONE render $voice" | tee -a "$C/logs/render.log"
