# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Voice profile: id, per-emotion reference directories, 432 Hz pitch centre.

``pitch_centre_hz`` is the voice's fundamental on the 432 Hz pitch standard
(A4 = 432). A speaking fundamental may land near 440 Hz; that is not a
retune. ``pitch_standard_hz`` must be 432. Any requested retune whose target
is 440 Hz is refused.

Reference directories are paths only. This module does not open them, so a
real person's recordings are never read on the stub path.

The capture branch (``cursor/emotion-v2-capture``) can fill missing
directories later. See :func:`emotion_v2_dataset.plugins.resolve_references`.
"""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from emotion_v2_dataset.errors import PitchPolicyError, ProfileError
from emotion_v2_dataset.paths import check_id
from emotion_v2_dataset.taxonomy import CANONICAL_EMOTIONS, require_emotion

PITCH_STANDARD_HZ = 432.0
# Concert pitch. The pipeline never retunes a voice onto this standard.
FORBIDDEN_RETUNE_HZ = 440.0


@dataclass(frozen=True)
class VoiceProfile:
    voice_id: str
    kind: str
    pitch_standard_hz: float
    pitch_centre_hz: float
    references: dict[str, str] = field(default_factory=dict)
    retune_hz: float | None = None

    @property
    def synthetic(self) -> bool:
        return self.kind == "synthetic"


def load_profile(path: Path) -> VoiceProfile:
    file = Path(path)
    if not file.is_file():
        raise ProfileError(f"voice profile is missing: {file}")
    try:
        raw = json.loads(file.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise ProfileError(f"voice profile is not JSON: {file.name}") from exc
    if not isinstance(raw, dict):
        raise ProfileError("voice profile must be a JSON object")
    return parse_profile(raw)


def parse_profile(raw: dict[str, Any]) -> VoiceProfile:
    voice_id = raw.get("voice_id")
    if not isinstance(voice_id, str):
        raise ProfileError("voice_id is missing")
    voice_id = check_id("voice_id", voice_id.strip())
    kind = raw.get("kind", "real")
    if kind not in ("synthetic", "real"):
        raise ProfileError("kind must be 'synthetic' or 'real'")
    pitch_standard = _number(raw, "pitch_standard_hz")
    pitch_centre = _number(raw, "pitch_centre_hz")
    forbid_pitch_standard(pitch_standard)
    _refuse_retune_request(raw, "retune_hz")
    _refuse_retune_request(raw, "retune_to_hz")
    _refuse_retune_request(raw, "target_a4_hz")
    if "a4_hz" in raw and raw["a4_hz"] is not None:
        # An explicit A4 must be the 432 Hz standard. 440 is the concert retune.
        forbid_pitch_standard(_number(raw, "a4_hz"))
    if not (40.0 <= pitch_centre <= 800.0):
        raise ProfileError("pitch_centre_hz must be between 40 and 800")
    references = _references(raw.get("references", {}), kind=kind)
    return VoiceProfile(
        voice_id=voice_id,
        kind=kind,
        pitch_standard_hz=float(pitch_standard),
        pitch_centre_hz=float(pitch_centre),
        references=references,
        retune_hz=None,
    )


def _refuse_retune_request(raw: dict[str, Any], key: str) -> None:
    if key not in raw or raw[key] is None:
        return
    target = _number(raw, key)
    forbid_retune_target(target)
    raise PitchPolicyError(
        "this pipeline does not retune a voice; set pitch_centre_hz on the 432 Hz standard"
    )


def forbid_pitch_standard(hz: float) -> None:
    if abs(float(hz) - PITCH_STANDARD_HZ) > 1e-6:
        raise PitchPolicyError(
            f"pitch_standard_hz must be {PITCH_STANDARD_HZ:g} "
            f"(refusing {hz:g}, including a 440 Hz concert standard)"
        )


def forbid_retune_target(hz: float) -> None:
    """Refuse a pitch shift onto concert A4.

    Emotion offsets are cents relative to the 432 Hz centre. They are not a
    retune, and this function is the gate a shifter must call first.
    """
    if abs(float(hz) - FORBIDDEN_RETUNE_HZ) <= 0.5:
        raise PitchPolicyError(
            "refusing to retune to 440 Hz; the voice stays on the 432 Hz standard"
        )


def shift_ratio(from_hz: float, to_hz: float) -> float:
    """Ratio for a pitch move that is not a concert-pitch retune.

    Calling this with ``to_hz`` near 440 raises :class:`PitchPolicyError`
    and returns nothing. The stub renderer does not call it: emotion cents
    are applied to ``pitch_centre_hz`` directly.
    """
    forbid_retune_target(to_hz)
    if from_hz <= 0 or to_hz <= 0:
        raise PitchPolicyError("pitch shift frequencies must be positive")
    return float(to_hz) / float(from_hz)


def require_real_references(references: dict[str, str]) -> None:
    missing = [name for name in CANONICAL_EMOTIONS if not references.get(name, "").strip()]
    if missing:
        raise ProfileError(
            "real voice is missing a reference directory for: " + ", ".join(missing)
        )


def _references(raw: object, *, kind: str) -> dict[str, str]:
    del kind  # Completeness is checked after the capture plugin runs.
    if raw is None:
        raw = {}
    if not isinstance(raw, dict):
        raise ProfileError("references must be an object of emotion -> directory")
    cleaned: dict[str, str] = {}
    for key, value in raw.items():
        if not isinstance(key, str) or not isinstance(value, str):
            raise ProfileError("each reference must be emotion -> directory string")
        emotion = require_emotion(key)
        directory = value.strip()
        if not directory:
            raise ProfileError(f"reference directory for {emotion} is empty")
        # Paths are stored, not opened.
        cleaned[emotion] = directory
    return cleaned


def _number(raw: dict[str, Any], key: str) -> float:
    value = raw.get(key)
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ProfileError(f"{key} must be a number")
    return float(value)
