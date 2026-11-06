#!/usr/bin/env python3
"""Aggregate mild Jarvis emotion scores vs matched baseline."""
import json, sys
from pathlib import Path
BASE = Path('/workspace/swarm/voice-v2-sprint/cloning-v2/emotion-fix/scores')
KEEP = {'t01','t04','t06','t08'}
FLOOR_DELTA = 0.01

def stem_id(path):
    s = Path(path).stem
    for k in KEEP | {f't{i:02d}' for i in range(1,13)}:
        if k in s:
            return k
    return s

def load(tag, matched=True):
    emo = json.loads((BASE/f'{tag}_emo.json').read_text())
    ecapa = json.loads((BASE/f'{tag}_ecapa.json').read_text())
    files = emo['files']
    if matched and len(files) > 4:
        files = [f for f in files if stem_id(f['file']) in KEEP]
    n=len(files)
    a=sum(f['avd']['arousal'] for f in files)/n
    v=sum(f['avd']['valence'] for f in files)/n
    d=sum(f['avd']['dominance'] for f in files)/n
    edist=sum(f['vs_ref']['emotion_distance'] for f in files)/n
    avd=sum(f['vs_ref']['avd_dist'] for f in files)/n
    labels={}
    for f in files:
        top=f['ser']['top']
        labels[top]=labels.get(top,0)+1
    scores = ecapa.get('scores') or {}
    if matched:
        sims=[val for path,val in scores.items() if stem_id(path) in KEEP]
        if not sims:
            sims=list(scores.values())
    else:
        sims=list(scores.values())
    ecapa_mean = sum(sims)/len(sims) if sims else float(ecapa.get('mean') or float('nan'))
    return {
        'tag': tag, 'n': n, 'ecapa': ecapa_mean, 'emotion_distance': edist,
        'avd_distance': avd, 'arousal': a, 'valence': v, 'dominance': d,
        'labels': labels,
    }

def main(tags):
    avail=[t for t in tags if (BASE/f'{t}_emo.json').exists() and (BASE/f'{t}_ecapa.json').exists()]
    if 'jar_old' not in avail and (BASE/'jar_old_emo.json').exists():
        avail=['jar_old']+avail
    rows=[load(t) for t in avail]
    if not rows:
        print('no scores yet'); return
    base=next(r for r in rows if r['tag']=='jar_old')
    print(f"{'tag':16} n  ecapa   emo_d   avd_d   A      V      D     labels           pass")
    for r in rows:
        if r['tag']==base['tag']:
            ok='BASE'
        else:
            ecapa_ok = r['ecapa'] >= base['ecapa'] - FLOOR_DELTA
            emo_ok = (r['emotion_distance'] < base['emotion_distance']) or (r['valence'] > base['valence'] + 1e-6)
            # strict rule from brief: ECAPA >= baseline-0.01 AND valence/emotion distance better
            both_emo = (r['emotion_distance'] < base['emotion_distance']) and (r['valence'] >= base['valence'])
            either = (r['emotion_distance'] < base['emotion_distance']) or (r['valence'] > base['valence'])
            ok = 'PASS' if (ecapa_ok and either) else ('ecapa_fail' if not ecapa_ok else 'emo_fail')
            if ecapa_ok and both_emo:
                ok='PASS*'
        print(f"{r['tag']:16} {r['n']:2d} {r['ecapa']:.4f} {r['emotion_distance']:.4f} {r['avd_distance']:.4f} {r['arousal']:.3f} {r['valence']:.3f} {r['dominance']:.3f} {str(r['labels']):16} {ok}")
    print(f"baseline ecapa floor {base['ecapa']-FLOOR_DELTA:.4f}; ref valence target ~0.698")

if __name__=='__main__':
    tags=sys.argv[1:] or ['jar_old','jd_m1_ln','jd_m2_warm','jd_m3_sim','jd_m4_simc','jd_m5_mup','jd_m6_ln_mup','jd_m7_sim_mup']
    main(tags)
