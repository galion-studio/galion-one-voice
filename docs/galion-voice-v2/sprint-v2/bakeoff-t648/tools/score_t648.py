# Eval only (not shipped). Loader copied VERBATIM from tools/spk_sim_cc0_t628u.py (same ECAPA ckpt,
# Fbank n_mels=80, sentence InputNormalization, mono mean, resample to 16 kHz) so numbers are comparable.
# usage: python score_t648.py out.json  wav1 wav2 ...   -> cos vs every ref in REFS
import json, sys, torch, torchaudio
from speechbrain.lobes.features import Fbank
from speechbrain.processing.features import InputNormalization
from speechbrain.lobes.models.ECAPA_TDNN import ECAPA_TDNN
torch.set_num_threads(2)
W='/workspace/swarm/voice-v2-sprint'; B=f'{W}/bakeoff-t648'
fb=Fbank(n_mels=80); norm=InputNormalization(norm_type='sentence',std_norm=False)
mdl=ECAPA_TDNN(input_size=80,channels=[1024,1024,1024,1024,3072],kernel_sizes=[5,3,3,3,1],dilations=[1,2,3,4,1],attention_channels=128,lin_neurons=192)
mdl.load_state_dict(torch.load(f'{W}/ecapa-local/embedding_model.ckpt',map_location='cpu')); mdl.eval()
def emb(p):
    w,sr=torchaudio.load(p); w=w.mean(0,keepdim=True)
    if sr!=16000: w=torchaudio.functional.resample(w,sr,16000)
    with torch.no_grad():
        f=fb(w); f=norm(f,torch.ones(f.shape[0])); return torch.nn.functional.normalize(mdl(f).squeeze(),dim=0)
cos=lambda a,b: round(float((a*b).sum()),4)
REFS={'samantha_orig':f'{W}/clip-store-cc0/samantha/reference.wav','jarvis_orig':f'{W}/clip-store-cc0/jarvis/reference.wav',
      'samantha_alt':f'{B}/refs/samantha-ALT-src-17.84-29.25.wav',
      'samantha_heldout':f'{B}/refs/samantha-heldout-29.60-38.20.wav','jarvis_heldout':f'{B}/refs/jarvis-heldout-43.30-53.60.wav'}
R={k:emb(v) for k,v in REFS.items()}
out={}
for p in sys.argv[2:]:
    e=emb(p); out[p]={k:cos(e,r) for k,r in R.items()}
json.dump(out,open(sys.argv[1],'w'),indent=1); print(len(out),'scored')
