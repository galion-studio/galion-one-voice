#!/usr/bin/env python3
"""Table from scores/<tag>/*.json (all numbers read from scorer outputs)."""
import json, sys, os, statistics as st
root = os.environ.get('SCORES_ROOT', os.path.dirname(os.path.abspath(__file__)) + '/scores')
def row(tag):
    L = lambda f: json.load(open(f'{root}/{tag}/{f}.json'))
    e, w, m, h, em = L('ecapa'), L('wer'), L('mos'), L('hygiene'), L('emotion')
    f99 = [f['bandwidth']['f99_hz'] for f in h['files']]; g4 = [100*f['bandwidth']['energy_frac_gt_4k'] for f in h['files']]
    s = em['summary']
    return dict(tag=tag, n=e['n'], ecapa=e['mean'], ecapa_sd=e['sd'], wer=w['summary']['mean'], mos=m['summary']['mos']['mean'],
                emo_dist=round(s['mean_emotion_distance'], 4), labels=s['label_counts'], f99=round(st.mean(f99)), pct_gt4k=round(st.mean(g4), 2),
                clicks=sum(f['clicks'] for f in h['files']), clipped=sum(f['clipped_samples'] for f in h['files']),
                dur_s=round(sum(f['duration_s'] for f in h['files']), 1))
tags = sys.argv[1:] or sorted(os.listdir(root))
print('| tag | n | ECAPA ↑ | WER ↓ | MOS ↑ | emotion dist ↓ | SER labels | f99 Hz | %E>4k | clicks/clipped | audio s |\n|---|---|---|---|---|---|---|---|---|---|---|')
for t in tags:
    try: r = row(t)
    except Exception as ex: print(f'| {t} | missing ({ex}) |'); continue
    lab = ', '.join(f'{k} {v}' for k, v in sorted(r['labels'].items()))
    print(f"| {t} | {r['n']} | {r['ecapa']:.3f}±{r['ecapa_sd']:.3f} | {r['wer']:.3f} | {r['mos']:.2f} | {r['emo_dist']:.3f} | {lab} | {r['f99']} | {r['pct_gt4k']} | {r['clicks']}/{r['clipped']} | {r['dur_s']} |")
