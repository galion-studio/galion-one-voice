# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Samantha's twelve Emotion V2 categories.

Names are the harness contract. Paths accept the lowercase id or the
display name (`neutral` or `Neutral`).
"""

from __future__ import annotations

EMOTIONS: tuple[str, ...] = (
    "neutral",
    "calm",
    "happy",
    "amused",
    "excited",
    "tender",
    "sad",
    "disappointed",
    "angry",
    "fearful",
    "surprised",
    "disgusted",
)

# Deterministic stub bins. Amused sits on the 432 Hz tuning standard so a
# pure 432 Hz tone is a categorical fixture for that emotion. These are
# analysis frequencies for the stub, not a musical scale of the voice.
STUB_FREQ_HZ: dict[str, float] = {
    "neutral": 210.0,
    "calm": 270.0,
    "happy": 330.0,
    "amused": 432.0,
    "excited": 510.0,
    "tender": 630.0,
    "sad": 750.0,
    "disappointed": 870.0,
    "angry": 1050.0,
    "fearful": 1230.0,
    "surprised": 1470.0,
    "disgusted": 1710.0,
}


def display_name(emotion: str) -> str:
    key = emotion.strip().lower()
    if key not in STUB_FREQ_HZ:
        raise KeyError(emotion)
    return key[:1].upper() + key[1:]


def canonical_emotion(name: str) -> str:
    key = name.strip().lower()
    if key not in STUB_FREQ_HZ:
        known = ", ".join(display_name(e) for e in EMOTIONS)
        raise KeyError(f"unknown emotion {name!r}; expected one of {known}")
    return key
