"""CLAP zero-shot for logic / reason (unchanged)."""
from __future__ import annotations

from typing import Any, Mapping

from .base import DetectorScore, clap_probs

SOURCE = "zero-shot text prompt (CLAP)"


class LogicDetector:
    name = "clap-logic"
    categories = ("logic", "reason")

    def score(self, rec: Mapping[str, Any]) -> dict[str, DetectorScore]:
        probs = clap_probs(rec)
        out = {}
        for cat in self.categories:
            out[cat] = DetectorScore(
                category=cat,
                value=float(probs.get(cat, 0.0)),
                source=SOURCE,
            )
        return out
