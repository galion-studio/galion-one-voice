# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Filesystem locations for the dataset tool."""

from __future__ import annotations

import re
from pathlib import Path

from emotion_v2_dataset.errors import PipelineError

PACKAGE_ROOT = Path(__file__).resolve().parents[1]
DATA_DIR = PACKAGE_ROOT / "data"

_ID_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$")


def find_repo_root(start: Path | None = None) -> Path | None:
    """Walk upwards for the galion-one-voice checkout. Does not read LOCKS.toml."""
    origin = (start or PACKAGE_ROOT).resolve()
    for candidate in (origin, *origin.parents):
        if (candidate / ".git").exists() and (candidate / "Cargo.toml").exists():
            return candidate
    return None


def default_output_root(repo_root: Path | None = None) -> Path:
    root = repo_root if repo_root is not None else find_repo_root()
    if root is None:
        return Path("datasets") / "emotion-v2"
    return root / "datasets" / "emotion-v2"


def check_id(label: str, value: str) -> str:
    if not _ID_RE.fullmatch(value):
        raise PipelineError(
            f"{label} must be 1-64 characters of letters, digits, '.', '_' or '-'"
        )
    return value


def assert_safe_output(run_dir: Path, repo_root: Path | None) -> None:
    """Allow tmp dirs and ``datasets/emotion-v2/`` only.

    Audio written anywhere else inside the checkout could be committed by
    accident. Reference clips, voices, and docs are never a legal output.
    """
    if repo_root is None:
        return
    resolved = run_dir.resolve()
    root = repo_root.resolve()
    try:
        relative = resolved.relative_to(root)
    except ValueError:
        return
    parts = relative.parts
    if len(parts) >= 2 and parts[0] == "datasets" and parts[1] == "emotion-v2":
        return
    raise PipelineError(
        "refusing to write dataset audio inside the repository outside "
        "datasets/emotion-v2/ (that directory is git-ignored)"
    )


def slug(text: str) -> str:
    chars: list[str] = []
    for char in text.casefold():
        if char.isascii() and char.isalnum():
            chars.append(char)
        else:
            chars.append("-")
    slug_text = "".join(chars)
    while "--" in slug_text:
        slug_text = slug_text.replace("--", "-")
    return slug_text.strip("-") or "item"
