#!/bin/bash
# Validate own-samantha-v2-emo-r2 on full 12 lines. ONE render at a time.
set -euo pipefail
cd /workspace/swarm/voice-v2-sprint/cloning-v2/emotion-fix
export HF_HUB_OFFLINE=1
ROOT=/workspace/swarm/voice-v2-sprint/cloning-v2
S=$ROOT/scorers
NMR=/workspace/swarm/voice-v2-sprint/clip-store-own/jarvis/reference.wav
CBX=./cbx-emo/target/release/cbx
Q=$S/quality/bin
LOG=logs/validate12.log
mkdir -p logs runs scores text

log() { echo "[$(date '+%Y-%m-%d %H:%M:%S %Z')] $*" | tee -a "$LOG"; }

wait_slot() {
  local tag="$1"
  while true; do
    avail=$(awk '/MemAvailable/ {print int($2/1024)}' /proc/meminfo)
    ncbx=$(pgrep -c -f '/release/cbx .* (speak|batch)' 2>/dev/null || true)
    ok=0
    if [[ ${avail:-0} -ge 3800 && ${ncbx:-0} -eq 0 ]]; then ok=1; fi
    if [[ ${avail:-0} -ge 4800 && ${ncbx:-0} -le 1 ]]; then ok=1; fi
    log "WAIT $tag avail=${avail}MB ncbx=${ncbx:-0} ok=$ok"
    if [[ $ok -eq 1 ]]; then return 0; fi
    sleep 20
  done
}

render_one() {
  local persona="$1" voice="$2" tag="$3"; shift 3
  local out="runs/$tag"
  local n=0
  n=$(ls "$out"/t[0-9][0-9].wav 2>/dev/null | wc -l || true)
  if [[ "$n" -eq 12 ]]; then
    log "SKIP render $tag (12 wavs present)"
    return 0
  fi
  wait_slot "$tag"
  mkdir -p "$out"
  log "START render $tag voice=$voice args=$*"
  nice -n 10 "$CBX" --intra-threads 2 batch --dtype fp32 --voice "$voice" \
    --manifest test12.tsv --out-dir "$out" --target "targets/${persona}.json" \
    --seed 0 "$@" >"$out/log.txt" 2>&1
  n=$(ls "$out"/t[0-9][0-9].wav 2>/dev/null | wc -l || true)
  log "DONE render $tag n=$n"
  [[ "$n" -eq 12 ]] || { log "ERROR $tag expected 12 wavs got $n"; return 1; }
}

score_full() {
  local persona="$1" tag="$2"
  local d="$3"
  local REF_P="/workspace/swarm/voice-v2-sprint/clip-store-own/${persona}/reference.wav"
  local W=()
  local t
  for t in t01 t02 t03 t04 t05 t06 t07 t08 t09 t10 t11 t12; do W+=("$d/$t.wav"); done

  if [[ -f "scores/${tag}_emo.json" && -f "scores/${tag}_ecapa.json" && -f "scores/${tag}_wer.json" && -f "scores/${tag}_mos.json" ]]; then
    log "SKIP score $tag (core scores present)"
    return 0
  fi

  log "START score emo $tag"
  nice -n 10 "$S/emotion/target/release/galion-emotion" score --threads 2 --compact \
    --ref "$REF_P" "${W[@]}" --out "scores/${tag}_emo.json"

  log "START score ecapa $tag"
  nice -n 10 "$S/ecapa/target/release/galion-ecapa" --ref "$REF_P" "${W[@]}" \
    --threads 2 --no-ceiling --out "scores/${tag}_ecapa.json" >/dev/null

  log "START score pros $tag"
  local words=""
  for w in "${W[@]}"; do
    id=$(basename "$w" .wav)
    words="$words $(awk -F'\t' -v id="$id" '$1==id{print $2}' test12.tsv | wc -w)"
  done
  "$CBX" analyze --wav "${W[@]}" --words $words 2>/dev/null | grep '^{' > "scores/${tag}_pros.jsonl" || true

  # persona dists into a sidecar from analyze+target via a tiny python helper after
  log "START score wer $tag"
  nice -n 10 "$Q/galion-wer" --text-dir text --jobs 2 --out "scores/${tag}_wer.json" "${W[@]}"

  log "START score mos $tag"
  nice -n 10 "$Q/galion-mos" --nmr "$NMR" --threads 2 --models "$S/quality/models" \
    --out "scores/${tag}_mos.json" "${W[@]}"

  log "START score hygiene $tag"
  nice -n 10 "$Q/galion-hygiene" --out "scores/${tag}_hygiene.json" "${W[@]}"

  log "START score artic $tag"
  nice -n 10 "$S/articulation/bin/galion-artic" profile -j 2 --out "scores/${tag}_artic.json" "${W[@]}"

  log "DONE score $tag"
}

