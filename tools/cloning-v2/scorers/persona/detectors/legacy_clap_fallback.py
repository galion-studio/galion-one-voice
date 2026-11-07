"""Temporary love/seductive backend: unmodified universal CLAP probs.

Used only when no `love_seductive*.py` plugin is present. Agent 2 replaces
this path by adding that plugin; load_detectors() then skips this fallback.
"""
from __future__ import annotations

from typing import Any, Mapping

from .base import DetectorScore, clap_probs

SOURCE = "legacy-clap-fallback (awaiting love_seductive plugin)"


class LegacyClapLoveSeductiveFallback:
    name = "legacy-clap-love-seductive"
    categories = ("love", "seductive")

    def score(self, rec: Mapping[str, Any]) -> dict[str, DetectorScore]:
        probs = clap_probs(rec)
        return {
            cat: DetectorScore(
                category=cat,
                value=float(probs.get(cat, 0.0)),
                source=SOURCE,
                extras={"plugin": "absent", "fallback": "universal-clap-probs"},
            )
            for cat in self.categories
        }
