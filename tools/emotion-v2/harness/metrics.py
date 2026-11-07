# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Comparisons used by the emotion gate. Pure functions, no audio I/O."""

from __future__ import annotations

import math


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


def l2_normalize(vector: list[float]) -> list[float]:
    norm = math.sqrt(sum(value * value for value in vector))
    if norm == 0.0:
        return [0.0] * len(vector)
    return [value / norm for value in vector]


def cosine(left: list[float], right: list[float]) -> float:
    if len(left) != len(right) or not left:
        return 0.0
    dot = sum(a * b for a, b in zip(left, right))
    left_norm = math.sqrt(sum(value * value for value in left))
    right_norm = math.sqrt(sum(value * value for value in right))
    if left_norm == 0.0 or right_norm == 0.0:
        return 0.0
    return dot / (left_norm * right_norm)


def mean_vector(rows: list[list[float]]) -> list[float]:
    if not rows:
        return []
    width = len(rows[0])
    if any(len(row) != width for row in rows):
        raise ValueError("cannot average vectors of different lengths")
    acc = [0.0] * width
    for row in rows:
        for index, value in enumerate(row):
            acc[index] += value
    scale = 1.0 / len(rows)
    return [value * scale for value in acc]


def meets_min(score: float, target: float) -> bool:
    """Inclusive lower bound. `target` itself passes."""
    return score >= target or math.isclose(score, target, rel_tol=0.0, abs_tol=1e-9)


def wer_passes(wer_mean: float, wer_max: float) -> bool:
    """Inclusive. WER must stay at or below the Samantha merge-v2 ceiling."""
    return wer_mean <= wer_max or math.isclose(wer_mean, wer_max, rel_tol=0.0, abs_tol=1e-12)


def ecapa_passes(ecapa_mean: float, ecapa_min: float) -> bool:
    """Strict. The merge-v2 baseline itself is not an improvement."""
    if math.isclose(ecapa_mean, ecapa_min, rel_tol=0.0, abs_tol=1e-12):
        return False
    return ecapa_mean > ecapa_min
