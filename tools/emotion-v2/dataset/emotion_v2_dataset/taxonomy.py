# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""The twelve Emotion V2 labels, and the slot the taxonomy branch fills.

``cursor/emotion-v2-taxonomy`` is a separate branch. When it lands, expose an
importable module ``emotion_v2_taxonomy`` with::

    def load_taxonomy() -> list[str]:
        '''Return the canonical emotion names.'''

This pipeline calls that function when the module imports. The returned set
must equal :data:`CANONICAL_EMOTIONS`. A missing module, or a set that does
not match, keeps the local list and records which choice was used. This
branch does not edit the taxonomy branch.
"""

from __future__ import annotations

from collections.abc import Callable, Sequence
from typing import Any

from emotion_v2_dataset.errors import ProfileError

# Order is the dataset order. Names match the Emotion V2 brief.
CANONICAL_EMOTIONS: tuple[str, ...] = (
    "Neutral",
    "Calm",
    "Happy",
    "Amused",
    "Excited",
    "Tender",
    "Sad",
    "Disappointed",
    "Angry",
    "Fearful",
    "Surprised",
    "Disgusted",
)

# Cents relative to the voice pitch centre. Applied as 2^(cents/1200).
# Never expressed as a ratio against 440 Hz.
EMOTION_CENTS: dict[str, float] = {
    "Neutral": 0.0,
    "Calm": -30.0,
    "Happy": 50.0,
    "Amused": 35.0,
    "Excited": 80.0,
    "Tender": -15.0,
    "Sad": -60.0,
    "Disappointed": -40.0,
    "Angry": 20.0,
    "Fearful": 70.0,
    "Surprised": 90.0,
    "Disgusted": -10.0,
}

# Relative high-harmonic weight and a little noise. Synthetic timbre only.
EMOTION_TILT: dict[str, float] = {
    "Neutral": 0.35,
    "Calm": 0.22,
    "Happy": 0.55,
    "Amused": 0.48,
    "Excited": 0.70,
    "Tender": 0.28,
    "Sad": 0.18,
    "Disappointed": 0.24,
    "Angry": 0.62,
    "Fearful": 0.58,
    "Surprised": 0.66,
    "Disgusted": 0.40,
}

TAXONOMY_MODULE = "emotion_v2_taxonomy"


def canonical_emotions() -> tuple[str, ...]:
    return CANONICAL_EMOTIONS


def require_emotion(name: str) -> str:
    if name not in CANONICAL_EMOTIONS:
        known = ", ".join(CANONICAL_EMOTIONS)
        raise ProfileError(f"unknown emotion {name!r}; expected one of: {known}")
    return name


def load_emotions(
    importer: Callable[[str], Any] | None = None,
) -> tuple[tuple[str, ...], str]:
    """Return ``(names, source)``.

    ``source`` is ``emotion_v2_taxonomy`` when that module imports and its
    ``load_taxonomy()`` returns exactly the canonical set. Otherwise the
    source is ``local`` or ``local-mismatch``.
    """
    if importer is None:
        importer = _import_module
    try:
        module = importer(TAXONOMY_MODULE)
    except ImportError:
        return CANONICAL_EMOTIONS, "local"
    loader = getattr(module, "load_taxonomy", None)
    if not callable(loader):
        return CANONICAL_EMOTIONS, "local-mismatch"
    try:
        raw = loader()
    except Exception:
        return CANONICAL_EMOTIONS, "local-mismatch"
    names = _as_names(raw)
    if names is None or set(names) != set(CANONICAL_EMOTIONS):
        return CANONICAL_EMOTIONS, "local-mismatch"
    # Keep this pipeline's order so manifests stay stable.
    return CANONICAL_EMOTIONS, TAXONOMY_MODULE


def _as_names(raw: object) -> list[str] | None:
    if isinstance(raw, (str, bytes)) or not isinstance(raw, Sequence):
        return None
    names: list[str] = []
    for item in raw:
        if not isinstance(item, str):
            return None
        names.append(item)
    return names


def _import_module(name: str) -> Any:
    import importlib

    return importlib.import_module(name)
