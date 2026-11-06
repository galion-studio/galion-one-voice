"""emotion2vec+ trained-class detectors (unchanged mapping)."""
from __future__ import annotations

from typing import Any, Mapping

from .base import DetectorScore, e2v_probs

# catalog category -> e2v probability key
E2V_TRAINED = {
    "joyful": "happy",
    "aggressive": "angry",
    "disgust": "disgusted",
    "fear": "fearful",
    "sad": "sad",
    "neutral": "neutral",
    "surprise": "surprised",
}
SOURCE = "trained class (emotion2vec+)"


class TrainedDetector:
    name = "trained-emotion2vec+"
    categories = tuple(E2V_TRAINED)

    def score(self, rec: Mapping[str, Any]) -> dict[str, DetectorScore]:
        probs = e2v_probs(rec)
        out = {}
        for cat, key in E2V_TRAINED.items():
            out[cat] = DetectorScore(
                category=cat,
                value=float(probs.get(key, 0.0)),
                source=SOURCE,
            )
        return out
