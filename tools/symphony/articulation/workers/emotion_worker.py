# Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
"""Utterance arousal / dominance / valence from the existing galion-emotion binary
(audeering wav2vec2 MSP-dim). This process does not load ort itself.

usage: GALION_EMOTION_BIN=galion-emotion python emotion_worker.py IN.wav OUT.json
Writes a gap when the binary is not set or not executable.
"""
import json
import os
import subprocess
import sys


def main():
    wav, out = sys.argv[1], sys.argv[2]
    binary = os.environ.get("GALION_EMOTION_BIN", "")
    if not binary or not os.path.isfile(binary) or not os.access(binary, os.X_OK):
        json.dump(
            {
                "source": "gap",
                "gap": "GALION_EMOTION_BIN is not an executable galion-emotion (MSP-dim)",
            },
            open(out, "w"),
        )
        return
    tmp = out + ".emotion.json"
    subprocess.check_call([binary, "score", "--no-ser", "--out", tmp, wav])
    doc = json.load(open(tmp))
    files = doc.get("files") or []
    avd = (files[0].get("avd") if files else None) or {}
    json.dump(
        {
            "source": "audeering/wav2vec2-large-robust-12-ft-emotion-msp-dim",
            "scale": "unit",
            "arousal": avd.get("arousal"),
            "valence": avd.get("valence"),
            "dominance": avd.get("dominance"),
        },
        open(out, "w"),
    )


if __name__ == "__main__":
    main()
