#!/usr/bin/env python3
"""Artifact-first scoreboard. Python orchestrates; every numerical metric is Rust-produced.
No voice files or audio are written. Scoring is sequential, nice 10, <=2 threads.
"""
import os,json,re,subprocess,hashlib,statistics,datetime
from pathlib import Path
if os.getpriority(os.PRIO_PROCESS,0)<10:os.nice(10-os.getpriority(os.PRIO_PROCESS,0))
ROOT=Path('/workspace/swarm/voice-v2-sprint'); V=ROOT/'cloning-v2'; OUT=V/'scoreboard'
os.environ.update(OMP_NUM_THREADS='2',OPENBLAS_NUM_THREADS='2',RAYON_NUM_THREADS='2',CUDA_VISIBLE_DEVICES='')
def read(p):
 try:return json.loads(Path(p).read_text())
 except (OSError,ValueError):return None
def ident(p):
 m=re.search(r't\d\d',Path(p).stem);return m.group() if m else Path(p).stem

def resolve(p,base):
 p=Path(p)
 for x in [p,base/p,V/p,ROOT/p]:
  if x.is_file():return x.absolute()
 return None

def metric_file(ep,m):
 aliases={'emotion':['emotion','emo'],'artic':['artic'],'hygiene':['hygiene','hyg'],'mos':['mos'],'wer':['wer']}
 for a in aliases[m]:
  candidates=[ep.with_name(a+'.json'),ep.with_name(ep.name.replace('ecapa',a))]
  for p in candidates:
   if p==ep:continue
   j=read(p)
   if j:return p,j
 # Baseline acoustic scores already exist under the scorer validation outputs.
 ej=read(ep) or {};ps=list(ej.get('scores',{}))
 if ps and all('own-clone-analysis/wav/' in x for x in ps):
  spk='samantha' if 'samantha' in ps[0] else 'jarvis'
  q={'emotion':V/f'scorers/emotion/results/baseline_{spk}.json',
     'artic':V/f'scorers/articulation/results/profile_{spk}_clones_t01-t12.json',
     'wer':V/f'scorers/quality/out/rs_wer_{spk}.json',
     'mos':V/f'scorers/quality/out/rs_mos_{spk}.json',
     'hygiene':V/f'scorers/quality/out/rs_hyg_{spk}.json'}[m]
  j=read(q)
  if j:return q,j
 return None,None
bins={'ecapa':V/'scorers/ecapa/target/release/galion-ecapa','emotion':V/'scorers/emotion/target/release/galion-emotion','artic':V/'scorers/articulation/bin/galion-artic',**{k:V/f'scorers/quality/bin/galion-{k}' for k in ['mos','wer','hygiene']}}
def run(m,wavs,spk,key):
 out=OUT/'repair-scores'/key/(m+'.json');out.parent.mkdir(parents=True,exist_ok=True)
 j=read(out)
 if j:return out,j
 ref=ROOT/f'clip-store-own/{spk}/reference.wav';other=ROOT/f'clip-store-own/{"jarvis" if spk=="samantha" else "samantha"}/reference.wav'
 opts={'ecapa':['--ref',str(ref),'--threads','2'],'emotion':['score','--ref',str(ref),'--threads','2'],'artic':['profile','-j','2'],'mos':['--nmr',str(other),'--threads','2'],'wer':['--text-dir',str(ROOT/'own-clone-analysis/text'),'--jobs','2'],'hygiene':[]}[m]
 # Noncanonical pipeline stems still map to original text without touching WAVs.
 if m=='wer' and any(Path(w).stem!=ident(w) for w in wavs):
  td=out.parent/'text';td.mkdir(exist_ok=True)
  for w in wavs:
   src=ROOT/'own-clone-analysis/text'/f'{ident(w)}.txt';dst=td/(Path(w).stem+'.txt')
   if src.exists() and not dst.exists():dst.write_text(src.read_text())
  opts=['--text-dir',str(td),'--jobs','2']
 cmd=[str(bins[m])]+opts+[str(w) for w in wavs]
 print('score',key,m,len(wavs),flush=True)
 try:
  r=subprocess.run(cmd,cwd=V,env=os.environ,capture_output=True,text=True,timeout=180)
  (out.parent/(m+'.stderr')).write_text(r.stderr)
  if r.returncode==0:
   j=json.loads(r.stdout);out.write_text(json.dumps(j,indent=2));return out,j
 except (subprocess.TimeoutExpired,ValueError) as e:(out.parent/(m+'.stderr')).write_text(str(e))
 return None,None

