#!/bin/bash
set -euo pipefail
cd /workspace/swarm/voice-v2-sprint/cloning-v2/emotion-fix
export HF_HUB_OFFLINE=1
LOG=logs/bon_m9.log
TAG=jd_m12_bon
VOICE=own-jarvis-v2-emo-m6
REF=../../clip-store-own/jarvis/reference.wav
mkdir -p runs/$TAG scores logs

wait_slot() {
  while true; do
    n=$(pgrep -af '/cbx .* (speak|batch|voice)' | grep -v grep | grep -v bon_m9 | wc -l || true)
    echo "$(date -Iseconds) wait cbx=$n" | tee -a "$LOG"
    [[ "$n" -le 1 ]] && return 0
    sleep 20
  done
}

echo "$(date -Iseconds) BON START" | tee -a "$LOG"
wait_slot
# n-best=3, mild temp 0.4 for diversity; keep cands
nice -n 10 ./cbx-emo/target/release/cbx --intra-threads 2 batch --dtype fp32 --voice $VOICE \
  --manifest dev4.tsv --out-dir runs/$TAG --target targets/jarvis-avd.json \
  --n-best 3 --keep-cands --temperature 0.4 --top-k 1000 --top-p 0.975 --rep-unique --punc-norm \
  --seed 0 > runs/$TAG/log.txt 2>&1
echo "$(date -Iseconds) RENDER DONE" | tee -a "$LOG"

# Score ALL candidates + winners with emotion
mapfile -t ALL < <(find runs/$TAG -name 't*.wav' | sort)
nice -n 10 ../scorers/emotion/target/release/galion-emotion score --threads 2 --compact --ref $REF "${ALL[@]}" --out scores/${TAG}_all_emo.json

# Build emo-json for select: path -> {arousal, valence}
python3 - <<'PY'
import json
from pathlib import Path
d=json.loads(Path('scores/jd_m12_bon_all_emo.json').read_text())
emo={}
for f in d['files']:
  emo[f['file']]={'arousal':f['avd']['arousal'],'valence':f['avd']['valence']}
Path('scores/jd_m12_bon_emo_map.json').write_text(json.dumps(emo,indent=2))
# also print per-line cand valence
from collections import defaultdict
by=defaultdict(list)
for f in d['files']:
  p=Path(f['file'])
  # cands/t01_c0.wav or t01.wav
  name=p.name
  by[name[:3]].append((f['avd']['valence'], f['vs_ref']['emotion_distance'], str(p)))
for k,v in sorted(by.items()):
  v.sort(reverse=True)
  print(k, 'top valence', v[0][0], 'emo_d', v[0][1], 'n', len(v))
PY

# Re-select with external emo toward ref AVD
mkdir -p runs/${TAG}_sel
nice -n 10 ./cbx-emo/target/release/cbx select \
  --cands-dir runs/$TAG/cands --manifest dev4.tsv --target targets/jarvis-avd.json \
  --emo-json scores/jd_m12_bon_emo_map.json --out-dir runs/${TAG}_sel \
  > runs/${TAG}_sel/log.txt 2>&1 || true

# If select wrote wavs, score them; else pick max valence manually
if ls runs/${TAG}_sel/t*.wav >/dev/null 2>&1; then
  OUT=runs/${TAG}_sel
  STAG=${TAG}_sel
else
  # manual pick highest valence per id among cands
  python3 - <<'PY'
import json, shutil
from pathlib import Path
from collections import defaultdict
d=json.loads(Path('scores/jd_m12_bon_all_emo.json').read_text())
by=defaultdict(list)
for f in d['files']:
  p=Path(f['file'])
  # only cands
  if 'cands' not in str(p) and p.parent.name!='jd_m12_bon':
    continue
  id_=p.stem.split('_')[0] if '_c' in p.stem else p.stem
  if not id_.startswith('t'): continue
  by[id_].append((f['avd']['valence'], -f['vs_ref']['emotion_distance'], p))
outdir=Path('runs/jd_m12_bon_vpick'); outdir.mkdir(parents=True, exist_ok=True)
for id_, items in sorted(by.items()):
  if id_ not in {'t01','t04','t06','t08'}:
    continue
  items.sort(reverse=True)
  src=items[0][2]
  shutil.copy2(src, outdir/f'{id_}.wav')
  print('picked', id_, 'V', items[0][0], 'from', src)
PY
  OUT=runs/jd_m12_bon_vpick
  STAG=jd_m12_bon_vpick
fi

W=$(ls $OUT/t[0-9][0-9].wav | sort)
nice -n 10 ../scorers/emotion/target/release/galion-emotion score --threads 2 --compact --ref $REF $W --out scores/${STAG}_emo.json
nice -n 10 ../scorers/ecapa/target/release/galion-ecapa --ref $REF $W --threads 2 --no-ceiling --out scores/${STAG}_ecapa.json >/dev/null
echo "$(date -Iseconds) BON_DONE $STAG" | tee -a "$LOG"
echo BON_DONE
