# Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
"""Out-of-process bricks: DisVoice (MIT) glottal + phonation statics and Phonet (MIT)
phonological posteriors, called as-is (`pip install disvoice==0.1.10 phonet`).
No Rust IAIF/GCI, jitter/shimmer or phonological-posterior crate exists (searched
crates.io and GitHub, 2026-11-17), so these stay Python per RULES "RUST PRIMARY".
Glue only: resample to 16 kHz mono (what both tools expect) and dump JSON.

usage: python disvoice_worker.py IN.wav OUT.json
"""
import json, os, sys, tempfile
import numpy as np
import librosa, soundfile as sf

def statics(cls, path):
    df = cls().extract_features_file(path, static=True, plots=False, fmt="dataframe")
    return {str(k): float(v) for k, v in df.iloc[0].items() if np.isfinite(v)}

def main():
    wav, out = sys.argv[1], sys.argv[2]
    y, _ = librosa.load(wav, sr=16000, mono=True)
    tmp = tempfile.mkdtemp()
    p16 = os.path.join(tmp, "in16k.wav")
    sf.write(p16, y, 16000, subtype="PCM_16")
    from disvoice.glottal import Glottal
    from disvoice.phonation import Phonation
    j = {"glottal": statics(Glottal, p16), "phonation": statics(Phonation, p16)}
    from phonet.phonet import Phonet
    csv = os.path.join(tmp, "phonet.csv")
    ph = Phonet(["all"])
    df = ph.get_phon_wav(p16, csv, False)
    if df is None:
        import pandas as pd
        df = pd.read_csv(csv)
    t = df["time"].to_numpy()
    rate = 1.0 / float(np.median(np.diff(t))) if len(t) > 1 else 100.0
    j["phonet"] = {"rate_hz": rate, "t0": float(t[0]) if len(t) else 0.0,
                   "classes": {c: df[c].astype(float).tolist() for c in df.columns if c not in ("time", "phoneme")}}
    j["bricks"] = [
        {"name": "DisVoice (glottal, phonation)", "version": "0.1.10", "licence": "MIT", "lang": "Python", "process": "out of process"},
        {"name": "Phonet", "version": "0.3.7", "licence": "MIT", "lang": "Python/PyTorch", "process": "out of process"},
    ]
    json.dump(j, open(out, "w"))

if __name__ == "__main__":
    main()
