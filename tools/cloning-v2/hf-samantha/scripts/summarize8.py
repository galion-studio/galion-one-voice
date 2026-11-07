import json,os,sys,statistics as st
B=os.path.dirname(os.path.dirname(os.path.abspath(__file__)))+'/scores8'
def L(v,f): return json.load(open(f'{B}/{v}/{f}'))
rows=[]
for v in sys.argv[1:]:
    e=L(v,'ecapa.json');h=L(v,'hygiene.json');m=L(v,'mos.json');w=L(v,'wer.json');em=L(v,'emotion.json');a=L(v,'artic.json')
    hf=h['files']; ef=em['files']
    wps=st.mean(x['n_ref_words']/y['proxies']['speech_span_s'] for x,y in zip(sorted(w['files'],key=lambda r:r['path']),sorted(ef,key=lambda r:r['file'])))
    lab=em['summary']['label_counts']
    r=dict(v=v,f99=st.mean(x['bandwidth']['f99_hz'] for x in hf),gt4k=100*st.mean(x['bandwidth']['energy_frac_gt_4k'] for x in hf),
      ecapa=e['mean'],wer=w['summary']['mean'],mos=m['summary']['mos']['mean'],
      emo_sim=em['summary']['mean_emotion_similarity'],ang=lab.get('ang',0),neu=lab.get('neu',0),wps=wps,
      f0=a['distribution']['f0_median_hz']['median'],artic=a['distribution']['articulation_rate_syl_s']['mean'],
      fric=a['distribution'].get('fric_centroid_hz',{}).get('mean',float('nan')),clips=sum(x['clipped_samples'] for x in hf),clicks=sum(x['clicks'] for x in hf))
    rows.append(r)
print('| variant | f99 Hz | %E>4k | ECAPA | WER | MOS | emo-sim | ang/neu (of 8) | words/s | F0 med Hz | artic syl/s | fric centroid Hz | clipped/clicks |')
print('|'+'---|'*13)
for r in rows: print(f"| {r['v']} | {r['f99']:.0f} | {r['gt4k']:.2f} | {r['ecapa']:.3f} | {r['wer']:.3f} | {r['mos']:.2f} | {r['emo_sim']:.3f} | {r['ang']}/{r['neu']} | {r['wps']:.2f} | {r['f0']:.0f} | {r['artic']:.2f} | {r['fric']:.0f} | {r['clips']}/{r['clicks']} |")
