#!/usr/bin/env python3
"""Python FALLBACK scorer until cloning-v2/scorers/{ecapa,emotion}/READY.
ECAPA path mirrors scorers/ecapa/py/ref_score.py. Emotion: superb SER KL + F0/rate distance.
"""
from __future__ import annotations
import json, os, sys, time
from pathlib import Path
import numpy as np
import torch, torchaudio

torch.set_num_threads(2)
W = Path('/workspace/swarm/voice-v2-sprint')
BASE = W / 'cloning-v2/similarity'
from speechbrain.lobes.features import Fbank
from speechbrain.processing.features import InputNormalization
from speechbrain.lobes.models.ECAPA_TDNN import ECAPA_TDNN

fb = Fbank(n_mels=80)
norm = InputNormalization(norm_type='sentence', std_norm=False)
mdl = ECAPA_TDNN(input_size=80, channels=[1024,1024,1024,1024,3072],
                 kernel_sizes=[5,3,3,3,1], dilations=[1,2,3,4,1],
                 attention_channels=128, lin_neurons=192)
mdl.load_state_dict(torch.load(W/'ecapa-local/embedding_model.ckpt', map_location='cpu'))
mdl.eval()

def load16(p):
    w, sr = torchaudio.load(p)
    w = w.mean(0, keepdim=True)
    if sr != 16000:
        w = torchaudio.functional.resample(w, sr, 16000)
    return w

def emb(w):
    with torch.no_grad():
        f = fb(w); f = norm(f, torch.ones(f.shape[0]))
        return torch.nn.functional.normalize(mdl(f).squeeze(), dim=0)

def cos(a,b):
    return float((a*b).sum())

def f0_median(w16):
    # cheap yin via librosa if available else zero-cross proxy skip
    try:
        import librosa
        y = w16.squeeze().numpy()
        f0 = librosa.yin(y, fmin=60, fmax=400, sr=16000)
        f0 = f0[np.isfinite(f0)]
        return float(np.median(f0)) if len(f0) else None
    except Exception:
        return None

def speaking_rate(path, n_words):
    info = torchaudio.info(str(path))
    dur = info.num_frames / info.sample_rate
    return n_words / max(dur, 1e-3)

# optional SER for emotion distance
ser = None
try:
    from transformers import AutoModelForAudioClassification, AutoFeatureExtractor
    name = 'superb/wav2vec2-base-superb-er'
    feat = AutoFeatureExtractor.from_pretrained(name)
    ser_m = AutoModelForAudioClassification.from_pretrained(name)
    ser_m.eval()
    def ser_probs(w16):
        inputs = feat(w16.squeeze().numpy(), sampling_rate=16000, return_tensors='pt', padding=True)
        with torch.no_grad():
            logits = ser_m(**inputs).logits[0]
            p = torch.softmax(logits, dim=-1).cpu().numpy()
        return p
    ser = ser_probs
except Exception as e:
    print('SER unavailable:', e, file=sys.stderr)

def kl(p, q, eps=1e-8):
    p = np.clip(p, eps, 1); q = np.clip(q, eps, 1)
    p = p/p.sum(); q = q/q.sum()
    return float(np.sum(p * np.log(p/q)))

def main():
    t0 = time.time()
    manifest = json.load(open(BASE/'benchmark_manifest.json'))
    words = {m['id']: m['n_words'] for m in manifest}
    configs = sorted([p.name for p in (BASE/'wav').iterdir() if p.is_dir()])
    out = {'fallback': True, 'configs': {}, 'timing': {}}
    refs = {
        'sam': str(BASE/'inputs/samantha-reference.wav'),
        'jar': str(BASE/'inputs/jarvis-reference.wav'),
    }
    ref_emb = {k: emb(load16(v)) for k,v in refs.items()}
    ref_ser = {}
    ref_f0 = {}
    if ser:
        for k,v in refs.items():
            w = load16(v)
            ref_ser[k] = ser(w)
            ref_f0[k] = f0_median(w)

    for cfg in configs:
        persona = 'sam' if cfg.startswith('sam') else 'jar'
        r = ref_emb[persona]
        scores = []
        per = {}
        for tid in [f't{i:02d}' for i in range(1,13)]:
            wav = BASE/'wav'/cfg/f'{tid}.wav'
            if not wav.exists() or wav.stat().st_size < 1000:
                continue
            w = load16(wav)
            e = emb(w)
            sim = cos(e, r)
            entry = {'ecapa': round(sim,4), 'rate_wps': round(speaking_rate(wav, words[tid]),3)}
            entry['f0_median'] = f0_median(w)
            if ser:
                p = ser(w)
                entry['emotion_kl_to_ref'] = round(kl(ref_ser[persona], p),4)
                entry['emotion_distance'] = entry['emotion_kl_to_ref']  # alias for best-of-N
            per[tid] = entry
            scores.append(sim)
        if not scores:
            continue
        arr = np.array(scores)
        out['configs'][cfg] = {
            'n': len(scores),
            'ecapa_mean': round(float(arr.mean()),4),
            'ecapa_sd': round(float(arr.std(ddof=1)) if len(arr)>1 else 0.0,4),
            'ecapa_min': round(float(arr.min()),4),
            'ecapa_max': round(float(arr.max()),4),
            'per_clip': per,
            'f0_median_mean': None,
            'emotion_distance_mean': None,
            'rate_mean': round(float(np.mean([per[t]['rate_wps'] for t in per])),3),
        }
        f0s = [per[t]['f0_median'] for t in per if per[t]['f0_median'] is not None]
        if f0s:
            out['configs'][cfg]['f0_median_mean'] = round(float(np.mean(f0s)),1)
        eds = [per[t]['emotion_distance'] for t in per if 'emotion_distance' in per[t]]
        if eds:
            out['configs'][cfg]['emotion_distance_mean'] = round(float(np.mean(eds)),4)

    out['timing']['wall_s'] = round(time.time()-t0, 2)
    out['baselines'] = {
        'samantha': {'ecapa': 0.707, 'sd': 0.030, 'ceiling': 0.828, 'wer': 0.050, 'mos': 3.73, 'f0': 201, 'rate': 3.49},
        'jarvis': {'ecapa': 0.733, 'sd': 0.053, 'ceiling': 0.725, 'wer': 0.018, 'mos': 4.14, 'f0': 143, 'rate': 3.31},
    }
    path = BASE/'scores/fallback_batch.json'
    json.dump(out, open(path,'w'), indent=2)
    print(json.dumps({k:{'n':v['n'],'ecapa_mean':v['ecapa_mean'],'ecapa_sd':v['ecapa_sd'],
                         'emotion_distance_mean':v.get('emotion_distance_mean'),
                         'f0_median_mean':v.get('f0_median_mean'),'rate_mean':v['rate_mean']}
                      for k,v in out['configs'].items()}, indent=2))
    print('wrote', path, 'wall_s', out['timing']['wall_s'])

if __name__ == '__main__':
    main()
