"""Crying detector: emotion2vec+ sad, not EfficientAT.

The original-voice probe (scorer-probe/results/SUMMARY.json) showed
EfficientAT crying at 0 of 8 intended crying lines, while emotion2vec+ sad
rated the same lines 0.99999+ on the ones that actually sounded sad.
EfficientAT also false-fires on non-crying speech. EfficientAT crying is
logged in extras and is never the decision value.
"""
from __future__ import annotations

from typing import Any, Mapping

from .base import DetectorScore, e2v_probs, eat_max

SOURCE = "trained class (emotion2vec+ sad)"


class CryingDetector:
    name = "crying-e2v-sad"
    categories = ("crying",)

    def score(self, rec: Mapping[str, Any]) -> dict[str, DetectorScore]:
        probs = e2v_probs(rec)
        sad = float(probs.get("sad", 0.0))
        logged = eat_max(rec).get("crying")
        extras = {
            "e2v_sad": sad,
            "efficientat_crying_used_for_decision": False,
        }
        if logged is not None:
            extras["efficientat_crying_logged"] = float(logged)
        return {
            "crying": DetectorScore(
                category="crying",
                value=sad,
                source=SOURCE,
                used_for_decision=True,
                extras=extras,
            )
        }
