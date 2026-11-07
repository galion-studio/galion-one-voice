import json,statistics,math
from pathlib import Path
root=Path(__file__).resolve().parent
results=[]
for p in ['samantha','jarvis']:
 b={'samantha':{'wer':.018,'ecapa':.7057,'mos':4.1323,'emotion_distance':.4651},'jarvis':{'wer':.019,'ecapa':.7331,'mos':4.3141,'emotion_distance':.5020}}[p]
 scores=root/'scores'/p
 if not all((scores/f'{name}.json').exists()for name in ['ecapa','wer','emotion','mos']):continue
 e=json.load(open(scores/'ecapa.json'));w=json.load(open(scores/'wer.json'));emo=json.load(open(scores/'emotion.json'));m=json.load(open(scores/'mos.json'));ref=json.load(open(root/f'refs/{p}-blend.json'))
 n=e['n'];wer=w['summary']['mean_raw'];identity=e['mean'];row={'persona':p,'voice':f'own-{p}-v2-blendfix-coherent','n':n,'temperature':0,'dtype':'fp32','timbre_match':.5,'wer':wer,'ecapa':identity,'mos':m['summary']['mos']['mean'],'emotion_distance':emo['summary']['mean_emotion_distance'],'labels':emo['summary']['label_counts'],'selected_start_s':ref['selected_start_s'],'baseline':b,'gate_pass':n==12 and w['summary']['n']==12 and emo['summary']['n_files']==12 and wer<=b['wer'] and identity>b['ecapa'],'source_dir':str(scores),'quality_warning':'MOS is a prediction, not listening; Jarvis baseline NMR differs from candidate, so MOS cross-job comparison is not controlled'}
 results.append(row)
(root/'results12.json').write_text(json.dumps({'schema':'galion-blendfix-validation/1','voices':results,'global_guard_pass':len(results)==2 and all(r['gate_pass']for r in results),'promotion_performed':False},indent=2)+'\n')
md=['# Coherent-prefix validation (authoritative completed-score table)','', '| Voice | n | WER (baseline) | ECAPA (baseline) | MOS | Emotion distance | WER/identity guard |','|---|---:|---:|---:|---:|---:|---|']
for r in results:
 b=r['baseline'];md.append(f'| {r["persona"]} | {r["n"]} | {r["wer"]:.5f} ({b["wer"]:.3f}) | {r["ecapa"]:.4f} ({b["ecapa"]:.4f}) | {r["mos"]:.4f} | {r["emotion_distance"]:.6f} | {"PASS" if r["gate_pass"] else "FAIL"} |')
md+=['','All12-line T0/fp32; no profile promoted. Samantha MOS regression remains material even when its requested WER/identity guard passes. Jarvis MOS uses OTHER-persona NMR whereas merged Jarvis baseline used Jarvis NMR, so these MOS figures are not directly comparable. Full fields, baseline thresholds, source directories and guard booleans: results12.json.']
(root/'VALIDATION.md').write_text('\n'.join(md)+'\n')
for name in ['REPORT.md','REPORT-coherent.md']:
 p=root/name
 if p.exists():
  s=p.read_text()
  if '## Automatically completed validation'not in s:s+='\n\n## Automatically completed validation\n\nThe completed-score table in [VALIDATION.md](VALIDATION.md) and `results12.json` supersedes any earlier pending status above. No global pass unless `global_guard_pass` is true.\n'
  p.write_text(s)
