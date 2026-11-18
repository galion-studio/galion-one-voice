# Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
"""Out-of-process brick: SPARC (Berkeley Speech Group, MIT, weights MIT on HF
cheoljun95/Speech-Articulatory-Coding). Calls the upstream package as-is
(`pip install speech-articulatory-coding==0.1.0`) and writes its encode() output
as JSON for `galion-articulation analyze --sparc`. No Rust SPARC exists (searched
crates.io and GitHub, 2026-11-17), so this stays Python per RULES "RUST PRIMARY".

usage: python sparc_worker.py IN.wav OUT.json [model=en] [device=cuda:0|cpu]
"""
import json, sys
import numpy as np
from sparc import load_model

EMA = ["TDX", "TDY", "TBX", "TBY", "TTX", "TTY", "LIX", "LIY", "ULX", "ULY", "LLX", "LLY"]

def main():
    wav, out = sys.argv[1], sys.argv[2]
    model = sys.argv[3] if len(sys.argv) > 3 else "en"
    device = sys.argv[4] if len(sys.argv) > 4 else "cuda:0"
    coder = load_model(model, device=device)
    c = coder.encode(wav)
    j = {
        "rate_hz": 50.0,
        "ema_names": EMA,
        "ema": np.asarray(c["ema"]).tolist(),
        "pitch": np.asarray(c["pitch"]).reshape(-1).tolist(),
        "loudness": np.asarray(c["loudness"]).reshape(-1).tolist(),
        "periodicity": np.asarray(c["periodicity"]).reshape(-1).tolist(),
        "spk_emb": np.asarray(c["spk_emb"]).reshape(-1).tolist(),
        "bricks": [{"name": "SPARC (speech-articulatory-coding)", "version": f"0.1.0, model {model}",
                    "licence": "MIT (code and HF weights)", "lang": "Python/PyTorch",
                    "process": "out of process"}],
    }
    json.dump(j, open(out, "w"))

if __name__ == "__main__":
    main()
