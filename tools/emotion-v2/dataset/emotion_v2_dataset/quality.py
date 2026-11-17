# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Quality gate. High-quality-only by default.

Defaults are the Galion Voice V2 rollout gate (``tools/emotion-v2/thresholds.toml``):

* WER at or below 0.018 (inclusive upper bound),
* ECAPA strictly above 0.7057 (the Samantha merge-v2 baseline),
* emotion match at or above 0.83, checked per emotion class. Every one of the
  twelve classes has its own threshold; all twelve default to 0.83.

A missing scorer rejects the clip by default (``require_scorers=True``).
The only way to keep an unscored clip is :data:`SYNTHETIC_TEST_THRESHOLDS`,
which the pipeline accepts only for the synthetic stub voice
(``RunConfig.allow_unscored_synthetic_test``). Present scores are still
compared with the gate in that mode.

Scorers are pluggable: any object with ``.name`` in ``{'wer', 'ecapa',
'emotion'}`` and ``.score(clip)``. An emotion scorer may return either one
float (the match score for ``clip.emotion``) or a mapping of all twelve class
names to scores, for example a 12-class recogniser's posterior. For a mapping,
the clip's own class is used.
"""

from __future__ import annotations

from collections.abc import Callable, Mapping
from dataclasses import dataclass, field
from typing import Any, Protocol

from emotion_v2_dataset.audio import AudioBuffer
from emotion_v2_dataset.errors import QualityError
from emotion_v2_dataset.taxonomy import CANONICAL_EMOTIONS

SCORER_NAMES = ("wer", "ecapa", "emotion")

GATE_MAX_WER = 0.018
GATE_MIN_ECAPA = 0.7057  # exclusive: a score must be greater than this
GATE_MIN_EMOTION = 0.83


def gate_emotion_targets() -> dict[str, float]:
    return {name: GATE_MIN_EMOTION for name in CANONICAL_EMOTIONS}


@dataclass(frozen=True)
class QualityThresholds:
    """``None`` disables one comparison (synthetic-test mode only).

    * WER is an error rate: reject when the score is above ``max_wer``.
    * ECAPA is a similarity: reject when the score is at or below ``min_ecapa``.
    * Emotion: reject when the score is below the clip's class threshold in
      ``min_emotion_by_class``; ``min_emotion`` is the fallback for a class
      that is not listed.
    """

    max_wer: float | None = GATE_MAX_WER
    min_ecapa: float | None = GATE_MIN_ECAPA
    min_emotion: float | None = GATE_MIN_EMOTION
    require_scorers: bool = True
    min_emotion_by_class: Mapping[str, float] = field(default_factory=gate_emotion_targets)

    def emotion_threshold(self, emotion: str) -> float | None:
        value = self.min_emotion_by_class.get(emotion)
        if value is not None:
            return float(value)
        return self.min_emotion

    def as_dict(self) -> dict[str, Any]:
        return {
            "max_wer": self.max_wer,
            "min_ecapa": self.min_ecapa,
            "min_ecapa_exclusive": True,
            "min_emotion": self.min_emotion,
            "min_emotion_by_class": {k: float(v) for k, v in self.min_emotion_by_class.items()},
            "require_scorers": self.require_scorers,
        }


HIGH_QUALITY_THRESHOLDS = QualityThresholds()

# Opt-out for synthetic tests only. Missing scorers do not reject; present
# scores are still held to the gate. The pipeline refuses this for a real
# voice or a non-stub backend.
SYNTHETIC_TEST_THRESHOLDS = QualityThresholds(require_scorers=False)


def high_quality_violations(thresholds: QualityThresholds) -> list[str]:
    """Ways ``thresholds`` is looser than the gate. Empty means high quality."""
    problems: list[str] = []
    if not thresholds.require_scorers:
        problems.append("require_scorers is off")
    if thresholds.max_wer is None or thresholds.max_wer > GATE_MAX_WER:
        problems.append(f"max_wer {thresholds.max_wer} is looser than {GATE_MAX_WER}")
    if thresholds.min_ecapa is None or thresholds.min_ecapa < GATE_MIN_ECAPA:
        problems.append(f"min_ecapa {thresholds.min_ecapa} is looser than {GATE_MIN_ECAPA}")
    for emotion in CANONICAL_EMOTIONS:
        value = thresholds.emotion_threshold(emotion)
        if value is None or value < GATE_MIN_EMOTION:
            problems.append(f"emotion {emotion} threshold {value} is looser than {GATE_MIN_EMOTION}")
    return problems


def require_high_quality(thresholds: QualityThresholds) -> None:
    problems = high_quality_violations(thresholds)
    if problems:
        raise QualityError(
            "high-quality-only: " + "; ".join(problems)
            + ". Use allow_unscored_synthetic_test only for synthetic stub tests."
        )


def thresholds_from_toml(path: Any) -> QualityThresholds:
    """Read ``tools/emotion-v2/thresholds.toml`` (guards + per-class emotion targets)."""
    import tomllib
    from pathlib import Path

    data = tomllib.loads(Path(path).read_text(encoding="utf-8"))
    try:
        guards = data["guards"]
        emotion = data["emotion"]
        target = float(emotion["target"])
        by_lower = {str(k).lower(): float(v) for k, v in emotion.get("targets", {}).items()}
    except KeyError as exc:
        raise QualityError(f"thresholds file is missing {exc}") from exc
    by_class = {name: by_lower.get(name.lower(), target) for name in CANONICAL_EMOTIONS}
    return QualityThresholds(
        max_wer=float(guards["wer_max"]),
        min_ecapa=float(guards["ecapa_min"]),
        min_emotion=target,
        require_scorers=True,
        min_emotion_by_class=by_class,
    )


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

    def score(self, clip: "RenderedClip") -> float | Mapping[str, float]:
        """One scalar, or (emotion only) a mapping of class name to score."""


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
        if name == "emotion" and isinstance(value, Mapping):
            value = _class_score(value, clip.emotion)
        if isinstance(value, bool) or not isinstance(value, (int, float)):
            raise QualityError(f"scorer {name} returned a non-numeric score")
        setattr(card, name, float(value))
    return card, tuple(unused)


def _class_score(scores: Mapping[str, Any], emotion: str) -> float:
    folded = {str(key).casefold(): value for key, value in scores.items()}
    value = folded.get(emotion.casefold())
    if value is None:
        raise QualityError(f"emotion scorer returned no score for class {emotion}")
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise QualityError(f"emotion scorer returned a non-numeric score for {emotion}")
    return float(value)


def decide(card: ScoreCard, thresholds: QualityThresholds, emotion: str | None = None) -> QualityDecision:
    """``emotion`` selects the per-class threshold. Without it, ``min_emotion`` applies."""
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
        and card.ecapa <= thresholds.min_ecapa
    ):
        return QualityDecision(
            accepted=False,
            reason=f"ecapa {card.ecapa:.4f} not above {thresholds.min_ecapa:.4f}",
            scores=card,
        )
    limit = thresholds.emotion_threshold(emotion) if emotion is not None else thresholds.min_emotion
    if card.emotion is not None and limit is not None and card.emotion < limit:
        label = f"emotion {emotion}" if emotion is not None else "emotion"
        return QualityDecision(
            accepted=False,
            reason=f"{label} {card.emotion:.4f} below {limit:.4f}",
            scores=card,
        )
    return QualityDecision(accepted=True, reason="pass", scores=card)


@dataclass
class FunctionScorer:
    """Small adapter so tests and plugins can pass a plain function."""

    name: str
    function: Callable[[RenderedClip], Any]

    def score(self, clip: RenderedClip) -> Any:
        value = self.function(clip)
        if isinstance(value, Mapping):
            return value
        return float(value)
