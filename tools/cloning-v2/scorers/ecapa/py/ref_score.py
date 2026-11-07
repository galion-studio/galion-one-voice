# Python reference: emb/cos copied VERBATIM from bakeoff-t648/tools/score_t648.py; refs = clip-store-own.
# usage: ref_score.py out.json --ref ref.wav f1.wav ...  (half-vs-half ceiling = first half vs second half of 16k ref)
import json, sys, time, torch, torchaudio, numpy as np
t0=time.time(); c0=time.process_time()
from speechbrain.lobes.features import Fbank
from speechbrain.processing.features import InputNormalization
from speechbrain.lobes.models.ECAPA_TDNN import ECAPA_TDNN
torch.set_num_threads(2)
W='/workspace/swarm/voice-v2-sprint'
fb=Fbank(n_mels=80); norm=InputNormalization(norm_type='sentence',std_norm=False)
mdl=ECAPA_TDNN(input_size=80,channels=[1024,1024,1024,1024,3072],kernel_sizes=[5,3,3,3,1],dilations=[1,2,3,4,1],attention_channels=128,lin_neurons=192)
mdl.load_state_dict(torch.load(f'{W}/ecapa-local/embedding_model.ckpt',map_location='cpu')); mdl.eval()
def load(p):
    w,sr=torchaudio.load(p); w=w.mean(0,keepdim=True)
    if sr!=16000: w=torchaudio.functional.resample(w,sr,16000)
    return w
def embw(w):
    with torch.no_grad():
        f=fb(w); f=norm(f,torch.ones(f.shape[0])); return torch.nn.functional.normalize(mdl(f).squeeze(),dim=0)
emb=lambda p: embw(load(p))
cos=lambda a,b: round(float((a*b).sum()),4)
a=sys.argv[1:]; out=a[0]; i=a.index('--ref'); ref=a[i+1]; files=a[i+2:]
tl=time.time()
rw=load(ref); r=emb(ref); h=rw.shape[1]//2
res={'ref':ref,'half_vs_half_ceiling':cos(embw(rw[:,:h]),embw(rw[:,h:])),'scores':{}}
for p in files: res['scores'][p]=cos(emb(p),r)
v=list(res['scores'].values()); res['mean']=round(sum(v)/len(v),4) if v else None
res['timing']={'wall_s_total':round(time.time()-t0,3),'wall_s_scoring':round(time.time()-tl,3),'cpu_s_total':round(time.process_time()-c0,3)}
json.dump(res,open(out,'w'),indent=1); print(res['mean'],res['half_vs_half_ceiling'],res['timing'])
if '--dump' in sys.argv or True:
    w=load(files[0] if files else ref); f=fb(w); np.save('out/py_wav16.npy',w.numpy()); np.save('out/py_fbank.npy',f.numpy())
