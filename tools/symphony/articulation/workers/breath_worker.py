# Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
"""Optional breath detector. Runs only when GALION_YAMNET points at a local
YAMNet SavedModel or TFHub cache. Otherwise it records the gap and the Rust
voicing-gap cues stand. Nothing is downloaded.

usage: python breath_worker.py IN.wav OUT.json
"""
import json
import os
import sys


def main():
    wav, out = sys.argv[1], sys.argv[2]
    path = os.environ.get("GALION_YAMNET", "")
    if not path or not os.path.exists(path):
        json.dump(
            {
                "source": "gap",
                "gap": "GALION_YAMNET is unset; breath cues stay on the praatfan voicing gap. CPPS is not computed here.",
                "cues": [],
            },
            open(out, "w"),
        )
        return
    import tensorflow_hub as hub
    import tensorflow as tf
    import numpy as np
    import soundfile as sf

    audio, sr = sf.read(wav, always_2d=False)
    if audio.ndim > 1:
        audio = audio.mean(axis=1)
    if sr != 16000:
        json.dump({"source": "gap", "gap": "YAMNet expects 16 kHz; resample upstream", "cues": []}, open(out, "w"))
        return
    model = hub.load(path)
    scores, _, _ = model(tf.constant(audio, tf.float32))
    class_map = tf.constant(model.class_map_path().numpy().decode("utf-8")) if hasattr(model, "class_map_path") else None
    del class_map
    # AudioSet "Breathing" is class 37 in the published YAMNet map.
    breath = scores.numpy()[:, 37]
    hop = 0.48
    cues = []
    for i, p in enumerate(breath):
        if float(p) >= 0.3:
            cues.append({"start_s": i * hop, "end_s": (i + 1) * hop, "kind": "inhale", "loudness_dbfs": 0.0})
    json.dump({"source": "yamnet", "cues": cues, "n": int(np.sum(breath >= 0.3))}, open(out, "w"))


if __name__ == "__main__":
    main()
