# Eval only (not shipped): ECAPA cosine of each clone wav vs its CC0 donor ref.
# Same model/recipe as tools/spk_sim.py (speechbrain ECAPA, Apache-2.0); refs amy/alan/pdx only.
import sys, json, glob, os, torch, torchaudio
from speechbrain.lobes.features import Fbank
from speechbrain.processing.features import InputNormalization
from speechbrain.lobes.models.ECAPA_TDNN import ECAPA_TDNN
W = '/workspace/swarm/voice-v2-sprint'; torch.set_num_threads(2)
fb = Fbank(n_mels=80); norm = InputNormalization(norm_type='sentence', std_norm=False)
mdl = ECAPA_TDNN(input_size=80, channels=[1024,1024,1024,1024,3072], kernel_sizes=[5,3,3,3,1], dilations=[1,2,3,4,1], attention_channels=128, lin_neurons=192)
mdl.load_state_dict(torch.load(f'{W}/ecapa-local/embedding_model.ckpt', map_location='cpu')); mdl.eval()
def emb(p):
    w, sr = torchaudio.load(p); w = w.mean(0, keepdim=True)
    if sr != 16000: w = torchaudio.functional.resample(w, sr, 16000)
    with torch.no_grad():
        f = fb(w); f = norm(f, torch.ones(1)); return torch.nn.functional.normalize(mdl(f).squeeze(), dim=0)
refs = {k: emb(f'{W}/refs/{k}.wav') for k in ['amy', 'alan', 'pdx']}
for d in sys.argv[1:]:
    res = {}
    for f in sorted(glob.glob(f'{d}/*.wav')):
        v = os.path.basename(f).split('-')[0]; res[os.path.basename(f)] = round(float((emb(f) * refs[v]).sum()), 4)
    per = {v: [x for k, x in res.items() if k.startswith(v + '-')] for v in refs}
    longs = {v: [x for k, x in res.items() if k.startswith(v + '-') and int(k.split('-')[1][0]) < 3] for v in refs}
    out = dict(files=res, min={v: min(x) for v, x in per.items() if x}, mean={v: round(sum(x)/len(x), 4) for v, x in per.items() if x},
               long_min={v: min(x) for v, x in longs.items() if x}, long_mean={v: round(sum(x)/len(x), 4) for v, x in longs.items() if x})
    json.dump(out, open(f'{d}/similarity.json', 'w'), indent=1); print(os.path.basename(d), json.dumps({k: out[k] for k in ('mean', 'min', 'long_mean', 'long_min')}))