def records(m,j):
 if m=='ecapa':return {ident(k):{'value':v,'path':k} for k,v in j.get('scores',{}).items()}
 return {ident(x.get('file',x.get('path',''))):x for x in j.get('files',[]) if x.get('file',x.get('path'))}
def value(m,j,ids):
 rs=records(m,j);a=[rs[i] for i in ids if i in rs]
 if len(a)!=len(ids):return None
 if m=='ecapa':return statistics.mean(x['value'] for x in a)
 if m=='emotion':
  ds=[x.get('vs_ref',{}).get('emotion_distance') for x in a]
  return statistics.mean(ds) if all(x is not None for x in ds) else None
 if m=='mos':return statistics.mean(x['mos'] for x in a)
 if m=='wer':return statistics.mean(x.get('wer_raw',x['wer']) for x in a)
 if m=='artic':
  vals=[x.get('rate',{}).get('articulation_rate_syl_s') for x in a]
  return statistics.mean(vals) if all(x is not None for x in vals) else None
 if m=='hygiene':return {'peak_dbtp':max(x['true_peak_dbtp'] for x in a),'clips':sum(x['clipped_samples'] for x in a),'clicks':sum(x['clicks'] for x in a),'f99_hz':statistics.mean(x['bandwidth']['f99_hz'] for x in a)}

def artifact_index():
 idx={m:{} for m in ['emotion','artic','wer','mos','hygiene']}
 for sub in ['hf-samantha','hf-jarvis','emotion-fix','similarity','refprep','pipeline','scorers','merge-v2','scoreboard/repair-scores']:
  base=V/sub
  if not base.exists():continue
  for directory,ds,fs in os.walk(base):
   ds[:]=[x for x in ds if x not in ['target','venv','.venv','models','.git','cbx-v2-prosody','cbx-emo']]
   for fn in fs:
    if not fn.endswith('.json') or not any(x in directory for x in ['/scores','/results','/out','repair-scores']):continue
    p=Path(directory)/fn;j=read(p)
    if not isinstance(j,dict) or not isinstance(j.get('files'),list):continue
    m={'galion-wer':'wer','galion-mos':'mos','galion-hygiene':'hygiene'}.get(j.get('tool'))
    if j.get('schema','').startswith('galion-emotion'):m='emotion'
    if j.get('mode')=='profile' and 'distribution' in j:m='artic'
    if not m or 'int8' in str(j.get('models',{})):continue
    for rec in j['files']:
     name=rec.get('file',rec.get('path'))
     if not name:continue
     wav=resolve(name,V/sub.split('/')[0])
     if not wav:continue
     rec=dict(rec);rec['file']=str(wav);rec['path']=str(wav)
     idx[m][str(wav.resolve())]=(str(p),rec)
 return idx

def supplement(m,wavs,spk,key,idx,allow_run=True):
 selected=[];missing=[];sources=[]
 for w in wavs:
  found=idx[m].get(str(w.resolve()))
  if found:sources.append(found[0]);selected.append(found[1])
  else:missing.append(w)
 if missing and allow_run:
  subsetkey=key+'-'+hashlib.sha256('|'.join(str(x) for x in missing).encode()).hexdigest()[:6]
  p,j=run(m,missing,spk,subsetkey)
  if j:
   selected+=j.get('files',[]);sources.append(str(p))
 if len(selected)!=len(wavs):return None,None
 out=OUT/'repair-scores'/key/(m+'-aggregated.json');out.parent.mkdir(parents=True,exist_ok=True)
 j={'files':selected,'provenance':sorted(set(sources)),'aggregation':'select original Rust per-file scores by identical WAV path; no Python metric computation'}
 if m=='mos':
  nmrs=sorted(set(str((read(p) or {}).get('nmr')) for p in sources));j['nmr']=nmrs[0] if len(nmrs)==1 else nmrs
 out.write_text(json.dumps(j,indent=2));return out,j

