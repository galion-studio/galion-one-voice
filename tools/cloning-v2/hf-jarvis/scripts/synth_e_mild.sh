#!/bin/bash
set -u
BASE=/workspace/swarm/voice-v2-sprint/cloning-v2/hf-jarvis
CBX=/workspace/swarm/voice-v2-sprint/bakeoff-t648/src/cbx/target/release/cbx
VOICE=own-jarvis-v2-hf-e-mild
is_long() { case "$1" in t08|t09|t11) return 0;; *) return 1;; esac; }
wait_mem() {
  local tid="$1"
  while true; do
    avail=$(awk '/MemAvailable/ {print int($2/1024)}' /proc/meminfo)
    ncbx=$(pgrep -c -f '/cbx .*speak' || true)
    if is_long "$tid"; then need=5000; maxo=1; else need=3200; maxo=1; fi
    # maxo=1 => allow at most 1 OTHER cbx (total 2 including us about to start)
    echo "$(date +%H:%M:%S) tid=$tid avail=${avail}MB ncbx=$ncbx need>=$need other<=$maxo"
    if [[ $avail -ge $need && ${ncbx:-0} -le $maxo ]]; then return 0; fi
    sleep 12
  done
}
mkdir -p "$BASE/wav/$VOICE"
for tid in t01 t02 t03 t04 t05 t06 t07 t08; do
  out="$BASE/wav/$VOICE/${tid}.wav"
  if [[ -s "$out" ]]; then echo "[$VOICE $tid] SKIP exists"; continue; fi
  text=$(cat "$BASE/text/${tid}.txt")
  ok=0
  for attempt in 1 2 3 4 5; do
    wait_mem "$tid"
    echo "[$VOICE $tid] START attempt=$attempt"
    rm -f "$out"
    nice -n 10 "$CBX" --intra-threads 2 speak --dtype fp32 --ep cpu --voice "$VOICE" --text "$text" --out-wav "$out" >>"$BASE/logs/synth.log" 2>&1
    ec=$?
    if [[ $ec -eq 0 && -s "$out" ]]; then echo "[$VOICE $tid] DONE size=$(stat -c%s "$out")"; ok=1; break; fi
    echo "[$VOICE $tid] FAIL ec=$ec"; rm -f "$out"; sleep 6
  done
  [[ $ok -eq 1 ]] || echo "[$VOICE $tid] GAVE_UP"
  sleep 3
done
echo ALL_E_MILD_DONE
