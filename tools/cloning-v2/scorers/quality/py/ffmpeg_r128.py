# Cross-check galion-hygiene LUFS/LRA/true-peak vs ffmpeg ebur128 (libavfilter, 4x/192k true-peak), f90/f99 vs scipy welch.
import json,subprocess,re,sys,glob
import numpy as np
try:
    from scipy.signal import welch; import soundfile
except Exception: welch=None
rows=[]
for p in ('samantha','jarvis'):
    rh=json.load(open(f'out/rs_hyg_{p}.json'))
    for f in rh['files']:
        o=subprocess.run(['ffmpeg','-nostats','-hide_banner','-i',f['path'],'-af','ebur128=peak=true','-f','null','-'],capture_output=True,text=True).stderr
        s=o[o.rfind('Summary'):]
        I=float(re.search(r'I:\s+(-?[\d.]+)',s).group(1)); L=float(re.search(r'LRA:\s+(-?[\d.]+)',s).group(1)); TP=float(re.search(r'True peak:\s+Peak:\s+(-?[\d.inf]+)',s).group(1))
        rows.append((f['path'].split('/')[-2]+'/'+f['path'].split('/')[-1],f['loudness']['integrated_lufs'],I,f['loudness']['lra_lu'],L,f['true_peak_dbtp'],TP))
dI=[abs(r[1]-r[2]) for r in rows]; dL=[abs(r[3]-r[4]) for r in rows if r[3] is not None]; dT=[abs(r[5]-r[6]) for r in rows]
for r in rows: print(*r)
print('max|dLUFS|',max(dI),'max|dLRA|',max(dL),'max|dTP|',max(dT),'mean|dTP|',sum(dT)/len(dT))
