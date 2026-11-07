#!/usr/bin/env python3
"""Build cleaned + multi-clip reference packs from owner's originals only."""
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
OUT.mkdir(parents=True, exist_ok=True)
TARGET_SR = 24000
TP_LIMIT = -1.0  # dBTP
LUFS_TARGET = -16.0


def load_mono(path: Path, sr: int = TARGET_SR):
    y, file_sr = sf.read(str(path), always_2d=True)
    y = y.mean(axis=1).astype(np.float32)
    if file_sr != sr:
        w = torch.from_numpy(y).unsqueeze(0)
        w = torchaudio.functional.resample(w, file_sr, sr)
        y = w.squeeze(0).numpy().astype(np.float32)
    return y, sr


def true_peak_dbtp(y: np.ndarray, sr: int) -> float:
    w = torch.from_numpy(y.astype(np.float32)).unsqueeze(0)
    up = torchaudio.functional.resample(w, sr, sr * 4).squeeze(0).numpy()
    peak = float(np.max(np.abs(up)) + 1e-12)
    return 20.0 * np.log10(peak)


def loudness_normalize(y: np.ndarray, sr: int, lufs=LUFS_TARGET, tp_limit=TP_LIMIT) -> np.ndarray:
    meter = pyln.Meter(sr)
    if len(y) < sr:
        y = np.pad(y, (0, sr - len(y)))
    loudness = meter.integrated_loudness(y.astype(np.float64))
    if not np.isfinite(loudness):
        loudness = -30.0
    y2 = pyln.normalize.loudness(y.astype(np.float64), loudness, lufs)
    # Always enforce true-peak after loudnorm (may warn about clip before we fix)
    tp = true_peak_dbtp(y2.astype(np.float32), sr)
    if tp > tp_limit:
        y2 = y2 * (10 ** ((tp_limit - tp) / 20.0))
    lim = 10 ** (tp_limit / 20.0)
    peak = np.max(np.abs(y2)) + 1e-12
    if peak > lim:
        y2 = y2 * (lim / peak)
    return y2.astype(np.float32)


def click_mask(y: np.ndarray, thresh=0.35):
    d = np.abs(np.diff(y, prepend=y[:1]))
    return d > thresh


def frame_metrics(y: np.ndarray, sr: int, frame=0.025, hop=0.010):
    n = int(sr * frame)
    h = int(sr * hop)
    rms = []
    for i in range(0, max(1, len(y) - n), h):
        seg = y[i : i + n]
        rms.append(float(np.sqrt(np.mean(seg**2) + 1e-12)))
    return np.array(rms), h


def find_speech_segments(y, sr, min_dur=0.6, max_dur=6.0, pad=0.05):
    rms, hop = frame_metrics(y, sr)
    q = np.quantile(rms, 0.15)
    thr = max(q * 4.0, 10 ** (-45 / 20))
    speech = rms > thr
    segs = []
    i = 0
    n = len(speech)
    while i < n:
        if not speech[i]:
            i += 1
            continue
        j = i
        while j < n and speech[j]:
            j += 1
        a = max(0, i * hop - int(pad * sr))
        b = min(len(y), j * hop + int(pad * sr))
        dur = (b - a) / sr
        if dur >= min_dur:
            if dur > max_dur:
                win = int(max_dur * sr)
                best, best_s = -1.0, a
                step = hop * 5
                for s in range(a, b - win + 1, max(1, step)):
                    e = float(np.mean(y[s : s + win] ** 2))
                    if e > best:
                        best, best_s = e, s
                a, b = best_s, best_s + win
            seg = y[a:b]
            ccount = int(np.sum(click_mask(seg)))
            noise = q + 1e-12
            snr = 20 * np.log10((np.sqrt(np.mean(seg**2)) + 1e-12) / noise)
            segs.append(
                {
                    "start": a / sr,
                    "end": b / sr,
                    "dur": (b - a) / sr,
                    "snr": float(snr),
                    "clicks": ccount,
                    "a": a,
                    "b": b,
                }
            )
        i = j
    segs.sort(key=lambda s: (s["clicks"], -s["snr"], -s["dur"]))
    return segs


def trim_edges(y, sr, pad_s=0.08):
    rms, hop = frame_metrics(y, sr)
    q = np.quantile(rms, 0.15)
    thr = max(q * 3.0, 10 ** (-45 / 20))
    speech = np.where(rms > thr)[0]
    if len(speech) == 0:
        return y
    a = max(0, speech[0] * hop - int(pad_s * sr))
    b = min(len(y), speech[-1] * hop + int(pad_s * sr))
    return y[a:b]


