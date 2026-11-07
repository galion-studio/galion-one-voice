#!/usr/bin/env python3
"""Build HF-enhanced Samantha reference variants.
HF content above ~2.7 kHz is ESTIMATED (not recovered from the recording).
"""
import os
os.environ.setdefault("OMP_NUM_THREADS", "2")
os.environ.setdefault("MKL_NUM_THREADS", "2")
os.environ.setdefault("TORCH_NUM_THREADS", "2")

import json
from pathlib import Path
import numpy as np
import soundfile as sf
import librosa
import scipy.signal as sig
import pyloudnorm as pyln
import noisereduce as nr

BASE = Path("/workspace/swarm/voice-v2-sprint/cloning-v2/hf-samantha")
OUT = BASE / "refs"
OUT.mkdir(parents=True, exist_ok=True)
TARGET_SR = 24000
TARGET_LUFS = -20.0
TRUE_PEAK_DBTP = -1.0


def load_mono(path: Path, target_sr: int | None = None):
    y, sr = sf.read(str(path), always_2d=True)
    mono = y.mean(axis=1).astype(np.float64)
    if target_sr and sr != target_sr:
        mono = librosa.resample(mono, orig_sr=sr, target_sr=target_sr)
        sr = target_sr
    return mono, sr


def bandwidth(y, sr, n_fft=4096):
    S = np.abs(librosa.stft(y, n_fft=n_fft, hop_length=n_fft // 4)) ** 2
    ps = S.mean(axis=1)
    fq = librosa.fft_frequencies(sr=sr, n_fft=n_fft)
    c = np.cumsum(ps)
    c = c / max(c[-1], 1e-20)
    f90 = float(fq[min(np.searchsorted(c, 0.90), len(fq) - 1)])
    f99 = float(fq[min(np.searchsorted(c, 0.99), len(fq) - 1)])
    e4 = float(ps[fq >= 4000].sum() / max(ps.sum(), 1e-20) * 100)
    return {"f90_hz": round(f90, 1), "f99_hz": round(f99, 1), "pct_energy_gt_4k": round(e4, 3)}


def declick(y, sr, thresh=0.35, win_ms=1.5):
    """Median-replace isolated large sample jumps (clicks)."""
    out = y.copy()
    d = np.abs(np.diff(out, prepend=out[0]))
    hits = np.where(d > thresh)[0]
    half = max(1, int(sr * win_ms / 1000 / 2))
    fixed = 0
    for i in hits:
        a = max(0, i - half)
        b = min(len(out), i + half + 1)
        # skip if dense (speech transient): many neighbors also large
        if (d[max(0, i - 3) : i + 4] > thresh * 0.7).sum() > 3:
            continue
        out[a:b] = np.median(out[max(0, a - half) : min(len(out), b + half)])
        fixed += 1
    return out, fixed


def spectral_gate_denoise(y, sr):
    """Gentle stationary noise reduce via noisereduce (speech-preserving)."""
    # use quietest 0.4s as noise profile hint via prop_decrease mild
    y32 = y.astype(np.float32)
    reduced = nr.reduce_noise(
        y=y32,
        sr=sr,
        stationary=True,
        prop_decrease=0.55,
        n_fft=2048,
        hop_length=512,
        n_std_thresh_stationary=1.8,
    )
    return reduced.astype(np.float64)


def high_shelf(y, sr, freq=2800.0, gain_db=6.0, Q=0.7):
    """RBJ high-shelf EQ."""
    A = 10 ** (gain_db / 40.0)
    w0 = 2 * np.pi * freq / sr
    alpha = np.sin(w0) / (2 * Q)
    cosw = np.cos(w0)
    b0 = A * ((A + 1) + (A - 1) * cosw + 2 * np.sqrt(A) * alpha)
    b1 = -2 * A * ((A - 1) + (A + 1) * cosw)
    b2 = A * ((A + 1) + (A - 1) * cosw - 2 * np.sqrt(A) * alpha)
    a0 = (A + 1) - (A - 1) * cosw + 2 * np.sqrt(A) * alpha
    a1 = 2 * ((A - 1) - (A + 1) * cosw)
    a2 = (A + 1) - (A - 1) * cosw - 2 * np.sqrt(A) * alpha
    b = np.array([b0, b1, b2]) / a0
    a = np.array([1.0, a1 / a0, a2 / a0])
    return sig.lfilter(b, a, y)


def harmonic_exciter(y, sr, drive=1.8, mix=0.18, hp_hz=2200.0):
    """Soft-saturate midband and highpass-blend for estimated presence."""
    # bandpass drive region ~1.2–4 kHz then HP blend
    sos_bp = sig.butter(2, [1200 / (sr / 2), min(4500, sr / 2 - 100) / (sr / 2)], btype="band", output="sos")
    band = sig.sosfilt(sos_bp, y)
    driven = np.tanh(drive * band)
    sos_hp = sig.butter(2, hp_hz / (sr / 2), btype="high", output="sos")
    air = sig.sosfilt(sos_hp, driven)
    # match RMS of air to a fraction of y
    y_rms = np.sqrt(np.mean(y**2)) + 1e-12
    air_rms = np.sqrt(np.mean(air**2)) + 1e-12
    air = air * (y_rms / air_rms) * mix
    return y + air


def spectral_mirror_bwe(y, sr, src_lo=900.0, src_hi=2700.0, dst_lo=2700.0, dst_hi=8000.0, mix=0.55):
    """Classical bandwidth extension: mirror lower speech band into HF with envelope.
    ESTIMATED HF — not true recovery.
    """
    n_fft = 4096
    hop = 1024
    S = librosa.stft(y, n_fft=n_fft, hop_length=hop)
    mag = np.abs(S)
    phase = np.angle(S)
    fq = librosa.fft_frequencies(sr=sr, n_fft=n_fft)
    src = np.where((fq >= src_lo) & (fq <= src_hi))[0]
    dst = np.where((fq >= dst_lo) & (fq <= min(dst_hi, sr / 2 - 50)))[0]
    if len(src) < 4 or len(dst) < 4:
        return y
    # map dst bins from reversed src (spectral folding)
    src_idx = src[::-1]
    # interpolate source magnitudes onto dst length
    src_mag = mag[src_idx, :]
    # resample along frequency axis
    from scipy.ndimage import zoom

    zoom_f = len(dst) / len(src_idx)
    mapped = zoom(src_mag, (zoom_f, 1.0), order=1)
    mapped = mapped[: len(dst), :]
    # taper and attenuate (~-8 to -14 dB sloping)
    taper = np.linspace(0.45, 0.18, len(dst))[:, None]
    # only add where original mag is near noise (don't overwrite existing)
    speech_mask = (mag[src, :].mean(axis=0, keepdims=True) > np.percentile(mag[src, :], 40))
    add = mapped * taper * mix
    existing = mag[dst, :]
    # fill mostly empty HF
    fill = np.maximum(add * speech_mask - existing * 0.3, 0)
    mag2 = mag.copy()
    mag2[dst, :] = existing + fill
    # synthesize with original phase + slight randomization on new bins for less metallic tone
    rng = np.random.default_rng(42)
    phase2 = phase.copy()
    phase2[dst, :] = phase[src_idx[np.linspace(0, len(src_idx) - 1, len(dst)).astype(int)], :] + rng.uniform(
        -0.3, 0.3, size=phase2[dst, :].shape
    )
    Y = mag2 * np.exp(1j * phase2)
    out = librosa.istft(Y, hop_length=hop, length=len(y))
    return out.astype(np.float64)


def loudness_normalize(y, sr, target_lufs=TARGET_LUFS, true_peak_dbtp=TRUE_PEAK_DBTP):
    meter = pyln.Meter(sr)  # BS.1770
    # pyloudnorm expects finite audio; pad tiny if short
    loudness = meter.integrated_loudness(y)
    if not np.isfinite(loudness):
        # fallback RMS
        rms = np.sqrt(np.mean(y**2)) + 1e-12
        y = y * (10 ** (target_lufs / 20) / rms)
    else:
        y = pyln.normalize.loudness(y, loudness, target_lufs)
    # true-peak limiter (oversampled peak)
    # 4x oversample peak estimate
    y_os = sig.resample_poly(y, 4, 1)
    peak = np.max(np.abs(y_os)) + 1e-12
    limit = 10 ** (true_peak_dbtp / 20.0)
    if peak > limit:
        y = y * (limit / peak)
    # final sample peak safety
    sp = np.max(np.abs(y)) + 1e-12
    if sp > limit:
        y = y * (limit / sp)
    return y, float(loudness) if np.isfinite(loudness) else None


def peak_dbfs(y):
    return float(20 * np.log10(np.max(np.abs(y)) + 1e-12))


def pipeline(y, sr, *, denoise=True, do_declick=True, bwe=False, shelf_db=0.0, exciter_mix=0.0, note=""):
    meta = {"steps": [], "note": note}
    meta["bw_in"] = bandwidth(y, sr)
    if denoise:
        y = spectral_gate_denoise(y, sr)
        meta["steps"].append("denoise_stationary_prop0.55")
    if do_declick:
        y, nfix = declick(y, sr)
        meta["steps"].append(f"declick_fixed={nfix}")
    if bwe:
        y = spectral_mirror_bwe(y, sr)
        meta["steps"].append("spectral_mirror_bwe_ESTIMATED")
    if shelf_db:
        y = high_shelf(y, sr, freq=2800.0, gain_db=shelf_db)
        meta["steps"].append(f"high_shelf_2800Hz_{shelf_db}dB")
    if exciter_mix:
        y = harmonic_exciter(y, sr, mix=exciter_mix)
        meta["steps"].append(f"harmonic_exciter_mix={exciter_mix}")
    y, in_lufs = loudness_normalize(y, sr)
    meta["steps"].append(f"loudness_norm_target={TARGET_LUFS}_LUFS_tp<={TRUE_PEAK_DBTP}")
    meta["input_lufs"] = in_lufs
    meta["bw_out"] = bandwidth(y, sr)
    meta["peak_dbfs_out"] = round(peak_dbfs(y), 2)
    # measure output LUFS
    try:
        meta["output_lufs"] = round(float(pyln.Meter(sr).integrated_loudness(y)), 2)
    except Exception:
        meta["output_lufs"] = None
    return y, meta


def write_wav(path, y, sr):
    y = np.clip(y, -1.0, 1.0)
    sf.write(str(path), y.astype(np.float32), sr, subtype="PCM_16")


def main():
    orig_path = BASE / "inputs" / "original.wav"
    ref_path = BASE / "inputs" / "reference.wav"

    # work from original at 44.1k then downsample for max processing headroom,
    # and also from reference at 24k for closer-to-current-clone path
    orig44, sr44 = load_mono(orig_path, target_sr=None)
    assert sr44 == 44100
    ref24, _ = load_mono(ref_path, target_sr=TARGET_SR)
    orig24 = librosa.resample(orig44, orig_sr=sr44, target_sr=TARGET_SR)

    variants = {}

    # A: cleanup only from reference (control — no HF invent)
    y, m = pipeline(
        ref24.copy(),
        TARGET_SR,
        denoise=True,
        do_declick=True,
        bwe=False,
        shelf_db=0.0,
        exciter_mix=0.0,
        note="control: dual-mono→mono already; denoise+declick+loudness; NO HF restore",
    )
    variants["A-clean"] = (y, m)

    # B: mild shelf+exciter on cleaned ref
    y, m = pipeline(
        ref24.copy(),
        TARGET_SR,
        denoise=True,
        do_declick=True,
        bwe=False,
        shelf_db=5.0,
        exciter_mix=0.14,
        note="ESTIMATED HF via high-shelf + mild harmonic exciter",
    )
    variants["B-shelf"] = (y, m)

    # C: spectral mirror BWE + shelf + mild exciter on cleaned ref
    y, m = pipeline(
        ref24.copy(),
        TARGET_SR,
        denoise=True,
        do_declick=True,
        bwe=True,
        shelf_db=4.0,
        exciter_mix=0.12,
        note="ESTIMATED HF via spectral-mirror BWE + shelf + exciter",
    )
    variants["C-bwe"] = (y, m)

    # D: process at 44.1k from original (BWE to ~10k) then downsample to 24k
    y44, m44 = pipeline(
        orig44.copy(),
        sr44,
        denoise=True,
        do_declick=True,
        bwe=True,
        shelf_db=4.5,
        exciter_mix=0.13,
        note="ESTIMATED HF at 44.1k from original then downsample to 24k",
    )
    # extend BWE destination effectively by running at 44.1k; downsample
    y24 = librosa.resample(y44, orig_sr=sr44, target_sr=TARGET_SR)
    # re-limit after resample
    y24, _ = loudness_normalize(y24, TARGET_SR)
    m44["bw_out_24k"] = bandwidth(y24, TARGET_SR)
    m44["peak_dbfs_out_24k"] = round(peak_dbfs(y24), 2)
    variants["D-orig44-bwe"] = (y24, m44)

    report = {"disclaimer": "Any energy above ~2.7 kHz is ESTIMATED / synthesized, not recovered from the recording.", "variants": {}}
    for name, (y, meta) in variants.items():
        path = OUT / f"samantha-ref-{name}.wav"
        write_wav(path, y, TARGET_SR)
        meta["path"] = str(path)
        meta["sr"] = TARGET_SR
        meta["duration_s"] = round(len(y) / TARGET_SR, 3)
        report["variants"][name] = meta
        print(name, meta["bw_in"], "->", meta.get("bw_out") or meta.get("bw_out_24k"), "peak", meta.get("peak_dbfs_out") or meta.get("peak_dbfs_out_24k"), flush=True)

    # also dump baseline bandwidth of inputs
    report["inputs"] = {
        "original": {"path": str(orig_path), **bandwidth(orig44, sr44)},
        "reference": {"path": str(ref_path), **bandwidth(ref24, TARGET_SR)},
    }
    with open(OUT / "variants_meta.json", "w") as f:
        json.dump(report, f, indent=2)
    print("Wrote", OUT / "variants_meta.json")


if __name__ == "__main__":
    main()
