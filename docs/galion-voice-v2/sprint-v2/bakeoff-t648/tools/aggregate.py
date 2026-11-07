# Aggregate scores + timings into results.json and markdown tables. Eval only.
import json, glob, re, statistics as st, subprocess, hashlib, os, collections
B='/workspace/swarm/voice-v2-sprint/bakeoff-t648'
def dur(p): return float(subprocess.check_output(['ffprobe','-v','error','-show_entries','format=duration','-of','csv=p=0',p]).decode())
def md5(p): return hashlib.md5(open(p,'rb').read()).hexdigest()
def stats(v):
    return {'n':len(v),'median':round(st.median(v),4),'mean':round(st.mean(v),4),'sd':round(st.stdev(v),4) if len(v)>1 else 0.0,'min':round(min(v),4),'max':round(max(v),4)} if v else None
S={}
for f in glob.glob(f'{B}/out/scores-*.json'): S.update({('/workspace/swarm/voice-v2-sprint/'+k if not k.startswith('/') else k):v for k,v in json.load(open(f)).items()})
WR={}
for f in glob.glob(f'{B}/out/wer-*.json'): WR.update({(B+'/'+k if not k.startswith('/') else k):v for k,v in json.load(open(f)).items()})
OWN={'samantha-orig':('samantha_orig','samantha_heldout'),'samantha-ALT-src-17.84-29.25':('samantha_orig','samantha_heldout'),'jarvis-orig':('jarvis_orig','jarvis_heldout')}
res={}
for opt in ['A','C-fp32','C-quantized']:
    for tgt,(ko,kh) in OWN.items():
        fs=sorted(glob.glob(f'{B}/out/{opt}/{tgt}/s?-r*.wav'))
        if not fs: continue
        rows=[S[f] for f in fs if f in S]
        hashes=collections.Counter(md5(f) for f in fs)
        r={'files':len(fs),'unique_audio':len(hashes),'vs_orig':stats([x[ko] for x in rows]),'vs_heldout':stats([x[kh] for x in rows]),
           'audio_s_mean':round(st.mean(dur(f) for f in fs),2)}
        if tgt.startswith('samantha-ALT'): r['vs_alt_ref_itself']=stats([x['samantha_alt'] for x in rows])
        # unique-only stats (dedupe deterministic repeats)
        seen={};
        for f in fs: seen.setdefault(md5(f),f)
        u=[S[f] for f in seen.values() if f in S]
        r['vs_orig_unique']=stats([x[ko] for x in u]); r['vs_heldout_unique']=stats([x[kh] for x in u])
        wv=[WR[f]['wer'] for f in fs if f in WR]; r['wer']=stats(wv); r['wer_le_0.10_frac']=round(sum(w<=0.10 for w in wv)/len(wv),3) if wv else None
        res[f'{opt}|{tgt}']=r
json.dump(res,open(f'{B}/out/results.json','w'),indent=1)
for k,v in res.items(): print(k, json.dumps(v))
