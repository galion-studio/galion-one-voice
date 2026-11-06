# numpy cross-check of galion-hygiene: sample peak, clipped(|x|>=0.99), clicks(|dx|>0.35), max jump, Welch f90/f99 (same definition).
import sys, json, wave, numpy as np
out={}
for p in sys.argv[2:]:
    w=wave.open(p); nc=w.getnchannels(); sr=w.getframerate()
    x=np.frombuffer(w.readframes(w.getnframes()),'<i2').astype(np.float64).reshape(-1,nc)/32768
    d=np.abs(np.diff(x,axis=0))
    m=x.mean(1); n=2048; h=n//2; win=0.5-0.5*np.cos(2*np.pi*np.arange(n)/n)
    segs=[m[s:s+n] for s in range(0,len(m)-n+1,h)]
    P=sum(np.abs(np.fft.rfft((s-s.mean())*win))**2 for s in segs); c=np.cumsum(P)/P.sum(); f=np.arange(len(P))*sr/n
    out[p]={'sample_peak_dbfs':20*np.log10(np.abs(x).max()),'clipped':int((np.abs(x)>=0.99).sum()),'clicks':int((d>0.35).sum()),
            'max_jump':float(d.max()),'f90':float(f[np.searchsorted(c,0.90)]),'f99':float(f[np.searchsorted(c,0.99)])}
json.dump(out,open(sys.argv[1],'w'),indent=1)
