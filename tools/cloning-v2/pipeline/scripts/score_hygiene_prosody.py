#!/usr/bin/env python3
"""Peak/clip/click + F0/rate/energy for v2-pipe WAVs. Does not replace galion scorers."""
import json, sys
from pathlib import Path
import numpy as np
import torchaudio
import torch

SR = 24000
CLICK = 0.35

def load(p):
    w, sr = torchaudio.load(p)
    w = w.mean(0)
    if sr != SR:
        w = torchaudio.functional.resample(w.unsqueeze(0), sr, SR).squeeze(0)
    return w.numpy()

def artifacts(x):
    peak = float(np.max(np.abs(x))) if len(x) else 0.0
    peak_db = 20*np.log10(peak+1e-12)
    clips = int(np.sum(np.abs(x) >= 0.999))
    if len(x) > 1:
        jumps = np.abs(np.diff(x))
        clicks = int(np.sum(jumps > CLICK))
        max_jump = float(jumps.max())
    else:
        clicks, max_jump = 0, 0.0
    return dict(peak_dbfs=round(peak_db,2), clipping_samples=clips,
                clicks_big_jumps=clicks, max_sample_jump=round(max_jump,4),
                duration_s=round(len(x)/SR,3))

def energy_dyn(x):
    # frame RMS dyn range
    hop=256; win=1024
    if len(x)<win: return 0.0
    frames=[]
    for i in range(0,len(x)-win,hop):
        f=x[i:i+win]
        frames.append(np.sqrt(np.mean(f*f)+1e-12))
    db=20*np.log10(np.array(frames))
    # use 10th-90th percentile span
    return float(np.percentile(db,90)-np.percentile(db,10))

def f0_median(x):
    try:
        import torchcrepe
    except Exception:
        # fallback: none
        return None, None
    # torchcrepe expects [B,T]
    t=torch.from_numpy(x).float().unsqueeze(0)
    with torch.no_grad():
        f0 = torchcrepe.predict(t, SR, hop_length=240, fmin=50, fmax=500,
                                model='tiny', batch_size=512, device='cpu',
                                return_periodicity=False)
    f=f0.squeeze().numpy()
    voiced=f[(f>60)&(f<500)]
    if len(voiced)==0: return None, None
    med=float(np.median(voiced))
    # sd in semitones vs median
    st=12*np.log2(voiced/med)
    return med, float(np.std(st))

def speaking_rate(path, text_path):
    text=Path(text_path).read_text().strip()
    words=len(text.split())
    w,sr=torchaudio.load(path)
    dur=w.shape[1]/sr
    # rough: full duration (incl pauses) — same as analysis method approximation
    return words/max(dur,1e-3)

def score_dir(wav_dir, text_dir):
    out={}
    for p in sorted(Path(wav_dir).glob('t*.wav')):
        x=load(p)
        a=artifacts(x)
        f0m,f0sd=f0_median(x)
        tid=p.stem
        out[tid]=dict(
            artifacts=a,
            energy_dyn_range_db=round(energy_dyn(x),2),
            f0_median_hz=None if f0m is None else round(f0m,1),
            f0_sd_st=None if f0sd is None else round(f0sd,2),
            speaking_rate_wps=round(speaking_rate(p, Path(text_dir)/f'{tid}.txt'),3),
        )
    # aggregates
    peaks=[v['artifacts']['peak_dbfs'] for v in out.values()]
    clips=sum(v['artifacts']['clipping_samples'] for v in out.values())
    clicks=sum(v['artifacts']['clicks_big_jumps'] for v in out.values())
    hit0=sum(1 for p in peaks if p>=-0.05)
    f0s=[v['f0_median_hz'] for v in out.values() if v['f0_median_hz']]
    rates=[v['speaking_rate_wps'] for v in out.values()]
    ener=[v['energy_dyn_range_db'] for v in out.values()]
    summary=dict(
        n=len(out),
        peak_dbfs_mean=round(float(np.mean(peaks)),2) if peaks else None,
        clips_with_peak_0dbfs=hit0,
        total_clipping_samples=clips,
        total_clicks=clicks,
        f0_median_hz_mean=round(float(np.mean(f0s)),1) if f0s else None,
        speaking_rate_wps_mean=round(float(np.mean(rates)),3) if rates else None,
        energy_dyn_range_db_mean=round(float(np.mean(ener)),2) if ener else None,
        per_clip=out,
    )
    return summary

if __name__=='__main__':
    root=Path(sys.argv[1]) if len(sys.argv)>1 else Path('/workspace/samples/v2')
    text='/workspace/swarm/voice-v2-sprint/own-clone-analysis/text'
    result={}
    for persona in ['samantha','jarvis']:
        d=root/persona
        if d.exists():
            result[persona]=score_dir(d, text)
    outp=Path('/workspace/swarm/voice-v2-sprint/cloning-v2/pipeline/scores/hygiene_prosody.json')
    outp.write_text(json.dumps(result, indent=2))
    print(json.dumps({k:{kk:vv for kk,vv in v.items() if kk!='per_clip'} for k,v in result.items()}, indent=2))
