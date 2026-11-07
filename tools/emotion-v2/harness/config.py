# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Load Emotion V2 thresholds and the fixed scenario texts."""

from __future__ import annotations

import tomllib
from dataclasses import dataclass
from pathlib import Path

from .emotions import EMOTIONS, canonical_emotion


class ConfigError(ValueError):
    """The thresholds file or a scenario directory is not usable."""


@dataclass(frozen=True)
class Scenario:
    stem: str
    text: str
    path: Path


@dataclass(frozen=True)
class Thresholds:
    schema: str
    persona: str
    tuning_hz: int
    wer_max: float
    ecapa_min: float
    emotion_target: float
    emotion_targets: dict[str, float]
    embedding_min: float | None
    backend_id: str
    source: Path

    def target_for(self, emotion: str) -> float:
        return self.emotion_targets[canonical_emotion(emotion)]


def emotion_v2_root() -> Path:
    return Path(__file__).resolve().parents[1]


def repo_root() -> Path:
    for parent in Path(__file__).resolve().parents:
        if (parent / ".git").exists() or (parent / "LOCKS.toml").is_file():
            return parent
    raise ConfigError("could not find the repository root from the harness")


def default_thresholds_path() -> Path:
    return emotion_v2_root() / "thresholds.toml"


def default_scenarios_path() -> Path:
    return emotion_v2_root() / "scenarios"


def load_thresholds(path: Path | None = None) -> Thresholds:
    source = path or default_thresholds_path()
    if not source.is_file():
        raise ConfigError(f"missing thresholds file: {source}")
    try:
        data = tomllib.loads(source.read_text(encoding="utf-8"))
    except tomllib.TOMLDecodeError as exc:
        raise ConfigError(f"thresholds file {source} is not valid TOML: {exc}") from exc
    try:
        guards = data["guards"]
        emotion = data["emotion"]
        targets = dict(emotion["targets"])
        embedding = data.get("embedding") or {}
        backend = data.get("backend") or {}
        parsed = Thresholds(
            schema=str(data["schema"]),
            persona=str(data["persona"]),
            tuning_hz=int(data["tuning_hz"]),
            wer_max=float(guards["wer_max"]),
            ecapa_min=float(guards["ecapa_min"]),
            emotion_target=float(emotion["target"]),
            emotion_targets={canonical_emotion(key): float(value) for key, value in targets.items()},
            embedding_min=_optional_float(embedding.get("min")),
            backend_id=str(backend.get("id") or "superb/wav2vec2-base-superb-er"),
            source=source,
        )
    except (KeyError, TypeError, ValueError) as exc:
        raise ConfigError(f"thresholds file {source} is incomplete: {exc}") from exc
    missing = [name for name in EMOTIONS if name not in parsed.emotion_targets]
    extra = [name for name in parsed.emotion_targets if name not in EMOTIONS]
    if missing or extra:
        raise ConfigError(
            f"emotion targets must list exactly the twelve Samantha emotions; "
            f"missing={missing or '-'} extra={extra or '-'}"
        )
    return parsed


def _optional_float(value: object) -> float | None:
    if value is None:
        return None
    return float(value)


def emotion_subdir(root: Path, emotion: str) -> Path:
    """Prefer an existing lowercase or display-name directory."""
    key = canonical_emotion(emotion)
    display = key[:1].upper() + key[1:]
    for candidate in (root / key, root / display):
        if candidate.is_dir():
            return candidate
    return root / key


def load_scenarios(scenario_root: Path, emotion: str) -> list[Scenario]:
    directory = emotion_subdir(scenario_root, emotion)
    if not directory.is_dir():
        raise ConfigError(f"missing scenario directory for {emotion}: {directory}")
    files = sorted(
        (path for path in directory.iterdir() if path.is_file() and path.suffix == ".txt"),
        key=lambda path: path.name,
    )
    if not files:
        raise ConfigError(f"no scenario texts in {directory}")
    scenarios: list[Scenario] = []
    for path in files:
        text = path.read_text(encoding="utf-8").strip()
        if not text:
            raise ConfigError(f"empty scenario text: {path}")
        scenarios.append(Scenario(stem=path.stem, text=text, path=path))
    return scenarios
