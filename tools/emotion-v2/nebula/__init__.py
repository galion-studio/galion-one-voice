# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Nebula live emotion feed.

Smoothed Emotion V2 posteriors as newline-delimited JSON, for the visualiser
at apps/nebula-voice-v2. The schema is docs/emotion-v2/NEBULA-LIVE-FEED.md.
"""

from .feed import (
    DEFAULT_TAU_MS,
    HEARTBEAT_MS,
    SCHEMA,
    FeedError,
    LiveFrame,
    NebulaFeed,
    Posterior,
    ema_alpha,
    encode,
    posterior_from_analysis,
)
from .gas import EMOTIONS, GAS_LINES

__all__ = [
    "DEFAULT_TAU_MS",
    "EMOTIONS",
    "GAS_LINES",
    "HEARTBEAT_MS",
    "SCHEMA",
    "FeedError",
    "LiveFrame",
    "NebulaFeed",
    "Posterior",
    "ema_alpha",
    "encode",
    "posterior_from_analysis",
]
