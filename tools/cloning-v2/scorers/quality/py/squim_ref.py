# Python cross-check: torchaudio SQUIM subjective MOS + objective (STOI/PESQ/SI-SDR) on own-clone-analysis clips.
# NMR convention (own-clone-analysis): the OTHER persona's reference.wav.
import torch, torchaudio, json, sys, time, glob, os
torch.set_num_threads(2)
S='/workspace/swarm/voice-v2-sprint'
sub=torchaudio.pipelines.SQUIM_SUBJECTIVE.get_model().eval()
obj=torchaudio.pipelines.SQUIM_OBJECTIVE.get_model().eval()
def load(p):
    w,sr=torchaudio.load(p); w=w.mean(0,keepdim=True)
    return torchaudio.functional.resample(w,sr,16000) if sr!=16000 else w
refs={p:load(f'{S}/clip-store-own/{p}/reference.wav') for p in ('samantha','jarvis')}
other={'samantha':'jarvis','jarvis':'samantha'}
out={}; t0=time.time()
with torch.inference_mode():
  for p in ('samantha','jarvis'):
    files=sorted(glob.glob(f'{S}/own-clone-analysis/wav/{p}/*.wav'))+[f'{S}/clip-store-own/{p}/reference.wav']
    for f in files:
        w=load(f); m=sub(w,refs[other[p]]).item(); s,q,d=[x.item() for x in obj(w)]
        out[f]={'mos':m,'stoi':s,'pesq':q,'si_sdr':d}
out['_elapsed_s']=time.time()-t0
json.dump(out,open(sys.argv[1],'w'),indent=1); print('done',out['_elapsed_s'])
