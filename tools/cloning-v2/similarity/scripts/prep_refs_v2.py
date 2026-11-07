#!/usr/bin/env python3
"""Improved multi-clip: longer overlapping windows from the same original + crossfade."""
from __future__ import annotations
import json
from pathlib import Path
import numpy as np
import soundfile as sf
import torch
import torchaudio
import pyloudnorm as pyln

BASE = Path("/workspace/swarm/voice-v2-sprint/cloning-v2/similarity")
IN = BASE / "inputs"
OUT = BASE / "refs"
TARGET_SR = 24000
TP_LIMIT = -1.0
LUFS_TARGET = -18.0  # slightly quieter target so TP lim doesn't crush as hard


def load_mono(path, sr=TARGET_SR):
    y, file_sr = sf.read(str(path), always_2d=True)
    y = y.mean(axis=1).astype(np.float32)
    if file_sr != sr:
        w = torch.from_numpy(y).unsqueeze(0)
        y = torchaudio.functional.resample(w, file_sr, sr).squeeze(0).numpy().astype(np.float32)
    return y, sr


def true_peak_dbtp(y, sr):
    w = torch.from_numpy(y.astype(np.float32)).unsqueeze(0)
    up = torchaudio.functional.resample(w, sr, sr * 4).squeeze(0).numpy()
    return 20.0 * np.log10(float(np.max(np.abs(up)) + 1e-12))


def loudness_normalize(y, sr, lufs=LUFS_TARGET, tp_limit=TP_LIMIT):
    meter = pyln.Meter(sr)
    if len(y) < sr:
        y = np.pad(y, (0, sr - len(y)))
    loudness = meter.integrated_loudness(y.astype(np.float64))
    if not np.isfinite(loudness):
        loudness = -30.0
    y2 = pyln.normalize.loudness(y.astype(np.float64), loudness, lufs)
    tp = true_peak_dbtp(y2.astype(np.float32), sr)
    if tp > tp_limit:
        y2 = y2 * (10 ** ((tp_limit - tp) / 20.0))
    lim = 10 ** (tp_limit / 20.0)
    peak = np.max(np.abs(y2)) + 1e-12
    if peak > lim:
        y2 = y2 * (lim / peak)
    return y2.astype(np.float32)


def declick(y, thresh=0.35):
    y2 = y.copy()
    d = np.abs(np.diff(y2, prepend=y2[:1]))
    for i in np.where(d > thresh)[0]:
        lo, hi = max(0, i - 2), min(len(y2), i + 3)
        y2[i] = float(np.median(y2[lo:hi]))
    return y2


def trim_edges(y, sr, pad_s=0.05):
    hop = int(0.01 * sr)
    n = int(0.025 * sr)
    rms = np.array([np.sqrt(np.mean(y[i:i+n]**2)+1e-12) for i in range(0, max(1,len(y)-n), hop)])
    q = np.quantile(rms, 0.15)
    thr = max(q * 3.0, 1e-5)
    speech = np.where(rms > thr)[0]
    if len(speech) == 0:
        return y
    a = max(0, speech[0]*hop - int(pad_s*sr))
    b = min(len(y), speech[-1]*hop + int(pad_s*sr))
    return y[a:b]


def crossfade_concat(parts, sr, xf_ms=30):
    xf = int(sr * xf_ms / 1000)
    out = parts[0].astype(np.float32)
    for p in parts[1:]:
        p = p.astype(np.float32)
        if len(out) < xf or len(p) < xf:
            out = np.concatenate([out, p])
            continue
        fade_out = np.linspace(1, 0, xf, dtype=np.float32)
        fade_in = np.linspace(0, 1, xf, dtype=np.float32)
        mid = out[-xf:] * fade_out + p[:xf] * fade_in
        out = np.concatenate([out[:-xf], mid, p[xf:]])
    return out


def build_cleaned(y, sr):
    return loudness_normalize(declick(trim_edges(y, sr)), sr)


def build_multiclip_long(y, sr):
    """Three overlapping windows (head / mid / tail) from the same take, crossfaded."""
    y0 = declick(trim_edges(y, sr))
    n = len(y0)
    # window ~60% of take, three starts at 0%, 20%, 40%
    win = max(int(0.55 * n), int(4.0 * sr))
    win = min(win, n)
    starts = [0, max(0, int(0.20 * n)), max(0, n - win)]
    starts = sorted(set(min(s, n - win) for s in starts))
    parts = []
    meta = []
    for s in starts:
        part = y0[s : s + win].copy()
        peak = np.max(np.abs(part)) + 1e-9
        part *= min(1.0, 0.75 / peak)
        parts.append(part)
        meta.append({"start": round(s / sr, 3), "end": round((s + win) / sr, 3), "dur": round(win / sr, 3)})
    y2 = crossfade_concat(parts, sr)
    return loudness_normalize(y2, sr), meta


