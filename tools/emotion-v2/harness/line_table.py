# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Element ids for an emitted emotion distribution.

The ids are labels. They are not fed back into pitch, energy, or logits.
"""

from __future__ import annotations

import importlib.util
from pathlib import Path

_SUPERB = {"neu": "neutral", "hap": "happy", "ang": "angry", "sad": "sad"}
_MODULE = None


def _elements():
    global _MODULE
    if _MODULE is not None:
        return _MODULE
    path = Path(__file__).resolve().parents[3] / "emotion-v2" / "elements.py"
    spec = importlib.util.spec_from_file_location("galion_emotion_elements", path)
    if spec is None or spec.loader is None:
        raise ImportError(path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    _MODULE = module
    return module


def element_ids_for(labels: tuple[str, ...]) -> tuple[str, ...]:
    table = _elements()
    out: list[str] = []
    for label in labels:
        key = _SUPERB.get(label.strip().casefold(), label.strip().casefold())
        out.append(table.element_id(key))
    return tuple(out)
