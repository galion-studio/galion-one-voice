# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Scenario text bank. One list of lines per emotion. Text only, no audio."""

from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from emotion_v2_dataset.errors import ScenarioError
from emotion_v2_dataset.taxonomy import CANONICAL_EMOTIONS, require_emotion


@dataclass(frozen=True)
class Scenario:
    emotion: str
    scenario_id: str
    text: str
    intensity: float


def load_scenarios(path: Path) -> dict[str, tuple[Scenario, ...]]:
    file = Path(path)
    if not file.is_file():
        raise ScenarioError(f"scenario bank is missing: {file}")
    try:
        raw = json.loads(file.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise ScenarioError(f"scenario bank is not JSON: {file.name}") from exc
    if not isinstance(raw, dict):
        raise ScenarioError("scenario bank must be a JSON object keyed by emotion")
    return parse_scenarios(raw)


def parse_scenarios(raw: dict[str, Any]) -> dict[str, tuple[Scenario, ...]]:
    bank: dict[str, tuple[Scenario, ...]] = {}
    for key, lines in raw.items():
        if not isinstance(key, str):
            raise ScenarioError("scenario emotion keys must be strings")
        emotion = require_emotion(key)
        if not isinstance(lines, list) or not lines:
            raise ScenarioError(f"{emotion} needs a non-empty list of lines")
        parsed: list[Scenario] = []
        seen: set[str] = set()
        for index, line in enumerate(lines):
            scenario = _line(emotion, index, line)
            if scenario.scenario_id in seen:
                raise ScenarioError(f"duplicate scenario id {scenario.scenario_id}")
            seen.add(scenario.scenario_id)
            parsed.append(scenario)
        bank[emotion] = tuple(parsed)
    return bank


def require_coverage(
    bank: dict[str, tuple[Scenario, ...]],
    emotions: tuple[str, ...],
) -> None:
    missing = [name for name in emotions if name not in bank or not bank[name]]
    if missing:
        raise ScenarioError("scenario bank has no lines for: " + ", ".join(missing))


def select_scenarios(
    bank: dict[str, tuple[Scenario, ...]],
    emotions: tuple[str, ...],
    *,
    max_per_emotion: int | None = None,
    scenario_ids: tuple[str, ...] | None = None,
) -> list[Scenario]:
    require_coverage(bank, emotions)
    chosen: list[Scenario] = []
    wanted = set(scenario_ids) if scenario_ids else None
    for emotion in emotions:
        rows = list(bank[emotion])
        if wanted is not None:
            rows = [row for row in rows if row.scenario_id in wanted]
            if not rows:
                raise ScenarioError(f"no selected scenario id for {emotion}")
        if max_per_emotion is not None:
            if max_per_emotion < 1:
                raise ScenarioError("max_per_emotion must be at least 1")
            rows = rows[:max_per_emotion]
        chosen.extend(rows)
    return chosen


def _line(emotion: str, index: int, line: object) -> Scenario:
    if not isinstance(line, dict):
        raise ScenarioError(f"{emotion} line {index} must be an object")
    text = line.get("text")
    scenario_id = line.get("id")
    if not isinstance(text, str) or not text.strip():
        raise ScenarioError(f"{emotion} line {index} has empty text")
    if not isinstance(scenario_id, str) or not scenario_id.strip():
        raise ScenarioError(f"{emotion} line {index} needs an id")
    intensity = line.get("intensity", 0.5)
    if isinstance(intensity, bool) or not isinstance(intensity, (int, float)):
        raise ScenarioError(f"{emotion} line {index} intensity must be a number")
    if not 0.0 <= float(intensity) <= 1.0:
        raise ScenarioError(f"{emotion} line {index} intensity must be between 0 and 1")
    return Scenario(
        emotion=emotion,
        scenario_id=scenario_id.strip(),
        text=text.strip(),
        intensity=float(intensity),
    )


def full_bank_emotions() -> tuple[str, ...]:
    return CANONICAL_EMOTIONS