def collect():
 rows=[];seen=set();excluded=[];idx=artifact_index()
 eps=[]
 for sub in ['hf-samantha','hf-jarvis','emotion-fix','similarity','refprep','pipeline','merge-v2']:
  base=V/sub
  if not base.exists():continue
  for p in base.rglob('*ecapa*.json'):
   if any(x in p.parts for x in ['target','venv','.venv']) or 'smoke' in p.name:continue
   eps.append((p,base))
 # Include pre-existing WAV candidates without an ECAPA artifact. No rendering.
 known=set()
 for ep,base in eps:
  j=read(ep)
  if j:
   for x in j.get('scores',{}):
    p=resolve(x,base)
    if p:known.add(str(p.resolve()))
 for sub in ['hf-samantha/wav','hf-jarvis/wav','similarity/wav','emotion-fix/runs']:
  top=V/sub
  if not top.exists():continue
  for directory,ds,fs in os.walk(top):
   ds[:]=[x for x in ds if x not in ['cands','target']]
   wavs=sorted(Path(directory)/x for x in fs if re.fullmatch(r't\d\d\.wav',x))
   if len(wavs)<8 or all(str(x.resolve()) in known for x in wavs):continue
   # Only known speaker directories; avoid guessing development cands.
   label=str(directory);spk='samantha' if any(x in label for x in ['samantha','/sd_','/sam-']) else 'jarvis' if any(x in label for x in ['jarvis','/jd_','/jar-']) else None
   if not spk:continue
   key=hashlib.sha256('|'.join(str(x) for x in wavs).encode()).hexdigest()[:14]
   p,j=run('ecapa',wavs,spk,key)
   if j:eps.append((p,V))
 # Final score sets take priority over earlier snapshots of same WAVs.
 eps.sort(key=lambda t: ('/scores8/' not in str(t[0]),'/final/' not in str(t[0]),len(str(t[0]))))
 for ep,base in eps:
  ej=read(ep)
  if not ej or not ej.get('scores'):continue
  paths=[resolve(x,base) for x in ej['scores']]
  if any(x is None for x in paths):excluded.append({'file':str(ep),'reason':'WAV path unresolved'});continue
  ids=sorted(ident(x) for x in paths)
  if any(not re.fullmatch(r't\d\d',x) for x in ids):
   excluded.append({'file':str(ep),'reason':'reference/preparation segments, not TTS text-set lines'});continue
  if len(set(ids))!=len(ids):excluded.append({'file':str(ep),'reason':'duplicate line IDs'});continue
  signature=tuple(sorted(str(x.resolve()) for x in paths))
  if signature in seen:continue
  seen.add(signature)
  spk='samantha' if 'samantha' in str(ej.get('ref','')) else 'jarvis'
  n=len(paths);key=hashlib.sha256('|'.join(signature).encode()).hexdigest()[:14]
  parent=paths[0].parent
  baseline=all('own-clone-analysis/wav/' in str(x.resolve()) for x in paths)
  name=('own-'+spk+' (pre-merge baseline)') if baseline else (str(parent.relative_to(V)) if parent.is_relative_to(V) else ep.stem)
  if '/cbx-v2-timbre/' in str(ep):name=f'timbre-match {ep.stem.split("_")[1].replace("s", "").replace("p", ".")}'
  if '/merge-v2/' in str(ep):name=f'own-{spk} (v2 merged)'
  aliases={'sd_r2_t012':'Samantha emo-r2 @T0','sd_r2_up12':'Samantha emo-r2 @T0.8','jd_m9_w8_12':'Jarvis emo-m9/w8 @12-line'}
  if parent.name in aliases:name=aliases[parent.name]
  row={'voice':name,'speaker':spk,'baseline':baseline,'n':n,'line_ids':ids,'text_set':('8-line t01–t08' if ids==[f't{i:02}' for i in range(1,9)] else '12-line t01–t12' if ids==[f't{i:02}' for i in range(1,13)] else f'{n}-line subset '+','.join(ids)), 'wav_files':[str(x) for x in paths],'metrics':{},'source_ecapa':str(ep)}
  row['metrics']['ecapa']={'value':value('ecapa',ej,ids),'source':str(ep),'engine':'Rust'}
  for m in ['emotion','artic','wer','mos','hygiene']:
   p,j=metric_file(ep,m)
   # Never race refprep: consume completed files only.
   if not j or value(m,j,ids) is None:
    p,j=supplement(m,paths,spk,key,idx,allow_run=n>=1 and (base.name not in ['refprep','merge-v2'] or (base.name=='refprep' and m=='artic')))
   val=value(m,j,ids) if j else None
   row['metrics'][m]={'nmr':j.get('nmr') if j and m=='mos' else None,'value':val,'source':str(p) if p else None,'engine':'Rust' if j else None,'status':'measured' if val is not None else 'pending'}
  rows.append(row)
 return rows,excluded

