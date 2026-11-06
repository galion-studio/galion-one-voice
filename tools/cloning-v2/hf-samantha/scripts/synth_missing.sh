#!/bin/bash
set -u
BASE=/workspace/swarm/voice-v2-sprint/cloning-v2/hf-samantha
CBX=/workspace/swarm/voice-v2-sprint/bakeoff-t648/src/cbx/target/release/cbx
is_long() { case "$1" in t08|t09|t11) return 0;; *) return 1;; esac; }
wait_mem() {
  local tid="$1"
  while true; do
    avail=$(awk '/MemAvailable/ {print int($2/1024)}' /proc/meminfo)
    ncbx=$(pgrep -c -f '/cbx .*speak' || true)
    # Under shared load: short lines ok with 1 peer; long lines need more RAM but allow 1 peer
    if is_long "$tid"; then need=5000; maxo=1; else need=3200; maxo=2; fi
    echo "$(date +%H:%M:%S) tid=$tid avail=${avail}MB ncbx=$ncbx need>=$need other<=$maxo"
    if [[ $avail -ge $need && ${ncbx:-0} -le $maxo ]]; then return 0; fi
    sleep 12
  done
}
for voice in own-samantha-v2-hf-b-shelf own-samantha-v2-hf-e-mild own-samantha-v2-hf-c-bwe own-samantha-v2-hf-d-orig44; do
  mkdir -p "$BASE/wav/$voice"
  for tid in t04 t03 t05 t02 t07 t06 t01 t10 t12 t08 t11 t09; do
    out="$BASE/wav/$voice/${tid}.wav"
    if [[ -s "$out" ]]; then continue; fi
    text=$(cat "$BASE/text/${tid}.txt")
    ok=0
    for attempt in 1 2 3 4 5; do
      wait_mem "$tid"
      echo "[$voice $tid] START attempt=$attempt"
      rm -f "$out"
      nice -n 10 "$CBX" --intra-threads 2 speak --dtype fp32 --ep cpu --voice "$voice" --text "$text" --out-wav "$out" >>"$BASE/logs/synth.log" 2>&1
      ec=$?
      if [[ $ec -eq 0 && -s "$out" ]]; then echo "[$voice $tid] DONE size=$(stat -c%s "$out")"; ok=1; break; fi
      echo "[$voice $tid] FAIL ec=$ec"; rm -f "$out"; sleep 6
    done
    [[ $ok -eq 1 ]] || echo "[$voice $tid] GAVE_UP"
    sleep 3
  done
done
echo ALL_MISSING_DONE
