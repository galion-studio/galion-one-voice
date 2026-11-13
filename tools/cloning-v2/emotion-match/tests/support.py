# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Path setup for emotion_match tests. Synthetic signals only."""

from __future__ import annotations

import json
import sys
from pathlib import Path

TOOL = Path(__file__).resolve().parents[1]
REPO = TOOL.parents[2]
if str(TOOL) not in sys.path:
    sys.path.insert(0, str(TOOL))

from sector_refs import SHARED_SECTORS, file_sha256  # noqa: E402
from stub import write_sector_tone  # noqa: E402


def sha256_file(path: Path) -> str:
    return file_sha256(path)


def write_manifest(
    path: Path,
    references: dict[str, Path],
    lines: list[dict] | None = None,
    *,
    persona: str = "samantha",
    extra: dict | None = None,
) -> Path:
    body = {
        "schema": "galion-emotion-v2-sector-refs/1",
        "persona": persona,
        "gate": 0.83,
        "formula": "0.70*category_blend+0.30*emb_sim",
        "weights": {"category_blend": 0.70, "emb_sim": 0.30},
        "references": {
            emotion: {"path": str(ref_path), "sha256": sha256_file(ref_path)}
            for emotion, ref_path in references.items()
        },
    }
    if lines is not None:
        body["lines"] = lines
    if extra:
        body.update(extra)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(body, indent=2) + "\n", encoding="utf-8")
    return path


def write_matching_trees(root: Path, emotions=SHARED_SECTORS, include_seductive: bool = False):
    """Write matching sine refs and renders. Audio stays in the temp dir."""
    names = list(emotions)
    if include_seductive and "seductive" not in names:
        names.append("seductive")
    refs = {}
    renders = {}
    for emotion in names:
        ref = write_sector_tone(root / "refs" / f"{emotion}.wav", emotion)
        render = write_sector_tone(root / "renders" / emotion / "s01.wav", emotion)
        refs[emotion] = ref
        renders[emotion] = render
    return refs, renders
