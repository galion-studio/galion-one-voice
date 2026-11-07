# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Minimal interfaces for the sibling Emotion V2 branches.

Those branches are not merged here and are not edited by this pipeline.
Until they import, the local defaults stand.

Taxonomy (``cursor/emotion-v2-taxonomy``)::

    # module emotion_v2_taxonomy
    def load_taxonomy() -> list[str]: ...

Capture (``cursor/emotion-v2-capture``)::

    # module emotion_v2_capture
    def resolve_references(voice_id: str) -> dict[str, str]:
        '''Emotion name -> reference directory. Directories are not opened.'''

CI / scorers (``cursor/emotion-v2-ci``)::

    # module emotion_v2_ci
    def build_scorers(config: dict) -> list:
        '''Objects with .name in {'wer','ecapa','emotion'} and .score(clip) -> float.'''
"""

from __future__ import annotations

import importlib
from collections.abc import Callable
from typing import Any

from emotion_v2_dataset.taxonomy import CANONICAL_EMOTIONS, load_emotions

CAPTURE_MODULE = "emotion_v2_capture"
CI_MODULE = "emotion_v2_ci"


def taxonomy_source(importer: Callable[[str], Any] | None = None) -> tuple[tuple[str, ...], str]:
    return load_emotions(importer)


def resolve_references(
    voice_id: str,
    references: dict[str, str],
    importer: Callable[[str], Any] | None = None,
) -> tuple[dict[str, str], str]:
    """Fill empty emotion directories from the capture plugin.

    Existing profile entries win. Returned paths are strings. Nothing in
    this function opens a file or a directory.
    """
    if importer is None:
        importer = importlib.import_module
    merged = dict(references)
    try:
        module = importer(CAPTURE_MODULE)
    except ImportError:
        return merged, "local"
    resolver = getattr(module, "resolve_references", None)
    if not callable(resolver):
        return merged, "local-mismatch"
    try:
        extra = resolver(voice_id)
    except Exception:
        return merged, "local-mismatch"
    if not isinstance(extra, dict):
        return merged, "local-mismatch"
    for key, value in extra.items():
        if not isinstance(key, str) or not isinstance(value, str) or not value.strip():
            continue
        if key not in CANONICAL_EMOTIONS:
            continue
        merged.setdefault(key, value.strip())
    return merged, CAPTURE_MODULE


def load_scorer_plugins(
    importer: Callable[[str], Any] | None = None,
    config: dict[str, Any] | None = None,
) -> tuple[list[Any], str]:
    if importer is None:
        importer = importlib.import_module
    try:
        module = importer(CI_MODULE)
    except ImportError:
        return [], "local"
    builder = getattr(module, "build_scorers", None)
    if not callable(builder):
        return [], "local-mismatch"
    try:
        built = builder({} if config is None else config)
    except Exception:
        return [], "local-mismatch"
    if not isinstance(built, list):
        return [], "local-mismatch"
    return list(built), CI_MODULE
