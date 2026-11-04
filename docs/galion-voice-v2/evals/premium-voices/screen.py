import json,subprocess,sys,numpy as np,random,os
P='/workspace/swarm/voice-v2-sprint/repo/weights/runtime/piper/piper'
M='/workspace/swarm/voice-v2-sprint/models/piper-premium/'
TEXT="I have checked the schedule, and everything is ready for this afternoon."
def f0(x,sr):
    fr=int(sr*0.04);hop=fr//2;lmin=int(sr/400);lmax=int(sr/60);out=[]
    for s in range(0,len(x)-fr-lmax,hop):
        seg=x[s:s+fr+lmax].astype(np.float64);a=seg[:fr]
        e0=(a*a).sum()
        if e0/fr<300**2: continue
        best=(0,0)
        for lag in range(lmin,lmax+1):
            b=seg[lag:lag+fr];r=(a*b).sum()/max(np.sqrt(e0*(b*b).sum()),1)
            if r>best[0]: best=(r,lag)
        if best[0]>0.6: out.append(sr/best[1])
    return float(np.median(out)) if out else 0.0
def synth_many(model,ids):
    cfg=json.load(open(M+model+'.onnx.json'));sr=cfg['audio']['sample_rate']
    res={}
    for sid in ids:
        r=subprocess.run([P,'--model',M+model+'.onnx','--output_raw','--json-input','--noise_scale','0.667','--noise_w','0.8'],input=json.dumps({"text":TEXT,"speaker_id":sid}).encode(),capture_output=True)
        x=np.frombuffer(r.stdout,dtype=np.int16)
        res[sid]=f0(x[::2],sr//2) if len(x) else 0
    return res,cfg
for model,n in [('en_GB-vctk-medium',None),('en_GB-aru-medium',None),('en_US-libritts_r-medium',60),('en_US-libritts-high',40)]:
    cfg=json.load(open(M+model+'.onnx.json'));smap=cfg.get('speaker_id_map',{})
    inv={v:k for k,v in smap.items()}
    ids=sorted(inv)
    if n: random.seed(7); ids=sorted(random.sample(ids,n))
    res,_=synth_many(model,ids)
    with open(f'screen-{model}.tsv','w') as f:
        for sid in ids: f.write(f"{sid}\t{inv[sid]}\t{res[sid]:.1f}\n")
    print(model,'done',flush=True)
