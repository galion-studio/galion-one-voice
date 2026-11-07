# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Generic JSONL manifest for one dataset run."""

from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path
from typing import Any

SCHEMA = "emotion-v2.manifest.v1"

# The brief's fields, in a stable order. Extra keys follow them.
REQUIRED_KEYS = (
    "audio_path",
    "text",
    "emotion",
    "intensity",
    "environment",
    "snr_db",
    "voice_id",
    "scores",
    "sha256",
)


@dataclass(frozen=True)
class ManifestRow:
    audio_path: str
    text: str
    emotion: str
    intensity: float
    environment: str
    snr_db: float | None
    voice_id: str
    scores: dict[str, float | None]
    sha256: str
    sample_rate_hz: int
    pitch_standard_hz: float
    pitch_centre_hz: float
    render_backend: str
    synthetic_signal: bool
    scenario_id: str
    ir_source: str
    ir_licence: str
    noise_source: str
    noise_licence: str
    telephony_hz: int | None = None
    duration_s: float = 0.0

    def as_dict(self) -> dict[str, Any]:
        row: dict[str, Any] = {
            "audio_path": self.audio_path,
            "text": self.text,
            "emotion": self.emotion,
            "intensity": self.intensity,
            "environment": self.environment,
            "snr_db": self.snr_db,
            "voice_id": self.voice_id,
            "scores": {
                "wer": self.scores.get("wer"),
                "ecapa": self.scores.get("ecapa"),
                "emotion": self.scores.get("emotion"),
            },
            "sha256": self.sha256,
            "schema": SCHEMA,
            "sample_rate_hz": self.sample_rate_hz,
            "duration_s": self.duration_s,
            "pitch_standard_hz": self.pitch_standard_hz,
            "pitch_centre_hz": self.pitch_centre_hz,
            "render_backend": self.render_backend,
            "synthetic_signal": self.synthetic_signal,
            "scenario_id": self.scenario_id,
            "ir_source": self.ir_source,
            "ir_licence": self.ir_licence,
            "noise_source": self.noise_source,
            "noise_licence": self.noise_licence,
            "telephony_hz": self.telephony_hz,
        }
        return row


def write_jsonl(path: Path, rows: list[dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8") as handle:
        for row in rows:
            handle.write(json.dumps(row, ensure_ascii=False, allow_nan=False))
            handle.write("\n")


def read_jsonl(path: Path) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    text = path.read_text(encoding="utf-8")
    for line in text.splitlines():
        if not line.strip():
            continue
        item = json.loads(line)
        if not isinstance(item, dict):
            raise ValueError(f"{path} contains a non-object JSONL row")
        rows.append(item)
    return rows
