# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Emotion V2 harness for Samantha's twelve emotions."""

from __future__ import annotations

from .backends import StubBackend, resolve_backend
from .config import load_thresholds
from .emotions import EMOTIONS
from .score import score_all, score_emotion

__all__ = [
    "EMOTIONS",
    "StubBackend",
    "load_thresholds",
    "resolve_backend",
    "score_all",
    "score_emotion",
]