log "VALIDATE12_START"

# Baseline: reuse existing 12-line own-samantha WAVs (production speak binary, t≈0).
# Same voice profile; REPORT already treated these as the 12-line baseline.
BASE_WAV=/workspace/swarm/voice-v2-sprint/own-clone-analysis/wav/samantha
mkdir -p runs/sam_base12
for t in t01 t02 t03 t04 t05 t06 t07 t08 t09 t10 t11 t12; do
  if [[ ! -e runs/sam_base12/$t.wav ]]; then
    ln -s "$BASE_WAV/$t.wav" "runs/sam_base12/$t.wav"
  fi
done
# If sam_old emo/ecapa exist, copy/adapt; else score. Always ensure quality scores.
if [[ -f scores/sam_old_emo.json && ! -f scores/sam_base12_emo.json ]]; then
  # Re-score against symlink paths for consistent file names in JSON, only if needed.
  # Cheaper: symlink score files and also compute pros/persona for baseline.
  log "Reusing sam_old emo/ecapa as baseline reference; will also score quality on baseline wavs"
fi
wait_slot "score_baseline"
score_full samantha sam_base12 runs/sam_base12
# Also compute persona distances for baseline via analyze already in score_full;
# write batch-like dists with python after all scores.

# 1) r2 at dev settings (t=0.8)
render_one samantha own-samantha-v2-emo-r2 sd_r2_up12 \
  --temperature 0.8 --top-k 1000 --top-p 0.95 --rep-unique --punc-norm
score_full samantha sd_r2_up12 runs/sd_r2_up12

# 2) r2 at temperature 0
render_one samantha own-samantha-v2-emo-r2 sd_r2_t012
score_full samantha sd_r2_t012 runs/sd_r2_t012

# Build persona distance sidecar for baseline (no batch.jsonl)
python3 - <<'PY'
import json, math
from pathlib import Path
root = Path('.')
target = json.loads(Path('targets/samantha.json').read_text())
def dist(p,t):
    def st(a,b): return 12.0*math.log2(max(a,1e-9)/max(b,1e-9))
    d = abs(st(p['f0_median_hz'], t['f0_median_hz']))
    d += abs(math.log(max(p['rate_wps'],1e-9)/max(t['rate_wps'],1e-9)))/0.10
    d += 0.5*abs(p['energy_range_db']-t['energy_range_db'])/3.0
    d += 0.5*abs(p['tilt_db']-t['tilt_db'])/3.0
    d += 0.25*abs(p['f0_sd_st']-t['f0_sd_st'])
    if p['rate_wps'] > t['rate_wps']*1.8: d += 5.0
    return d
for tag in ('sam_base12','sd_r2_up12','sd_r2_t012'):
    batch = Path(f'runs/{tag}/batch.jsonl')
    if batch.exists():
        continue
    pros = Path(f'scores/{tag}_pros.jsonl')
    if not pros.exists():
        continue
    lines=[]
    for line in pros.read_text().splitlines():
        o=json.loads(line)
        tid=Path(o['wav']).stem
        d=dist(o['prosody'], target)
        lines.append(json.dumps({"cand":0,"id":tid,"dist":d,"prosody":o['prosody']}))
        lines.append(json.dumps({"chosen":0,"id":tid,"dist":d}))
    batch.write_text('\n'.join(lines)+'\n')
    print('wrote', batch)
PY

log "VALIDATE12_RENDERS_SCORED"
python3 build_validate12.py
log "VALIDATE12_DONE"
