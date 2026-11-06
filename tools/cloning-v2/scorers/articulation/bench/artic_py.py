"""Python reference implementation (praat-parselmouth + numpy/scipy) of the same pipeline, for benchmarking galion-artic.
profile: Praat pitch (ac), Praat formants (burg), intensity, de Jong-style nuclei, pauses, fricative moments, plosive bursts, MFCC.
compare: + DTW-MCD (numpy min-plus scan DTW) and vowel-aligned formant deltas.
usage: artic_py.py profile f.wav...   |  artic_py.py compare ref.wav clone.wav"""
import sys, json, time, numpy as np, parselmouth
from scipy.signal import resample_poly, stft
from math import gcd
HOP = 0.01

def load(f):
    s = parselmouth.Sound(f)
    if s.n_channels > 1: s = s.convert_to_mono()
    return s

def mfcc(x, fs, t):
    wl = int(0.025*fs); nfft = 512; nb = 40
    win = np.hamming(wl); c = (t*fs).round().astype(int)
    xp = np.concatenate([[0], x[1:] - 0.97*x[:-1]])
    pad = np.pad(xp, (wl, wl)); idx = c[:, None] - wl//2 + np.arange(wl)[None, :] + wl
    P = np.abs(np.fft.rfft(pad[idx]*win, nfft))**2
    mel = lambda f: 2595*np.log10(1+f/700); imel = lambda m: 700*(10**(m/2595)-1)
    e = imel(np.linspace(mel(0), mel(fs/2), nb+2)); fb = np.zeros((nb, nfft//2+1)); bf = np.arange(nfft//2+1)*fs/nfft
    for m in range(nb):
        l, cc, h = e[m], e[m+1], e[m+2]
        fb[m] = np.where((bf > l) & (bf <= cc), (bf-l)/(cc-l), np.where((bf > cc) & (bf < h), (h-bf)/(h-cc), 0))
    L = np.log(np.maximum(P @ fb.T, 1e-10))
    k = np.arange(25)[:, None]; n = np.arange(nb)[None, :]
    D = np.cos(np.pi*k*(n+0.5)/nb)/nb; D[0] *= 0.5
    return L @ D.T

def analyze(f):
    s = load(f); fs = s.sampling_frequency; x = s.values[0]
    dur = s.duration; nf = int(dur/HOP); t = np.arange(nf)*HOP + HOP/2
    s16 = s.resample(16000); x16 = s16.values[0]
    inten = s16.to_intensity(minimum_pitch=100, time_step=HOP)
    idb = np.array([inten.get_value(tt) for tt in t], float); idb = np.nan_to_num(idb, nan=np.nanmin(idb))
    thr = max(np.quantile(idb, .99) - 25, idb.min()); sp = idb > thr
    pit = s16.to_pitch_ac(time_step=HOP, pitch_floor=65, pitch_ceiling=600)
    f0 = np.nan_to_num(np.array([pit.get_value_at_time(tt) for tt in t], float))
    med = np.median(f0[f0 > 0]) if (f0 > 0).any() else 0
    ceil = 5500 if med > 160 else 5000
    fo = s.to_formant_burg(time_step=HOP, max_number_of_formants=5, maximum_formant=ceil, window_length=0.025, pre_emphasis_from=50)
    F = np.array([[fo.get_value_at_time(k, tt) for k in (1, 2, 3)] for tt in t], float)
    # nuclei
    pk = [k for k in range(1, nf-1) if idb[k] > thr and idb[k] >= idb[k-1] and idb[k] > idb[k+1]]
    kept = []
    for p in pk:
        if kept:
            q = kept[-1]; dip = idb[q:p+1].min()
            if min(idb[q], idb[p]) - dip < 2:
                if idb[p] > idb[q]: kept[-1] = p
                continue
        kept.append(p)
    nuc = [k for k in kept if (f0[max(0, k-2):k+3] > 0).any()]
    vow = []
    for k in nuc:
        lim = idb[k]-3; a = b = k
        while a > 0 and k-a < 10 and f0[a-1] > 0 and idb[a-1] >= lim: a -= 1
        while b+1 < nf and b-k < 10 and f0[b+1] > 0 and idb[b+1] >= lim: b += 1
        vow.append((a, b, np.nanmedian(F[a:b+1], axis=0)))
    first = np.argmax(sp); last = nf-1-np.argmax(sp[::-1])
    pauses = []; i = first
    while i <= last:
        if not sp[i]:
            j = i
            while j <= last and not sp[j]: j += 1
            if (j-i)*HOP >= 0.3: pauses.append((j-i)*HOP)
            i = j
        else: i += 1
    # fricatives at 24k
    g = gcd(int(fs), 24000); xc = resample_poly(x, 24000//g, int(fs)//g) if fs > 24000 else x; fc = min(fs, 24000)
    wl = int(0.02*fc); c = (t*fc).round().astype(int); pad = np.pad(xc, (wl, wl))
    P = np.abs(np.fft.rfft(pad[c[:, None]-wl//2+np.arange(wl)[None, :]+wl]*np.hanning(wl), 512))**2
    bf = np.arange(257)*fc/512; hi = min(11000, fc/2-500)
    band = (bf >= 500) & (bf <= hi); tot = P[:, (bf >= 60) & (bf <= hi)].sum(1); hf = P[:, (bf >= 2500) & (bf <= hi)].sum(1)
    fr = sp & (f0 == 0) & (hf/np.maximum(tot, 1e-20) > 0.5)
    Pb = P[fr][:, band]; w = Pb/Pb.sum(1, keepdims=True); fb_ = bf[band]
    cen = (w*fb_).sum(1); sd = np.sqrt((w*(fb_-cen[:, None])**2).sum(1))
    skew = (w*(fb_-cen[:, None])**3).sum(1)/sd**3; kurt = (w*(fb_-cen[:, None])**4).sum(1)/sd**4-3
    # bursts (2.5 ms hop energy)
    bw = int(0.005*fc); bh = bw//2; xe = np.concatenate([[0], xc[1:]-0.97*xc[:-1]])
    nb_ = len(xc)//bh; e = np.array([10*np.log10((xe[n*bh:n*bh+bw]**2).sum()/bw+1e-12) for n in range(nb_)])
    eref = np.quantile(e, .99); bursts = []; n = 12
    while n+2 < nb_:
        pre = e[n-12:n-2]
        if e[n]-pre.mean() >= 15 and e[n]-e[n-1] >= 6 and e[n] > eref-35 and pre.max() < eref-25:
            p = n + int(np.argmax(e[n:n+3])); tt = (p*bh+bw/2)/fc
            if not bursts or tt-bursts[-1] > 0.04: bursts.append(tt)
            n = p+8; continue
        n += 1
    M = mfcc(x16, 16000, t)
    span = (last-first+1)*HOP; ph = span-sum(pauses)
    pts = np.array([[v[2][1], v[2][0]] for v in vow]) if vow else np.zeros((0, 2))
    from scipy.spatial import ConvexHull
    area = ConvexHull(pts[np.isfinite(pts).all(1)]).volume if len(pts) > 3 else 0
    prof = {"file": f, "f0_median": float(med), "n_syl": len(nuc), "articulation_rate": len(nuc)/ph, "vsa_hz2": float(area),
            "pauses": len(pauses), "fric_centroid": float(np.median(cen)) if len(cen) else None, "n_bursts": len(bursts)}
    return dict(t=t, sp=sp, f0=f0, F=F, M=M, vow=vow, span=(first, last), prof=prof)

def dtw(A, B):
    d = 6.1415*np.sqrt(((A[:, None, 1:]-B[None, :, 1:])**2).sum(-1)); n, m = d.shape
    D = np.full((n, m), np.inf); S = np.cumsum(d[0]); D[0] = S
    for i in range(1, n):
        a = np.minimum(D[i-1], np.concatenate([[np.inf], D[i-1][:-1]]))
        S = np.cumsum(d[i]); Sp = np.concatenate([[0], S[:-1]])
        D[i] = np.minimum.accumulate(a - Sp) + S
    i, j = n-1, m-1; path = [(i, j)]
    while i > 0 or j > 0:
        if i == 0: j -= 1
        elif j == 0: i -= 1
        else:
            k = np.argmin([D[i-1, j-1], D[i-1, j], D[i, j-1]]); i, j = (i-1, j-1) if k == 0 else ((i-1, j) if k == 1 else (i, j-1))
        path.append((i, j))
    path = path[::-1]; return path, float(np.mean([d[p] for p in path]))

if __name__ == "__main__":
    t0 = time.time(); mode = sys.argv[1]
    if mode == "profile":
        out = [analyze(f)["prof"] for f in sys.argv[2:]]
    else:
        r, c = analyze(sys.argv[2]), analyze(sys.argv[3])
        ri = [k for k in range(r["span"][0], r["span"][1]+1) if r["sp"][k]]; ci = [k for k in range(c["span"][0], c["span"][1]+1) if c["sp"][k]]
        path, mcd = dtw(r["M"][ri], c["M"][ci]); path = [(ri[i], ci[j]) for i, j in path]
        dl = []
        for a, b, fv in r["vow"]:
            js = [j for i, j in path if a <= i <= b and c["f0"][j] > 0]
            if js: dl.append(np.nanmedian(c["F"][js], axis=0)-fv)
        out = {"mcd_dtw": mcd, "n_vowels": len(dl), "mean_abs_df": np.nanmean(np.abs(dl), axis=0).tolist() if dl else None, "ref": r["prof"], "clone": c["prof"]}
    print(json.dumps({"result": out, "elapsed_s": time.time()-t0}, default=float))
