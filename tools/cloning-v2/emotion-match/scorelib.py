# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
# emotion_match = 0.70*category_blend+0.30*emb_sim
#
# Pure functions. No audio I/O, no weights, no network. The weights and the
# 0.83 gate are locked: callers may report them, not change them.

from __future__ import annotations

import math
from dataclasses import dataclass, field

FORMULA = "0.70*category_blend+0.30*emb_sim"
CATEGORY_W = 0.70
EMB_W = 0.30
GATE = 0.83


def emotion_match(category_blend: float, emb_sim: float) -> float:
    """Evaluate 0.70*category_blend+0.30*emb_sim."""
    return CATEGORY_W * float(category_blend) + EMB_W * float(emb_sim)


def softmax(logits: list[float]) -> list[float]:
    if not logits:
        return []
    peak = max(logits)
    shifted = [math.exp(value - peak) for value in logits]
    total = sum(shifted)
    if total == 0.0:
        share = 1.0 / len(logits)
        return [share] * len(logits)
    return [value / total for value in shifted]


def as_prob(row: list[float]) -> list[float]:
    values = [float(item) for item in row]
    total = sum(values)
    if any(item < -1e-5 for item in values) or abs(total - 1.0) > 1e-2:
        return softmax(values)
    if total == 0.0:
        return [0.0] * len(values)
    return [item / total for item in values]


def as_unit(emb: list[float]) -> list[float]:
    values = [float(item) for item in emb]
    norm = math.sqrt(sum(item * item for item in values))
    if norm == 0.0:
        return [0.0] * len(values)
    return [item / norm for item in values]


def intersection(left: list[float], right: list[float]) -> float:
    """Histogram intersection of two categorical posteriors."""
    n = min(len(left), len(right))
    return float(sum(min(left[i], right[i]) for i in range(n)))


def cosine(left: list[float], right: list[float]) -> float:
    """Dot product of two L2-normalised embeddings, truncated to the shorter."""
    n = min(len(left), len(right))
    return float(sum(left[i] * right[i] for i in range(n)))


def meets_gate(score: float, gate: float = GATE) -> bool:
    """Inclusive lower bound. The gate itself passes. The gate cannot be lowered."""
    if gate != GATE and not math.isclose(gate, GATE, rel_tol=0.0, abs_tol=1e-12):
        raise ValueError(f"emotion_match gate is locked at {GATE}, got {gate}")
    return float(score) >= GATE or math.isclose(float(score), GATE, rel_tol=0.0, abs_tol=1e-9)


def pair_score(render_prob: list[float], render_emb: list[float],
               ref_prob: list[float], ref_emb: list[float]) -> tuple[float, float, float]:
    """Return (category_blend, emb_sim, emotion_match) for one render vs its sector ref."""
    category = intersection(as_prob(render_prob), as_prob(ref_prob))
    sim = cosine(as_unit(render_emb), as_unit(ref_emb))
    return category, sim, emotion_match(category, sim)


@dataclass
class ClipScore:
    path: str
    emotion: str
    line_id: str | None
    category_blend: float
    emb_sim: float
    emotion_match: float

    def as_dict(self) -> dict[str, object]:
        return {
            "path": self.path,
            "emotion": self.emotion,
            "line": self.line_id,
            "category_blend": _round(self.category_blend),
            "emb_sim": _round(self.emb_sim),
            "emotion_match": _round(self.emotion_match),
        }


@dataclass
class SectorScore:
    emotion: str
    n: int
    ref_path: str
    ref_sha256: str
    category_blend: float
    emb_sim: float
    emotion_match: float
    passed: bool
    files: list[ClipScore] = field(default_factory=list)

    def as_dict(self) -> dict[str, object]:
        return {
            "emotion": self.emotion,
            "n": self.n,
            "ref": {"path": self.ref_path, "sha256": self.ref_sha256},
            "category_blend": _round(self.category_blend),
            "emb_sim": _round(self.emb_sim),
            "emotion_match": _round(self.emotion_match),
            "gate": GATE,
            "pass": self.passed,
            "files": [item.as_dict() for item in self.files],
        }


