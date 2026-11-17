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
    module_names: tuple[str, ...] | None = None,
) -> tuple[list[Any], str]:
    """Load scorers from ``emotion_v2_ci`` or from named modules.

    ``module_names`` lets a run plug in another scorer module, for example a
    12-class emotion scorer, without editing this package. Each module must
    expose ``build_scorers(config) -> list``. Scorers from later modules
    replace earlier ones with the same ``name``. A named module that does not
    import, or does not match the interface, raises ``QualityError``: a
    scorer the caller asked for is never silently dropped.
    """
    if importer is None:
        importer = importlib.import_module
    if module_names:
        return _load_named_scorers(importer, config, module_names)
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


def _load_named_scorers(
    importer: Callable[[str], Any],
    config: dict[str, Any] | None,
    module_names: tuple[str, ...],
) -> tuple[list[Any], str]:
    from emotion_v2_dataset.errors import QualityError

    by_name: dict[str, Any] = {}
    extra: list[Any] = []
    for module_name in module_names:
        try:
            module = importer(module_name)
        except ImportError as exc:
            raise QualityError(f"scorer module {module_name} does not import") from exc
        builder = getattr(module, "build_scorers", None)
        if not callable(builder):
            raise QualityError(f"scorer module {module_name} has no build_scorers()")
        built = builder({} if config is None else config)
        if not isinstance(built, list):
            raise QualityError(f"scorer module {module_name} build_scorers() must return a list")
        for scorer in built:
            name = getattr(scorer, "name", None)
            if isinstance(name, str):
                by_name[name] = scorer
            else:
                extra.append(scorer)
    return list(by_name.values()) + extra, "+".join(module_names)
