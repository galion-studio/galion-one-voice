# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Quality gate. WER, ECAPA, and emotion scores apply only when a scorer exists.

Missing scorers do not reject a clip unless ``require_scorers`` is set. This
package does not bundle speech-recognition or speaker-embedding models. The
CI branch can supply them through ``emotion_v2_ci.build_scorers``.
"""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass
from typing import Any, Protocol

from emotion_v2_dataset.audio import AudioBuffer
from emotion_v2_dataset.errors import QualityError

SCORER_NAMES = ("wer", "ecapa", "emotion")


@dataclass(frozen=True)
class QualityThresholds:
    """``None`` disables that one comparison. Defaults match a strict filter.

    * WER is an error rate: reject when the score is above ``max_wer``.
    * ECAPA is a similarity: reject when the score is below ``min_ecapa``.
    * Emotion is a match score: reject when the score is below ``min_emotion``.
    """

    max_wer: float | None = 0.35
    min_ecapa: float | None = 0.50
    min_emotion: float | None = 0.40
    require_scorers: bool = False


@dataclass
class ScoreCard:
    wer: float | None = None
    ecapa: float | None = None
    emotion: float | None = None

    def as_dict(self) -> dict[str, float | None]:
        return {"wer": self.wer, "ecapa": self.ecapa, "emotion": self.emotion}


@dataclass
class QualityDecision:
    accepted: bool
    reason: str
    scores: ScoreCard
    unused_scorers: tuple[str, ...] = ()


class Scorer(Protocol):
    name: str

    def score(self, clip: "RenderedClip") -> float:
        """Return one scalar. WER is an error rate; the others are similarities."""


@dataclass
class RenderedClip:
    audio: AudioBuffer
    text: str
    emotion: str
    voice_id: str
    intensity: float


def score_clip(clip: RenderedClip, scorers: list[Any]) -> tuple[ScoreCard, tuple[str, ...]]:
    card = ScoreCard()
    unused: list[str] = []
    for scorer in scorers:
        name = getattr(scorer, "name", None)
        score_fn = getattr(scorer, "score", None)
        if not isinstance(name, str) or not callable(score_fn):
            raise QualityError("a scorer needs a string name and a score() method")
        if name not in SCORER_NAMES:
            unused.append(name)
            continue
        value = score_fn(clip)
        if isinstance(value, bool) or not isinstance(value, (int, float)):
            raise QualityError(f"scorer {name} returned a non-numeric score")
        setattr(card, name, float(value))
    return card, tuple(unused)


def decide(card: ScoreCard, thresholds: QualityThresholds) -> QualityDecision:
    missing = [name for name in SCORER_NAMES if getattr(card, name) is None]
    if thresholds.require_scorers and missing:
        return QualityDecision(
            accepted=False,
            reason="missing scorers: " + ",".join(missing),
            scores=card,
        )
    if card.wer is not None and thresholds.max_wer is not None and card.wer > thresholds.max_wer:
        return QualityDecision(
            accepted=False,
            reason=f"wer {card.wer:.4f} above {thresholds.max_wer:.4f}",
            scores=card,
        )
    if (
        card.ecapa is not None
        and thresholds.min_ecapa is not None
        and card.ecapa < thresholds.min_ecapa
    ):
        return QualityDecision(
            accepted=False,
            reason=f"ecapa {card.ecapa:.4f} below {thresholds.min_ecapa:.4f}",
            scores=card,
        )
    if (
        card.emotion is not None
        and thresholds.min_emotion is not None
        and card.emotion < thresholds.min_emotion
    ):
        return QualityDecision(
            accepted=False,
            reason=f"emotion {card.emotion:.4f} below {thresholds.min_emotion:.4f}",
            scores=card,
        )
    return QualityDecision(accepted=True, reason="pass", scores=card)


@dataclass
class FunctionScorer:
    """Small adapter so tests and plugins can pass a plain function."""

    name: str
    function: Callable[[RenderedClip], float]

    def score(self, clip: RenderedClip) -> float:
        return float(self.function(clip))