def build_cleaned(y, sr):
    y2 = trim_edges(y, sr).copy()
    d = np.abs(np.diff(y2, prepend=y2[:1]))
    bad = d > 0.35
    if bad.any():
        for i in np.where(bad)[0]:
            lo = max(0, i - 2)
            hi = min(len(y2), i + 3)
            y2[i] = float(np.median(y2[lo:hi]))
    return loudness_normalize(y2, sr)


def build_multiclip(y, sr, target_dur=20.0):
    segs = find_speech_segments(y, sr)
    if not segs:
        return build_cleaned(y, sr), []
    chosen = []
    total = 0.0
    gap = np.zeros(int(0.08 * sr), dtype=np.float32)
    parts = []
    meta = []
    for s in segs:
        if s["clicks"] > 2:
            continue
        if any(abs(s["start"] - c["start"]) < 0.4 for c in chosen):
            continue
        chosen.append(s)
        part = y[s["a"] : s["b"]].astype(np.float32)
        peak = np.max(np.abs(part)) + 1e-9
        part = part * min(1.0, 0.7 / peak)
        if parts:
            parts.append(gap)
        parts.append(part)
        meta.append({k: s[k] for k in ("start", "end", "dur", "snr", "clicks")})
        total += s["dur"]
        if total >= target_dur:
            break
    if total < 8.0:
        for s in segs:
            if any(abs(s["start"] - c["start"]) < 0.4 for c in chosen):
                continue
            chosen.append(s)
            part = y[s["a"] : s["b"]].astype(np.float32)
            peak = np.max(np.abs(part)) + 1e-9
            part = part * min(1.0, 0.7 / peak)
            if parts:
                parts.append(gap)
            parts.append(part)
            meta.append({k: s[k] for k in ("start", "end", "dur", "snr", "clicks")})
            total += s["dur"]
            if total >= 12.0:
                break
    if not parts:
        return build_cleaned(y, sr), []
    y2 = np.concatenate(parts)
    return loudness_normalize(y2, sr), meta


def write_wav(path: Path, y: np.ndarray, sr: int):
    peak = np.max(np.abs(y)) + 1e-12
    if peak > 0.99:
        y = y * (0.99 / peak)
    sf.write(str(path), y, sr, subtype="PCM_16")


def analyze(path: Path, y, sr, extra=None):
    info = {
        "path": str(path),
        "sr": sr,
        "duration_s": round(len(y) / sr, 3),
        "peak_dbfs": round(20 * np.log10(float(np.max(np.abs(y)) + 1e-12)), 2),
        "rms_dbfs": round(20 * np.log10(float(np.sqrt(np.mean(y**2)) + 1e-12)), 2),
        "true_peak_dbtp_est": round(true_peak_dbtp(y, sr), 2),
        "clicks": int(np.sum(click_mask(y))),
        "channels": 1,
    }
    try:
        meter = pyln.Meter(sr)
        info["lufs"] = round(float(meter.integrated_loudness(y.astype(np.float64))), 2)
    except Exception as e:
        info["lufs_error"] = str(e)
    if extra is not None:
        info["segments"] = extra
    return info


def main():
    report = {}
    for persona in ("samantha", "jarvis"):
        src = IN / f"{persona}-original.wav"
        y, sr = load_mono(src)
        mono_path = OUT / f"{persona}-mono24.wav"
        y_w = y if np.max(np.abs(y)) <= 0.99 else y * (0.99 / (np.max(np.abs(y)) + 1e-12))
        write_wav(mono_path, y_w, sr)

        cleaned = build_cleaned(y, sr)
        cleaned_path = OUT / f"{persona}-cleaned-ln.wav"
        write_wav(cleaned_path, cleaned, sr)

        multi, meta = build_multiclip(y, sr)
        multi_path = OUT / f"{persona}-multiclip-ln.wav"
        write_wav(multi_path, multi, sr)

        ref_y, ref_sr = load_mono(IN / f"{persona}-reference.wav")
        report[persona] = {
            "original_mono24": analyze(mono_path, y_w, sr),
            "cleaned_ln": analyze(cleaned_path, cleaned, sr),
            "multiclip_ln": analyze(multi_path, multi, sr, meta),
            "existing_reference": analyze(IN / f"{persona}-reference.wav", ref_y, ref_sr),
        }
        print(
            persona,
            "cleaned",
            report[persona]["cleaned_ln"]["duration_s"],
            "s TP",
            report[persona]["cleaned_ln"]["true_peak_dbtp_est"],
            "multi",
            report[persona]["multiclip_ln"]["duration_s"],
            "s segs",
            len(meta),
        )

    (OUT / "prep_report.json").write_text(json.dumps(report, indent=2))
    print("Wrote", OUT / "prep_report.json")


if __name__ == "__main__":
    main()