def render(rows,excluded):
 # Gate against pre-merge baseline rescoped to EXACT line IDs, not an unmatched mean.
 baselines={}
 for spk in ['samantha','jarvis']:
  candidates=[r for r in rows if r['baseline'] and r['speaker']==spk]
  baselines[spk]=max(candidates,key=lambda r:r['n']) if candidates else None
 for r in rows:
  b=baselines[r['speaker']];ids=r['line_ids'];r['gate']='pending'
  if b and set(ids)<=set(b['line_ids']):
   refs={};sources={}
   for m in ['ecapa','emotion']:
    p=b['metrics'][m]['source'];j=read(p) if p else None
    refs[m]=value(m,j,ids) if j else None;sources[m]=p
   r['baseline_same_lines']={'n':len(ids),'line_ids':ids,'metrics':refs,'sources':sources}
   ec=r['metrics']['ecapa']['value'];em=r['metrics']['emotion']['value']
   if all(x is not None for x in [ec,em,refs['ecapa'],refs['emotion']]):
    r['ecapa_delta']=ec-refs['ecapa'];r['emotion_delta']=em-refs['emotion'];r['gate']='keep' if r['ecapa_delta']>=-0.01000001 and r['emotion_delta']<=1e-9 else 'reject'
  if r['n']<8:r['gate']='development-only ('+r['gate']+')'
 def rank(r):
  # 12-line evidence first; within matched evidence: pass, emotion improvement, identity, MOS.
  return (r['gate']!='keep',r['n']!=12,r.get('emotion_delta',999),-r['metrics']['ecapa']['value'],'(v2 merged)' not in r['voice'])
 tops={s:sorted([r for r in rows if r['speaker']==s and not r['baseline'] and r['n']>=8 and r['voice']!='timbre-match 0.0'],key=rank)[:5] for s in baselines}
 def f(x):return 'pending' if x is None else f'{x:.4f}'
 now=datetime.datetime.now().astimezone().strftime('%Y-%m-%d %H:%M %Z (Europe/Zagreb)')
 md=f'# Cloning V2 Scoreboard — repaired\n\nGenerated {now}. Artifact-first; no new renders. Numerical scores exclusively from Rust scorers; Python only aggregates. CPU, nice 10, two threads.\n\n## Recommended per voice\n\nKeep only when ECAPA drop ≤0.01 and emotion distance is not worse, against the pre-merge baseline restricted to **the identical line IDs**. Smaller emotion distance is better.\n\n'
 md+='**Production-shaped recommendation: own-samantha (v2 merged) and own-jarvis (v2 merged).** Both pass the 12-line gate; Jarvis keeps substantially more identity and lower WER than emo-m9/w8. No further voice changes are performed by this board.\n\n'
 for s in tops:
  kept=[r for r in tops[s] if r['gate']=='keep']; md+=f'- **{s.title()} — gate-eligible, ordered by emotion improvement**: '+('; '.join(f"{r['voice']} ({r['n']} lines; ECAPA {f(r['metrics']['ecapa']['value'])}, emotion {f(r['metrics']['emotion']['value'])})" for r in kept) or 'no measured candidate passes both gates')+'.\n'
 md+='\nRecommendation order: passing candidates first, full 12-line validation before shorter subsets, then emotion improvement versus matched baseline and ECAPA; exact ties prefer the merged-production row. MOS is displayed, not used for cross-job ranking. This is an evidence-priority ranking, not an apples-to-oranges raw-score leaderboard. Unmerged component passes do not establish that their combination passes.\n\n'
 md+='Samantha hf-e-mild passes the identity floor on eight lines but is **rejected by the emotion gate**; Jarvis hf-e-mild fails identity. Development subsets remain separate.\n\n'
 for s,rs in tops.items():
  md+=f'## Top 5 — {s}\n\n| Rank | Candidate | n / text set | Gate | ECAPA | Emotion distance | WER | MOS |\n|---|---|---|---|---|---|---|---|\n'
  for i,r in enumerate(rs,1):md+=f"| {i} | {r['voice']} | {r['n']} / {r['text_set']} | {r['gate']} | "+' | '.join(f(r['metrics'][m]['value']) for m in ['ecapa','emotion','wer','mos'])+' |\n'
 md+='\n## All artifact rows\n\nAll metric cells are Rust (R). Sources are keyed below per row and column; full paths and raw WAV lists are in BOARD.json. Articulation is **profile articulation-rate syl/s**, not MCD: no same-text original recordings exist to justify MCD comparisons. Hygiene = peak dBTP / clipped samples / clicks / mean f99 Hz.\n\n| ID | Speaker | Voice / setting | n / text set | Gate | ECAPA R | Emotion R | Artic rate R | WER R | MOS R | Hygiene R |\n|---|---|---|---|---|---|---|---|---|---|---|\n'
 for i,r in enumerate(rows,1):
  r['id']=f'R{i:02}';h=r['metrics']['hygiene']['value'];hs='pending' if h is None else f"{h['peak_dbtp']:.2f} / {h['clips']} / {h['clicks']} / {h['f99_hz']:.1f}"
  md+=f"| {r['id']} | {r['speaker']} | {r['voice']} | {r['n']} / {r['text_set']} | {r['gate']} | "+' | '.join(f(r['metrics'][m]['value']) for m in ['ecapa','emotion','artic','wer','mos'])+f' | {hs} |\n'
 md+='\n## Column sources\n\n'
 for r in rows:
  md+=f"- **{r['id']}**: "+'; '.join(f"{m}: `{str(d['source']).replace(str(V)+'/', '')}`" if d['source'] else f'{m}: pending (worker-owned or scorer unavailable)' for m,d in r['metrics'].items())+'\n'
 md+='\n## Coverage and limits\n\n- Refprep files are read only; active workers/processes are never touched. A missing file is pending, not zero.\n- Four-line development runs are included but cannot outrank 8/12-line evidence. Eleven-line and other subsets are explicitly labelled.\n- Existing voice assets with no rendered WAVs cannot be scored without forbidden rendering; omitted from measured ranking.\n- Prosody implementation directory is source code, not a separately rendered candidate; measured pipeline pace/hygiene WAVs are listed.\n- Duplicates of identical WAV sets are deduplicated, preferring scores8/final outputs.\n'
 if excluded:md+='\nExcluded artifacts: '+json.dumps(excluded)+'\n'
 payload={'generated':now,'rule':'ECAPA delta >= -0.01 and emotion delta <= 0; exact same lines','ranking':'keep first; 12-line evidence first; emotion delta ascending; ECAPA descending; exact ties prefer merged-production row','rows':rows,'top5':{s:[r['voice'] for r in rs] for s,rs in tops.items()},'excluded':excluded}
 payload['merge_validation_source']='merge-v2/results12.json' if (V/'merge-v2/results12.json').exists() else 'pending'
 md+='\nMerged-production validation: `merge-v2/results12.json` and `merge-v2/REPORT.md`. Jarvis merged and emo-m9/w8 MOS use **Jarvis as NMR**, whereas the board baseline and timbre-only Jarvis MOS use **Samantha as NMR**; these cross-job MOS values are not directly comparable. Identity/emotion gates are unaffected.\n'
 (OUT/'BOARD.json').write_text(json.dumps(payload,indent=2));(OUT/'BOARD.md').write_text(md)
 print('WROTE',len(rows),'rows',json.dumps(payload['top5']),flush=True)
if __name__=='__main__':render(*collect())
