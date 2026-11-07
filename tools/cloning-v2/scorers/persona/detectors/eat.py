"""EfficientAT passthrough for yelling and laughter only.

Crying is owned by CryingDetector (emotion2vec+ sad). This module still
exposes the raw EfficientAT crying value under extras of yelling/laughter
records? No — it simply does not emit a crying decision score, so it cannot
override CryingDetector.
"""
from __future__ import annotations

from typing import Any, Mapping

from .base import DetectorScore, eat_max

DECISION_STATES = ("yelling", "laughter")
SOURCE = "efficientat"


class EatDetector:
    name = "efficientat-yelling-laughter"
    categories = DECISION_STATES

    def score(self, rec: Mapping[str, Any]) -> dict[str, DetectorScore]:
        mx = eat_max(rec)
        out = {}
        for cat in DECISION_STATES:
            out[cat] = DetectorScore(
                category=cat,
                value=float(mx.get(cat, 0.0)),
                source=SOURCE,
                extras={"efficientat_crying_logged": mx.get("crying")} if mx.get("crying") is not None else {},
            )
        return out
