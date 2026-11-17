# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Galion Labs.
"""``--scorer-module emotion2vec_gate`` for the emotion-v2 dataset pipeline (#182).

Glue only. ``build_scorers(config)`` returns one scorer named ``emotion``.
For a clip it returns a score for every emotion that has a reference: the
locked formula ``0.70*category_blend + 0.30*emb_sim`` (histogram intersection
of the emotion2vec+ posteriors over the gate set, cosine of the emotion2vec+
embeddings) against that emotion's reference clip. The pipeline checks the
clip's own emotion at 0.83 (unchanged). The voice is an input: references
come from a sector-refs manifest (``tools/cloning-v2/emotion-match`` schema
``galion-emotion-v2-sector-refs/1``, path + sha256 per emotion) named by
``EMOTION_V2_SECTOR_REFS``. No manifest, or no reference for the clip's
emotion, means no score for it and the pipeline refuses the clip.

Recogniser: Rust ``galion-emotion12`` (emotion2vec+ via ort). ``config``
``device`` is passed through (``cpu`` default; ``auto`` uses CUDA only when
present).
"""

from __future__ import annotations

import os
import sys
import tempfile
import wave
from pathlib import Path

_HERE = Path(__file__).resolve().parent
if str(_HERE) not in sys.path:
    sys.path.insert(0, str(_HERE))

from harness import scorer12  # noqa: E402
from harness.metrics import cosine, l2_normalize  # noqa: E402

CATEGORY_W = 0.70
EMB_W = 0.30


def _load_refs(manifest: str) -> dict[str, Path]:
    sys.path.insert(0, str(_HERE.parents[0] / "cloning-v2" / "emotion-match"))
    from sector_refs import load_manifest  # reuse: path + sha256 checks

    loaded = load_manifest(Path(manifest))
    return {emotion: ref.path for emotion, ref in loaded.references.items()}


def _write_wav(path: Path, samples, rate: int) -> None:
    import numpy as np

    pcm = (np.clip(np.asarray(samples, dtype=np.float64), -1.0, 1.0) * 32767.0).astype("<i2")
    with wave.open(str(path), "wb") as handle:
        handle.setnchannels(1)
        handle.setsampwidth(2)
        handle.setframerate(int(rate))
        handle.writeframes(pcm.tobytes())


class Emotion2VecGateScorer:
    name = "emotion"

    def __init__(self, config: dict) -> None:
        self.device = str(config.get("device") or "cpu")
        self.threads = config.get("num_threads")
        manifest = os.environ.get("EMOTION_V2_SECTOR_REFS")
        self.refs = _load_refs(manifest) if manifest else {}
        self._ref_rows: dict[str, dict] | None = None

    def _refs(self) -> dict[str, dict]:
        if self._ref_rows is None:
            rows = scorer12.run_batch(list(self.refs.values()), device=self.device, threads=self.threads) if self.refs else {}
            self._ref_rows = {e: rows[str(p)] for e, p in self.refs.items()}
        return self._ref_rows

    def score(self, clip) -> dict[str, float]:
        refs = self._refs()
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "clip.wav"
            _write_wav(path, clip.audio.samples, clip.audio.sample_rate)
            row = scorer12.run_batch([path], device=self.device, threads=self.threads)[str(path)]
        emb = l2_normalize(row["embedding"])
        out: dict[str, float] = {}
        for emotion, ref in refs.items():
            category = sum(min(a, b) for a, b in zip(row["posterior"], ref["posterior"]))
            sim = cosine(emb, l2_normalize(ref["embedding"]))
            out[emotion] = CATEGORY_W * category + EMB_W * sim
        return out


def build_scorers(config: dict) -> list:
    return [Emotion2VecGateScorer(config)]