@dataclass
class PerSectorReport:
    sectors: list[SectorScore]
    n: int
    category_blend: float
    emb_sim: float
    emotion_match: float
    min_emotion_match: float | None
    passed: bool
    persona: str | None = None
    backend: str = "stub"

    @property
    def n_sectors(self) -> int:
        return len(self.sectors)

    @property
    def n_passed(self) -> int:
        return sum(1 for row in self.sectors if row.passed)

    def as_dict(self) -> dict[str, object]:
        sector_rows = [row.as_dict() for row in self.sectors]
        files = [item.as_dict() for row in self.sectors for item in row.files]
        summary = {
            "category_blend": _round(self.category_blend),
            "emb_sim": _round(self.emb_sim),
            "emotion_match": _round(self.emotion_match),
            "min_emotion_match": _round(self.min_emotion_match),
            "n_sectors": self.n_sectors,
            "n_passed": self.n_passed,
            "pass": self.passed,
            "gate": GATE,
            "sectors": [
                {
                    "emotion": row.emotion,
                    "emotion_match": _round(row.emotion_match),
                    "pass": row.passed,
                    "n": row.n,
                }
                for row in self.sectors
            ],
        }
        return {
            "tool": "emotion_match",
            "formula": FORMULA,
            "gate": GATE,
            "mode": "per-sector",
            "backend": self.backend,
            "device": "cpu",
            "persona": self.persona,
            "n": self.n,
            "n_sectors": self.n_sectors,
            "n_passed": self.n_passed,
            "category_blend": _round(self.category_blend),
            "emb_sim": _round(self.emb_sim),
            "emotion_match": _round(self.emotion_match),
            "min_emotion_match": _round(self.min_emotion_match),
            "pass": self.passed,
            "sectors": {row.emotion: row.as_dict() for row in self.sectors},
            "files": files,
            "summary": summary,
        }


def aggregate(sectors: list[SectorScore], *, persona: str | None = None,
              backend: str = "stub") -> PerSectorReport:
    """Mean of the per-sector scores. Pass requires every sector to clear 0.83.

    One weak sector cannot hide inside the blended float. An empty sector
    list does not pass.
    """
    if not sectors:
        return PerSectorReport(
            sectors=[],
            n=0,
            category_blend=0.0,
            emb_sim=0.0,
            emotion_match=0.0,
            min_emotion_match=None,
            passed=False,
            persona=persona,
            backend=backend,
        )
    n = sum(row.n for row in sectors)
    # Mean of the mix equals the mix of the means, same as the single-ref tool.
    category = sum(row.category_blend for row in sectors) / len(sectors)
    sim = sum(row.emb_sim for row in sectors) / len(sectors)
    score = emotion_match(category, sim)
    lowest = min(row.emotion_match for row in sectors)
    passed = all(row.passed for row in sectors)
    return PerSectorReport(
        sectors=list(sectors),
        n=n,
        category_blend=category,
        emb_sim=sim,
        emotion_match=score,
        min_emotion_match=lowest,
        passed=passed,
        persona=persona,
        backend=backend,
    )


def score_sector(
    emotion: str,
    clips: list[ClipScore],
    ref_path: str,
    ref_sha256: str,
) -> SectorScore:
    if not clips:
        raise ValueError(f"no renders to score for sector {emotion}")
    n = len(clips)
    category = sum(item.category_blend for item in clips) / n
    sim = sum(item.emb_sim for item in clips) / n
    score = emotion_match(category, sim)
    return SectorScore(
        emotion=emotion,
        n=n,
        ref_path=ref_path,
        ref_sha256=ref_sha256,
        category_blend=category,
        emb_sim=sim,
        emotion_match=score,
        passed=meets_gate(score),
        files=list(clips),
    )


def _round(value: float | None) -> float | None:
    if value is None:
        return None
    return round(float(value), 6)
