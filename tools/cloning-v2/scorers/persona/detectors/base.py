"""Detector protocol for the persona scorer.

A detector reads an already-scored record (emotion2vec+ / CLAP / EfficientAT
fields, same shapes as scorers/universal/py/score_universal.py) and returns
per-category decision scores.

Agent 2 plugs love and seductive in by adding
`scorers/persona/detectors/love_seductive*.py` that exports one of:
  - class LoveSeductiveDetector
  - build_detector()
  - DETECTOR
The module is discovered automatically; this package never implements those
two categories itself (legacy CLAP fallback lives in legacy_clap_fallback.py
and is used only when no plugin is present).
"""
from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Mapping, Protocol, Sequence


@dataclass
class DetectorScore:
    category: str
    value: float
    source: str
    used_for_decision: bool = True
    extras: dict[str, Any] = field(default_factory=dict)

    def to_dict(self) -> dict[str, Any]:
        return {
            "category": self.category,
            "value": float(self.value),
            "source": self.source,
            "used_for_decision": self.used_for_decision,
            "extras": self.extras,
        }


class Detector(Protocol):
    name: str
    categories: Sequence[str]

    def score(self, rec: Mapping[str, Any]) -> dict[str, DetectorScore]:
        """Return decision scores keyed by category id (catalog names)."""


def e2v_probs(rec: Mapping[str, Any]) -> dict[str, float]:
    """Accept score_universal, universal_match cache, or scorer-probe SUMMARY shapes."""
    e2v = rec.get("e2v") or {}
    if isinstance(e2v.get("probs"), dict):
        return {k: float(v) for k, v in e2v["probs"].items() if _is_num(v)}
    # SUMMARY.json stores the 9-class map directly under e2v
    out = {}
    for k, v in e2v.items():
        if _is_num(v):
            out[k] = float(v)
    return out


def clap_probs(rec: Mapping[str, Any]) -> dict[str, float]:
    clap = rec.get("clap") or {}
    if isinstance(clap.get("probs"), dict):
        return {k: float(v) for k, v in clap["probs"].items() if _is_num(v)}
    probs = rec.get("clap_probs") or {}
    return {k: float(v) for k, v in probs.items() if _is_num(v)}


def clap_cosine(rec: Mapping[str, Any]) -> dict[str, float]:
    clap = rec.get("clap") or {}
    if isinstance(clap.get("cosine"), dict):
        return {k: float(v) for k, v in clap["cosine"].items() if _is_num(v)}
    cos = rec.get("clap_cosine") or {}
    return {k: float(v) for k, v in cos.items() if _is_num(v)}


def eat_max(rec: Mapping[str, Any]) -> dict[str, float]:
    eat = rec.get("efficientat") or rec.get("efficientat_max") or {}
    out = {}
    for k, v in eat.items():
        if _is_num(v):
            out[k] = float(v)
        elif isinstance(v, dict) and _is_num(v.get("max")):
            out[k] = float(v["max"])
    return out


def _is_num(v: Any) -> bool:
    return isinstance(v, (int, float)) and not isinstance(v, bool)
