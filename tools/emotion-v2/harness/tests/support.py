# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Path setup for the Emotion V2 harness tests. Synthetic signals only."""

from __future__ import annotations

import sys
from pathlib import Path

EMOTION_V2 = Path(__file__).resolve().parents[2]
REPO = EMOTION_V2.parents[1]
if str(EMOTION_V2) not in sys.path:
    sys.path.insert(0, str(EMOTION_V2))
