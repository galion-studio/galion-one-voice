
import wave, json
from pathlib import Path
import numpy as np

def read_wav(path):
    with wave.open(str(path), "rb") as w:
        sr = w.getframerate(); nch = w.getnchannels(); sw = w.getsampwidth(); n = w.getnframes()
        raw = w.readframes(n)
    assert sw == 2
    x = np.frombuffer(raw, dtype=np.int16).astype(np.float64) / 32768.0
    if nch == 2:
        x = x.reshape(-1, 2).mean(axis=1)
    return x, sr

def write_wav(path, x, sr):
    x = np.clip(x, -1.0, 1.0)
    y = (x * 32767.0).astype(np.int16)
    with wave.open(str(path), "wb") as w:
        w.setnchannels(1); w.setsampwidth(2); w.setframerate(sr)
        w.writeframes(y.tobytes())

def frame_rms(x, win):
    n = len(x) // win
    if n < 1:
        return np.array([np.sqrt(np.mean(x**2)+1e-12)])
    frames = x[:n*win].reshape(n, win)
    return np.sqrt(np.mean(frames**2, axis=1) + 1e-12)

def apply_energy_expand(x, sr, expand=1.12, floor_db=-45.0):
    win = max(1, int(sr * 0.02))
    rms = frame_rms(x, win)
    rms_db = 20 * np.log10(rms + 1e-12)
    active = rms_db > floor_db
    if active.sum() < 3:
        return x.copy()
    med = np.median(rms_db[active])
    new_db = med + (rms_db - med) * expand
    gain_db = new_db - rms_db
    gains = 10 ** (gain_db / 20.0)
    out = np.zeros_like(x)
    hop = win
    for i, g in enumerate(gains):
        a = i * hop; b = min(len(x), a + win)
        out[a:b] = x[a:b] * g
    r0 = np.sqrt(np.mean(x**2) + 1e-12)
    r1 = np.sqrt(np.mean(out**2) + 1e-12)
    out *= (r0 / r1)
    peak = np.max(np.abs(out)) + 1e-12
    if peak > 0.99:
        out *= 0.99 / peak
    return out

def yin_f0(frame, sr, fmin=70.0, fmax=400.0):
    # lightweight autocorrelation pitch
    x = frame.astype(np.float64)
    x = x - x.mean()
    if np.max(np.abs(x)) < 1e-4:
        return np.nan
    n = len(x)
    # difference function approx via autocorr
    corr = np.correlate(x, x, mode="full")[n-1:]
    if corr[0] <= 0:
        return np.nan
    corr = corr / corr[0]
    i_min = int(sr / fmax); i_max = min(n - 1, int(sr / fmin))
    if i_max <= i_min + 2:
        return np.nan
    seg = corr[i_min:i_max]
    # find peak
    k = int(np.argmax(seg))
    if seg[k] < 0.3:
        return np.nan
    lag = i_min + k
    # parabolic refine
    if 0 < k < len(seg) - 1:
        a, b, c = seg[k-1], seg[k], seg[k+1]
        denom = (a - 2*b + c)
        if abs(denom) > 1e-12:
            lag = lag + 0.5 * (a - c) / denom
    return sr / lag

def apply_f0_contour_expand(x, sr, scale=1.08, max_shift_st=0.35):
    """Expand F0 deviations from median via overlapping granular resample.
    Mean/median F0 kept; |local shift| capped. Light = safe for ECAPA.
    """
    win = int(sr * 0.04); hop = int(sr * 0.01)
    if len(x) < win * 3:
        return x.copy(), {"f0_applied": False}
    # estimate F0 contour
    f0s = []
    for i in range(0, len(x) - win, hop):
        f0s.append(yin_f0(x[i:i+win], sr))
    f0 = np.array(f0s, dtype=np.float64)
    voiced = np.isfinite(f0)
    if voiced.sum() < 5:
        return x.copy(), {"f0_applied": False, "reason": "too few voiced frames"}
    med = np.nanmedian(f0)
    # desired: new_f0 = med * (f0/med)**scale ≈ expand log-F0 around median
    ratio = np.ones_like(f0)
    ratio[voiced] = (f0[voiced] / med) ** (scale - 1.0)  # multiply factor on instantaneous F0
    # convert to semitone shift of the grain: st = 12*log2(ratio_applied_to_playback)
    # To raise F0 by factor r, resample grain shorter (play faster): rate = r
    st = np.zeros_like(f0)
    st[voiced] = 12.0 * np.log2(np.clip(ratio[voiced], 0.5, 2.0))
    st = np.clip(st, -max_shift_st, max_shift_st)
    # smooth
    kern = np.hanning(5); kern /= kern.sum()
    st_pad = np.pad(st, (2, 2), mode="edge")
    st_s = np.convolve(st_pad, kern, mode="valid")
    # synthesize with OLA granular resample
    out = np.zeros(len(x) + win, dtype=np.float64)
    wnorm = np.zeros(len(x) + win, dtype=np.float64)
    window = np.hanning(win)
    for i, shift in enumerate(st_s):
        a = i * hop
        grain = x[a:a+win] * window
        if len(grain) < win:
            break
        rate = 2 ** (shift / 12.0)  # >1 raises pitch
        new_len = max(16, int(round(win / rate)))
        # linear resample grain to new_len then place at original position (duration preserve ≈ pitch change with formant shift — mild)
        t_old = np.linspace(0, 1, win, endpoint=False)
        t_new = np.linspace(0, 1, new_len, endpoint=False)
        g2 = np.interp(t_new, t_old, grain)
        # place centered
        if new_len >= win:
            g2 = g2[:win]
            place = g2
        else:
            place = np.zeros(win)
            off = (win - new_len) // 2
            place[off:off+new_len] = g2
            place *= window  # re-window
        out[a:a+win] += place
        wnorm[a:a+win] += window
    m = wnorm > 1e-6
    out[m] /= wnorm[m]
    out = out[:len(x)]
    # preserve RMS + peak
    r0 = np.sqrt(np.mean(x**2) + 1e-12)
    r1 = np.sqrt(np.mean(out**2) + 1e-12)
    out *= (r0 / r1)
    peak = np.max(np.abs(out)) + 1e-12
    if peak > 0.99:
        out *= 0.99 / peak
    mean_shift = float(np.nanmean(st_s[voiced])) if voiced.any() else 0.0
    return out, {
        "f0_applied": True,
        "f0_median_hz": float(med),
        "scale": scale,
        "max_shift_st": max_shift_st,
        "mean_shift_st_voiced": mean_shift,
        "n_voiced": int(voiced.sum()),
    }

def process_file(src, dst, energy_expand=1.12, f0_scale=1.08, max_shift_st=0.35):
    x, sr = read_wav(src)
    y = apply_energy_expand(x, sr, expand=energy_expand)
    y2, f0meta = apply_f0_contour_expand(y, sr, scale=f0_scale, max_shift_st=max_shift_st)
    write_wav(dst, y2, sr)
    meta = {"src": str(src), "dst": str(dst), "sr": sr, "energy_expand": energy_expand, **f0meta}
    return meta