def build_multiclip_best_phrases(y, sr):
    """Keep longer phrase chunks (>=1.2s) in time order, then also append mid window once more."""
    y0 = declick(trim_edges(y, sr))
    hop = int(0.01 * sr)
    n = int(0.025 * sr)
    rms = np.array([np.sqrt(np.mean(y0[i:i+n]**2)+1e-12) for i in range(0, max(1,len(y0)-n), hop)])
    q = np.quantile(rms, 0.15)
    thr = max(q * 3.5, 1e-5)
    speech = rms > thr
    # find contiguous regions >= 1.2s
    segs = []
    i = 0
    while i < len(speech):
        if not speech[i]:
            i += 1
            continue
        j = i
        while j < len(speech) and speech[j]:
            j += 1
        a = max(0, i * hop - int(0.04 * sr))
        b = min(len(y0), j * hop + int(0.04 * sr))
        if (b - a) / sr >= 1.2:
            segs.append((a, b))
        i = j
    if not segs:
        return build_multiclip_long(y, sr)
    # take up to 6 longest in chronological order
    segs = sorted(segs, key=lambda ab: -(ab[1] - ab[0]))[:6]
    segs = sorted(segs, key=lambda ab: ab[0])
    parts = []
    meta = []
    for a, b in segs:
        part = y0[a:b].copy()
        peak = np.max(np.abs(part)) + 1e-9
        part *= min(1.0, 0.75 / peak)
        parts.append(part)
        meta.append({"start": round(a/sr,3), "end": round(b/sr,3), "dur": round((b-a)/sr,3)})
    # also append full cleaned once for phonetic completeness if pack short
    total = sum(m["dur"] for m in meta)
    if total < 12:
        parts.append(y0.copy())
        meta.append({"start": 0.0, "end": round(len(y0)/sr,3), "dur": round(len(y0)/sr,3), "note": "full_append"})
    y2 = crossfade_concat(parts, sr)
    return loudness_normalize(y2, sr), meta


def write_wav(path, y, sr):
    peak = np.max(np.abs(y)) + 1e-12
    if peak > 0.99:
        y = y * (0.99 / peak)
    sf.write(str(path), y, sr, subtype="PCM_16")


def analyze(path, y, sr, extra=None):
    info = {
        "path": str(path),
        "sr": sr,
        "duration_s": round(len(y) / sr, 3),
        "peak_dbfs": round(20 * np.log10(float(np.max(np.abs(y)) + 1e-12)), 2),
        "rms_dbfs": round(20 * np.log10(float(np.sqrt(np.mean(y**2)) + 1e-12)), 2),
        "true_peak_dbtp_est": round(true_peak_dbtp(y, sr), 2),
        "clicks": int(np.sum(np.abs(np.diff(y, prepend=y[:1])) > 0.35)),
        "channels": 1,
    }
    try:
        info["lufs"] = round(float(pyln.Meter(sr).integrated_loudness(y.astype(np.float64))), 2)
    except Exception as e:
        info["lufs_error"] = str(e)
    if extra is not None:
        info["segments"] = extra
    return info


def main():
    report = {}
    for persona in ("samantha", "jarvis"):
        y, sr = load_mono(IN / f"{persona}-original.wav")
        cleaned = build_cleaned(y, sr)
        cpath = OUT / f"{persona}-cleaned-ln.wav"
        write_wav(cpath, cleaned, sr)

        multi_long, meta_long = build_multiclip_long(y, sr)
        mlpath = OUT / f"{persona}-multiclip-long-ln.wav"
        write_wav(mlpath, multi_long, sr)

        multi_phr, meta_phr = build_multiclip_best_phrases(y, sr)
        mppath = OUT / f"{persona}-multiclip-phrases-ln.wav"
        write_wav(mppath, multi_phr, sr)

        report[persona] = {
            "cleaned_ln": analyze(cpath, cleaned, sr),
            "multiclip_long_ln": analyze(mlpath, multi_long, sr, meta_long),
            "multiclip_phrases_ln": analyze(mppath, multi_phr, sr, meta_phr),
        }
        print(persona,
              "cleaned", report[persona]["cleaned_ln"]["duration_s"],
              "TP", report[persona]["cleaned_ln"]["true_peak_dbtp_est"],
              "long", report[persona]["multiclip_long_ln"]["duration_s"],
              "phr", report[persona]["multiclip_phrases_ln"]["duration_s"])
    (OUT / "prep_report_v2.json").write_text(json.dumps(report, indent=2))
    print("ok")


if __name__ == "__main__":
    main()
