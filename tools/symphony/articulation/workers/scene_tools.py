# Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
"""Optional real-life front end for `galion-articulation select --tools`.

Each brick is someone else's program, run only when it is already on the
machine. This file does not vendor weights and does not download them.

  deep-filter   DeepFilterNet CLI (Rust), when `deep-filter` is on PATH
  demucs        htdemucs two-stem split, when `demucs` is on PATH
  pyannote      diarization, when pyannote.audio imports and a local pipeline
                path is in GALION_PYANNOTE
  silero        Silero VAD, when GALION_SILERO_VAD is a local model file
  galion-ecapa  speaker cosine vs the enrolment clip, when GALION_ECAPA_BIN is set

`probe` (default) only reports which of those are visible.
`run` actually calls the ones that are visible. Pass --enrol for ECAPA.
"""
import json
import os
import shutil
import subprocess
import sys


def visible():
    gaps = []
    have = []
    if shutil.which("deep-filter"):
        have.append("deep-filter")
    else:
        gaps.append("deep-filter: CLI not on PATH")
    if shutil.which("demucs"):
        have.append("demucs")
    else:
        gaps.append("demucs: CLI not on PATH")
    if os.environ.get("GALION_PYANNOTE"):
        have.append("pyannote")
    else:
        gaps.append("pyannote: GALION_PYANNOTE is unset (no network fetch)")
    if os.environ.get("GALION_SILERO_VAD") and os.path.exists(os.environ["GALION_SILERO_VAD"]):
        have.append("silero")
    else:
        gaps.append("silero-vad: GALION_SILERO_VAD is unset")
    if os.environ.get("GALION_ECAPA_BIN") and os.path.isfile(os.environ["GALION_ECAPA_BIN"]):
        have.append("galion-ecapa")
    else:
        gaps.append("galion-ecapa: GALION_ECAPA_BIN is unset; select falls back to F0 and formants")
    return have, gaps


def main():
    mode = sys.argv[1] if len(sys.argv) > 1 else "probe"
    out = sys.argv[2] if len(sys.argv) > 2 else None
    have, gaps = visible()
    doc = {"source": "scene-tools", "present": have, "gaps": gaps, "segments": []}
    if mode == "run" and "galion-ecapa" in have and os.environ.get("GALION_ENROL"):
        wav = sys.argv[3]
        tmp = (out or "ecapa.json") + ".ecapa.json"
        subprocess.check_call(
            [
                os.environ["GALION_ECAPA_BIN"],
                "--ref",
                os.environ["GALION_ENROL"],
                "--no-ceiling",
                "--raw",
                "--out",
                tmp,
                wav,
            ]
        )
        scores = json.load(open(tmp)).get("scores") or {}
        cos = next(iter(scores.values()), None)
        if cos is not None:
            doc["segments"].append(
                {"start_s": 0.0, "end_s": 1.0e9, "embedding_cos": float(cos), "music_noise": "speech"}
            )
    text = json.dumps(doc)
    if out:
        open(out, "w").write(text)
    else:
        print(text)


if __name__ == "__main__":
    main()
