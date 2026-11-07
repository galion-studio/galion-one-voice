# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Smooth Emotion V2 posteriors into Nebula live frames.

The input is a posterior the existing scorer or engine already produced:
twelve Emotion V2 weights, plus seductive when the voice is Samantha, and
optional valence, arousal, dominance, and speech energy. The output is a
smoothed frame. This module does not score audio, does not condition a
voice, and does not write the 8-byte EmotionWire.

The smoother is an exponential moving average with a time constant ``tau_ms``
in milliseconds:

    alpha = 1 - exp(-dt_ms / tau_ms)
    y = y + alpha * (x - y)

``dt_ms`` is the gap since that channel's previous measurement. The first
measurement of a channel is copied. A hold, or a posterior that leaves a
channel out, repeats the last value and does not move it. Before any
measurement the frame is an absence: weights are null, not an invented
Neutral of 1.

``HEARTBEAT_MS`` is the locked 80 ms clock. It is not a parameter.
"""

from __future__ import annotations

import json
import math
from dataclasses import dataclass

from .gas import GAS_LINES, SEDUCTIVE, VOICES, emotions_for

SCHEMA = "galion-nebula-live-feed/1"
# Side-stream default. Not GatedDetector's 0.5 and not Rika's 0.08.
# At the 80 ms heartbeat, alpha = 1 - exp(-80/240) ≈ 0.283.
DEFAULT_TAU_MS = 240.0
# LOCKS.toml clock.frame_ms. Shared by audio and emotion. Not configurable.
HEARTBEAT_MS = 80
# A complete posterior may miss 1 by this much (rounded JSON, float noise).
SUM_TOLERANCE = 1e-3
KINDS = ("frame", "utterance")


class FeedError(ValueError):
    """The posterior or the clock cannot be turned into a frame."""


def ema_alpha(dt_ms: float, tau_ms: float) -> float:
    """EMA step for a gap of ``dt_ms`` and a time constant ``tau_ms``."""
    if not math.isfinite(tau_ms) or tau_ms <= 0.0:
        raise FeedError("tau_ms must be a positive number of milliseconds")
    if not math.isfinite(dt_ms) or dt_ms <= 0.0:
        raise FeedError("dt_ms must be a positive number of milliseconds")
    return 1.0 - math.exp(-dt_ms / tau_ms)


def ema(previous: float, sample: float, alpha: float) -> float:
    """One EMA step. ``alpha`` is ``ema_alpha`` for the gap just observed."""
    return previous + alpha * (sample - previous)


def _tau(value: float) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise FeedError("tau_ms must be a positive number of milliseconds")
    tau = float(value)
    if not math.isfinite(tau) or tau <= 0.0:
        raise FeedError("tau_ms must be a positive number of milliseconds")
    return tau


def _voice(voice_id: str) -> str:
    if not isinstance(voice_id, str):
        raise FeedError("voice_id must be a string")
    key = voice_id.strip().lower()
    if key not in VOICES:
        known = ", ".join(VOICES)
        raise FeedError(f"unknown voice {voice_id!r}; expected one of {known}")
    return key


def _t_ms(value: object) -> int:
    if isinstance(value, bool):
        raise FeedError("t_ms must be an integer number of milliseconds")
    if isinstance(value, int):
        stamp = value
    elif isinstance(value, float) and math.isfinite(value) and value.is_integer():
        stamp = int(value)
    else:
        raise FeedError("t_ms must be an integer number of milliseconds")
    if stamp < 0:
        raise FeedError("t_ms must be >= 0")
    return stamp


def _finite(value: object, name: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise FeedError(f"{name} must be a number")
    number = float(value)
    if not math.isfinite(number):
        raise FeedError(f"{name} must be finite")
    return number


def _optional(value: object, name: str, low: float, high: float) -> float | None:
    if value is None:
        return None
    number = _finite(value, name)
    if number < low or number > high:
        raise FeedError(f"{name} {number} is outside {low}..{high}")
    return number


def _weight(value: object, name: str) -> float:
    number = _finite(value, name)
    if number < 0.0 or number > 1.0:
        raise FeedError(f"{name} weight {number} is outside 0..1")
    return number


def _json_number(value: float | None) -> int | float | None:
    if value is None:
        return None
    rounded = round(float(value), 6)
    if rounded == int(rounded) and abs(rounded) < 1e15:
        return int(rounded)
    return rounded


@dataclass(frozen=True)
class Posterior:
    """One scorer observation. ``weights`` is None when this tick has no posterior."""

    t_ms: int
    kind: str
    weights: dict[str, float] | None
    valence: float | None
    arousal: float | None
    dominance: float | None
    energy: float | None

    @staticmethod
    def from_mapping(raw: object) -> Posterior:
        """Read a posterior object.

        Categorical mass is either ``weights`` (emotion id to probability) or
        ``labels`` plus ``posterior`` in the same order the harness scorer
        uses. The two forms are not combined. Omitted VAD axes and energy
        stay unmeasured.
        """
        if not isinstance(raw, dict):
            raise FeedError("posterior must be a JSON object")
        if "t_ms" not in raw:
            raise FeedError("posterior needs t_ms")
        has_weights = "weights" in raw and raw["weights"] is not None
        has_vector = "labels" in raw or "posterior" in raw
        if has_weights and has_vector:
            raise FeedError("pass weights or labels+posterior, not both")
        weights: dict[str, float] | None
        if has_vector:
            weights = _weights_from_vector(raw.get("labels"), raw.get("posterior"))
        elif has_weights:
            weights = _weights_from_object(raw["weights"])
        else:
            weights = None
        kind = raw.get("kind", "frame")
        if kind not in KINDS:
            raise FeedError("kind must be frame or utterance")
        return Posterior(
            t_ms=_t_ms(raw["t_ms"]),
            kind=kind,
            weights=weights,
            valence=_optional(raw.get("valence"), "valence", -1.0, 1.0),
            arousal=_optional(raw.get("arousal"), "arousal", -1.0, 1.0),
            dominance=_optional(raw.get("dominance"), "dominance", -1.0, 1.0),
            energy=_optional(raw.get("energy"), "energy", 0.0, 1.0),
        )


def posterior_from_analysis(
    analysis: object,
    *,
    t_ms: int,
    kind: str = "frame",
    valence: float | None = None,
    arousal: float | None = None,
    dominance: float | None = None,
    energy: float | None = None,
) -> Posterior:
    """Adapt a harness ``Analysis`` (``labels`` + ``posterior``) without importing it.

    The four-class SUPERB head and the nine-class emotion2vec head are not
    expanded into the twelve. ``push`` rejects them.
    """
    labels = getattr(analysis, "labels", None)
    posterior = getattr(analysis, "posterior", None)
    return Posterior.from_mapping(
        {
            "t_ms": t_ms,
            "kind": kind,
            "labels": list(labels) if labels is not None else None,
            "posterior": list(posterior) if posterior is not None else None,
            "valence": valence,
            "arousal": arousal,
            "dominance": dominance,
            "energy": energy,
        }
    )


def _weights_from_object(raw: object) -> dict[str, float]:
    if not isinstance(raw, dict) or isinstance(raw, list):
        raise FeedError("weights must be an object of emotion id to probability")
    weights: dict[str, float] = {}
    for key, value in raw.items():
        if not isinstance(key, str):
            raise FeedError("weight names must be strings")
        name = key.strip().lower()
        if name in weights:
            raise FeedError(f"duplicate weight {name}")
        weights[name] = _weight(value, name)
    return weights


def _weights_from_vector(labels: object, posterior: object) -> dict[str, float]:
    if not isinstance(labels, (list, tuple)) or not isinstance(posterior, (list, tuple)):
        raise FeedError("labels and posterior must be arrays of the same length")
    if len(labels) != len(posterior):
        raise FeedError("labels and posterior differ in length")
    weights: dict[str, float] = {}
    for label, value in zip(labels, posterior):
        if not isinstance(label, str):
            raise FeedError("each label must be a string")
        name = label.strip().lower()
        if name in weights:
            raise FeedError(f"duplicate label {name}")
        weights[name] = _weight(value, name)
    return weights


def _require_posterior(voice_id: str, weights: dict[str, float]) -> None:
    expected = emotions_for(voice_id)
    unknown = sorted(set(weights) - set(GAS_LINES))
    if unknown:
        raise FeedError(
            f"unknown emotion {', '.join(unknown)}; "
            "coarse heads are not expanded into Emotion V2"
        )
    if voice_id != "samantha" and SEDUCTIVE in weights:
        raise FeedError("seductive is Samantha only")
    if set(weights) != set(expected):
        raise FeedError(
            "a posterior must name exactly "
            + ", ".join(expected)
            + "; coarse heads are not expanded into Emotion V2"
        )
    total = sum(weights[key] for key in expected)
    if abs(total - 1.0) > SUM_TOLERANCE:
        raise FeedError(f"posterior sums to {total:.6f}, want 1 ± {SUM_TOLERANCE}")


@dataclass(frozen=True)
class Weight:
    id: str
    weight: float | None
    gas_line: str


@dataclass(frozen=True)
class LiveFrame:
    voice_id: str
    t_ms: int
    kind: str
    present: bool
    distribution: bool
    tau_ms: float
    weights: tuple[Weight, ...]
    valence: float | None
    arousal: float | None
    dominance: float | None
    energy: float | None


def encode(frame: LiveFrame) -> str:
    """One JSON object, no trailing newline. Stdout adds the newline."""
    weights = {
        item.id: {"weight": _json_number(item.weight), "gas_line": item.gas_line}
        for item in frame.weights
    }
    payload = {
        "schema": SCHEMA,
        "voice_id": frame.voice_id,
        "t_ms": frame.t_ms,
        "kind": frame.kind,
        "present": frame.present,
        "distribution": frame.distribution,
        "tau_ms": _json_number(frame.tau_ms),
        "weights": weights,
        "valence": _json_number(frame.valence),
        "arousal": _json_number(frame.arousal),
        "dominance": _json_number(frame.dominance),
        "energy": _json_number(frame.energy),
    }
    return json.dumps(payload, ensure_ascii=False, separators=(",", ":"), allow_nan=False)


class _Channel:
    def __init__(self) -> None:
        self.value: float | None = None
        self.last_t_ms: int | None = None

    def update(self, t_ms: int, sample: float | None, tau_ms: float) -> None:
        if sample is None:
            return
        if self.value is None or self.last_t_ms is None:
            self.value = sample
        else:
            dt = t_ms - self.last_t_ms
            self.value = ema(self.value, sample, ema_alpha(dt, tau_ms))
        self.last_t_ms = t_ms


class _Smoother:
    def __init__(self, keys: tuple[str, ...], tau_ms: float) -> None:
        self.keys = keys
        self.tau_ms = tau_ms
        self.weights = {key: _Channel() for key in keys}
        self.valence = _Channel()
        self.arousal = _Channel()
        self.dominance = _Channel()
        self.energy = _Channel()
        self.last_emit_t_ms: int | None = None

    def _tick(self, t_ms: int) -> None:
        if self.last_emit_t_ms is not None and t_ms <= self.last_emit_t_ms:
            raise FeedError(f"t_ms {t_ms} is not after {self.last_emit_t_ms}")
        self.last_emit_t_ms = t_ms

    def push(self, posterior: Posterior) -> None:
        self._tick(posterior.t_ms)
        if posterior.weights is not None:
            for key in self.keys:
                self.weights[key].update(posterior.t_ms, posterior.weights[key], self.tau_ms)
        self.valence.update(posterior.t_ms, posterior.valence, self.tau_ms)
        self.arousal.update(posterior.t_ms, posterior.arousal, self.tau_ms)
        self.dominance.update(posterior.t_ms, posterior.dominance, self.tau_ms)
        self.energy.update(posterior.t_ms, posterior.energy, self.tau_ms)

    def hold(self, t_ms: int) -> None:
        self._tick(t_ms)

    def snapshot(self, voice_id: str, t_ms: int, kind: str) -> LiveFrame:
        items: list[Weight] = []
        total = 0.0
        complete = True
        any_weight = False
        for key in self.keys:
            value = self.weights[key].value
            items.append(Weight(key, value, GAS_LINES[key]))
            if value is None:
                complete = False
            else:
                any_weight = True
                total += value
        axes = (self.valence.value, self.arousal.value, self.dominance.value, self.energy.value)
        present = any_weight or any(value is not None for value in axes)
        distribution = complete and abs(total - 1.0) <= SUM_TOLERANCE
        return LiveFrame(
            voice_id=voice_id,
            t_ms=t_ms,
            kind=kind,
            present=present,
            distribution=distribution,
            tau_ms=self.tau_ms,
            weights=tuple(items),
            valence=self.valence.value,
            arousal=self.arousal.value,
            dominance=self.dominance.value,
            energy=self.energy.value,
        )


class NebulaFeed:
    """Per-voice smoother. Frame ticks and utterance ticks keep separate state."""

    def __init__(self, voice_id: str, tau_ms: float = DEFAULT_TAU_MS) -> None:
        self.voice_id = _voice(voice_id)
        self.tau_ms = _tau(tau_ms)
        keys = emotions_for(self.voice_id)
        self._by_kind = {kind: _Smoother(keys, self.tau_ms) for kind in KINDS}

    def push(self, posterior: Posterior) -> LiveFrame:
        """Smooth one posterior and return the outbound frame for that kind."""
        if posterior.kind not in self._by_kind:
            raise FeedError("kind must be frame or utterance")
        if posterior.weights is not None:
            _require_posterior(self.voice_id, posterior.weights)
        smoother = self._by_kind[posterior.kind]
        smoother.push(posterior)
        return smoother.snapshot(self.voice_id, posterior.t_ms, posterior.kind)

    def hold(self, t_ms: int, kind: str = "frame") -> LiveFrame:
        """Repeat the last smoothed cloud. Absence if this kind has no estimate yet."""
        if kind not in self._by_kind:
            raise FeedError("kind must be frame or utterance")
        stamp = _t_ms(t_ms)
        smoother = self._by_kind[kind]
        smoother.hold(stamp)
        return smoother.snapshot(self.voice_id, stamp, kind)
