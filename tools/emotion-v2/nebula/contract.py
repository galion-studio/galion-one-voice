# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""UI contract for the Nebula live feed.

``galion-nebula-live-feed/1`` (docs/emotion-v2/NEBULA-LIVE-FEED.md) is the
side stream a caller already has. This module does not score audio and does
not talk to the live voice server. It checks that wire, then projects one
``galion-voice-v2-emotion-feed/1`` object for the picture.

``estimate`` is the state the picture must read:

- ``none``: no emotion estimate. Weights are null. This is not Neutral = 1.
- ``distribution``: every weight for this voice is numeric and sums to 1.
- ``partial``: at least one number, and not a full distribution.

The picture binds ``element_id`` (the Demo 13 id). ``gas_line`` stays the
feed id. Those two strings differ for several emotions.
"""

from __future__ import annotations

import json
import math
from typing import Mapping

from .feed import SCHEMA, SUM_TOLERANCE, FeedError
from .gas import GAS_LINES, SEDUCTIVE, VOICES, emotions_for

UI_SCHEMA = "galion-voice-v2-emotion-feed/1"
ESTIMATE_NONE = "none"
ESTIMATE_DISTRIBUTION = "distribution"
ESTIMATE_PARTIAL = "partial"
ESTIMATES = (ESTIMATE_NONE, ESTIMATE_DISTRIBUTION, ESTIMATE_PARTIAL)
ORIGINS = ("server", "client")
REASONS = (None, "rejected", "disconnected")

# Demo 13 element ids from crates/nebula-elements. Not the feed gas-line ids.
ELEMENT_IDS: dict[str, str] = {
    "neutral": "HeII-541.2",
    "calm": "H-beta",
    "happy": "NII-575.5",
    "amused": "HeI-587.6",
    "excited": "NeI-594.5",
    "tender": "HeI-667.8",
    "sad": "HeII-468.6",
    "disappointed": "ClIII-551.7",
    "angry": "SIII-631.2",
    "fearful": "H-delta",
    "surprised": "OIII-495.9",
    "disgusted": "ClIII-553.8",
    "seductive": "H-alpha",
}

_LIVE_KEYS = {
    "schema",
    "voice_id",
    "t_ms",
    "kind",
    "present",
    "distribution",
    "tau_ms",
    "weights",
    "valence",
    "arousal",
    "dominance",
    "energy",
}
_UI_KEYS = _LIVE_KEYS | {
    "source_schema",
    "estimate",
    "seq",
    "origin",
    "reason",
    "detail",
}
_SLOT_KEYS = {"weight", "gas_line"}
_UI_SLOT_KEYS = {"weight", "gas_line", "element_id"}


def _reject(message: str) -> None:
    raise FeedError(message)


def _object(raw: object, name: str) -> dict:
    if not isinstance(raw, dict):
        _reject(f"{name} must be a JSON object")
    return raw


def _voice(value: object, *, allow_null: bool) -> str | None:
    if value is None and allow_null:
        return None
    if not isinstance(value, str):
        _reject("voice_id must be a string")
    key = value.strip().lower()
    if key not in VOICES:
        known = ", ".join(VOICES)
        _reject(f"unknown voice {value!r}; expected one of {known}")
    return key


def _t_ms(value: object) -> int:
    if isinstance(value, bool):
        _reject("t_ms must be an integer number of milliseconds")
    if isinstance(value, int):
        stamp = value
    elif isinstance(value, float) and math.isfinite(value) and value.is_integer():
        stamp = int(value)
    else:
        _reject("t_ms must be an integer number of milliseconds")
    if stamp < 0:
        _reject("t_ms must be >= 0")
    return stamp


def _finite(value: object, name: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        _reject(f"{name} must be a number")
    number = float(value)
    if not math.isfinite(number):
        _reject(f"{name} must be finite")
    return number


def _optional(value: object, name: str, low: float, high: float) -> float | None:
    if value is None:
        return None
    number = _finite(value, name)
    if number < low or number > high:
        _reject(f"{name} {number} is outside {low}..{high}")
    return number


def _tau(value: object) -> float:
    number = _finite(value, "tau_ms")
    if number <= 0.0:
        _reject("tau_ms must be a positive number of milliseconds")
    return number


def _exact_keys(raw: Mapping[str, object], allowed: set[str], name: str) -> None:
    extra = sorted(set(raw) - allowed)
    if extra:
        _reject(f"{name} has unknown fields: {', '.join(extra)}")


def _emotions(voice_id: str | None) -> tuple[str, ...]:
    if voice_id is None:
        return tuple(key for key in GAS_LINES if key != SEDUCTIVE)
    return emotions_for(voice_id)


def classify_estimate(
    *,
    present: bool,
    distribution: bool,
    weights: dict[str, float | None],
    axes: tuple[float | None, float | None, float | None, float | None],
) -> str:
    """Name the estimate. Raises when the flags disagree with the numbers."""
    numeric_weights = [value is not None for value in weights.values()]
    any_weight = any(numeric_weights)
    all_weights = all(numeric_weights) and bool(weights)
    any_axis = any(value is not None for value in axes)
    total = sum(value or 0.0 for value in weights.values())
    simplex = all_weights and abs(total - 1.0) <= SUM_TOLERANCE
    if not present:
        if distribution or any_weight or any_axis:
            _reject("present false cannot carry a number; that is not an absence")
        return ESTIMATE_NONE
    if not any_weight and not any_axis:
        _reject("present true needs a weight or an axis; absence is present false")
    if distribution:
        if not simplex:
            _reject("distribution true needs every weight and a sum of 1")
        return ESTIMATE_DISTRIBUTION
    if simplex:
        _reject("a complete simplex must set distribution true")
    return ESTIMATE_PARTIAL


def _read_slots(raw: object, voice_id: str, *, ui: bool) -> dict[str, dict[str, object]]:
    weights = _object(raw, "weights")
    expected = emotions_for(voice_id)
    if voice_id != "samantha" and SEDUCTIVE in weights:
        _reject("seductive is Samantha only")
    if set(weights) != set(expected):
        _reject(
            "weights must name exactly "
            + ", ".join(expected)
            + "; coarse heads are not expanded into Emotion V2"
        )
    allowed = _UI_SLOT_KEYS if ui else _SLOT_KEYS
    parsed: dict[str, dict[str, object]] = {}
    for emotion in expected:
        slot = _object(weights[emotion], f"weights.{emotion}")
        _exact_keys(slot, allowed, f"weights.{emotion}")
        if "weight" not in slot or "gas_line" not in slot:
            _reject(f"weights.{emotion} needs weight and gas_line")
        gas_line = slot["gas_line"]
        if gas_line != GAS_LINES[emotion]:
            _reject(f"weights.{emotion} gas_line {gas_line!r} is not {GAS_LINES[emotion]}")
        weight = slot["weight"]
        if weight is not None:
            number = _finite(weight, f"weights.{emotion}.weight")
            if number < 0.0 or number > 1.0:
                _reject(f"weights.{emotion}.weight {number} is outside 0..1")
            weight = number
        element_id = ELEMENT_IDS[emotion]
        if ui:
            if slot.get("element_id") != element_id:
                _reject(
                    f"weights.{emotion} element_id {slot.get('element_id')!r} "
                    f"is not {element_id}"
                )
        parsed[emotion] = {
            "weight": weight,
            "gas_line": gas_line,
            "element_id": element_id,
        }
    return parsed


def validate_live(raw: object) -> dict[str, object]:
    """Return a checked ``galion-nebula-live-feed/1`` object.

    Unknown fields are rejected. The returned weights are in Emotion V2 order.
    """
    payload = _object(raw, "frame")
    _exact_keys(payload, _LIVE_KEYS, "frame")
    missing = sorted(_LIVE_KEYS - set(payload))
    if missing:
        _reject(f"frame is missing {', '.join(missing)}")
    if payload["schema"] != SCHEMA:
        _reject(f"schema must be {SCHEMA}")
    voice_id = _voice(payload["voice_id"], allow_null=False)
    assert voice_id is not None
    kind = payload["kind"]
    if kind not in ("frame", "utterance"):
        _reject("kind must be frame or utterance")
    if not isinstance(payload["present"], bool) or not isinstance(payload["distribution"], bool):
        _reject("present and distribution must be booleans")
    slots = _read_slots(payload["weights"], voice_id, ui=False)
    valence = _optional(payload["valence"], "valence", -1.0, 1.0)
    arousal = _optional(payload["arousal"], "arousal", -1.0, 1.0)
    dominance = _optional(payload["dominance"], "dominance", -1.0, 1.0)
    energy = _optional(payload["energy"], "energy", 0.0, 1.0)
    weights = {emotion: slots[emotion]["weight"] for emotion in slots}
    estimate = classify_estimate(
        present=payload["present"],
        distribution=payload["distribution"],
        weights=weights,
        axes=(valence, arousal, dominance, energy),
    )
    if payload["distribution"] and estimate != ESTIMATE_DISTRIBUTION:
        _reject("distribution flag does not match the weights")
    return {
        "schema": SCHEMA,
        "voice_id": voice_id,
        "t_ms": _t_ms(payload["t_ms"]),
        "kind": kind,
        "present": payload["present"],
        "distribution": payload["distribution"],
        "tau_ms": _tau(payload["tau_ms"]),
        "weights": {emotion: {"weight": slots[emotion]["weight"], "gas_line": slots[emotion]["gas_line"]} for emotion in emotions_for(voice_id)},
        "valence": valence,
        "arousal": arousal,
        "dominance": dominance,
        "energy": energy,
        "estimate": estimate,
    }


def _json_number(value: float | None) -> int | float | None:
    if value is None:
        return None
    rounded = round(float(value), 6)
    if rounded == int(rounded) and abs(rounded) < 1e15:
        return int(rounded)
    return rounded


def project(raw: object, *, seq: int) -> dict[str, object]:
    """Validate a live frame and return one UI message. ``seq`` starts at 1."""
    if isinstance(seq, bool) or not isinstance(seq, int) or seq < 1:
        _reject("seq must be an integer >= 1")
    live = validate_live(raw)
    voice_id = str(live["voice_id"])
    weights = {}
    for emotion in emotions_for(voice_id):
        slot = live["weights"][emotion]
        weights[emotion] = {
            "weight": _json_number(slot["weight"]),
            "gas_line": slot["gas_line"],
            "element_id": ELEMENT_IDS[emotion],
        }
    message = {
        "schema": UI_SCHEMA,
        "source_schema": SCHEMA,
        "voice_id": voice_id,
        "t_ms": live["t_ms"],
        "kind": live["kind"],
        "estimate": live["estimate"],
        "present": live["present"],
        "distribution": live["distribution"],
        "tau_ms": _json_number(live["tau_ms"]),
        "weights": weights,
        "valence": _json_number(live["valence"]),
        "arousal": _json_number(live["arousal"]),
        "dominance": _json_number(live["dominance"]),
        "energy": _json_number(live["energy"]),
        "seq": seq,
        "origin": "server",
        "reason": None,
        "detail": None,
    }
    validate_ui(message)
    return message


def project_text(line: str, *, seq: int) -> dict[str, object]:
    """Parse one live-feed JSON line and project it."""
    if not isinstance(line, str) or not line.strip():
        _reject("frame line is empty")
    try:
        raw = json.loads(line)
    except json.JSONDecodeError as exc:
        _reject(f"frame line is not JSON: {exc}")
    return project(raw, seq=seq)


def absence_message(
    *,
    voice_id: str | None,
    t_ms: int,
    kind: str,
    tau_ms: float,
    seq: int | None,
    origin: str,
    reason: str,
    detail: str | None,
) -> dict[str, object]:
    """An explicit no-estimate message. No weight is 1."""
    if origin not in ORIGINS:
        _reject("origin must be server or client")
    if reason not in ("rejected", "disconnected"):
        _reject("an absence message needs reason rejected or disconnected")
    if origin == "client":
        if reason != "disconnected" or seq is not None:
            _reject("a client line is a disconnect gap with seq null")
    elif seq is None or isinstance(seq, bool) or not isinstance(seq, int) or seq < 1:
        _reject("seq must be an integer >= 1")
    if kind not in ("frame", "utterance"):
        _reject("kind must be frame or utterance")
    voice = _voice(voice_id, allow_null=True)
    if voice is None and not (origin == "client" or reason == "rejected"):
        _reject("a measured frame needs a voice")
    stamp = _t_ms(t_ms)
    tau = _tau(tau_ms)
    weights = {
        emotion: {"weight": None, "gas_line": GAS_LINES[emotion], "element_id": ELEMENT_IDS[emotion]}
        for emotion in _emotions(voice)
    }
    message = {
        "schema": UI_SCHEMA,
        "source_schema": None,
        "voice_id": voice,
        "t_ms": stamp,
        "kind": kind,
        "estimate": ESTIMATE_NONE,
        "present": False,
        "distribution": False,
        "tau_ms": _json_number(tau),
        "weights": weights,
        "valence": None,
        "arousal": None,
        "dominance": None,
        "energy": None,
        "seq": seq,
        "origin": origin,
        "reason": reason,
        "detail": detail,
    }
    validate_ui(message)
    return message


def validate_ui(raw: object) -> dict[str, object]:
    """Check one UI message before it is written to a socket or to stdout."""
    payload = _object(raw, "ui frame")
    _exact_keys(payload, _UI_KEYS, "ui frame")
    missing = sorted(_UI_KEYS - set(payload))
    if missing:
        _reject(f"ui frame is missing {', '.join(missing)}")
    if payload["schema"] != UI_SCHEMA:
        _reject(f"schema must be {UI_SCHEMA}")
    origin = payload["origin"]
    if origin not in ORIGINS:
        _reject("origin must be server or client")
    reason = payload["reason"]
    if reason not in REASONS:
        _reject("reason must be null, rejected, or disconnected")
    estimate = payload["estimate"]
    if estimate not in ESTIMATES:
        _reject("estimate must be none, distribution, or partial")
    if origin == "client":
        if estimate != ESTIMATE_NONE or reason != "disconnected" or payload["seq"] is not None:
            _reject("a client line is only a disconnect gap")
        if payload["source_schema"] is not None:
            _reject("a client gap has no source frame")
    else:
        seq = payload["seq"]
        if isinstance(seq, bool) or not isinstance(seq, int) or seq < 1:
            _reject("seq must be an integer >= 1")
        if reason == "disconnected":
            _reject("the server does not invent a disconnect")
        if reason == "rejected":
            if estimate != ESTIMATE_NONE or payload["source_schema"] is not None:
                _reject("a rejected line is an absence with no source frame")
        elif payload["source_schema"] != SCHEMA:
            _reject(f"source_schema must be {SCHEMA}")
    detail = payload["detail"]
    if detail is not None and not isinstance(detail, str):
        _reject("detail must be a string or null")
    voice_id = _voice(payload["voice_id"], allow_null=(origin == "client" or reason == "rejected"))
    if voice_id is None and estimate != ESTIMATE_NONE:
        _reject("a frame with no voice has no emotion estimate")
    kind = payload["kind"]
    if kind not in ("frame", "utterance"):
        _reject("kind must be frame or utterance")
    if not isinstance(payload["present"], bool) or not isinstance(payload["distribution"], bool):
        _reject("present and distribution must be booleans")
    if voice_id is None:
        expected = _emotions(None)
        weights = _object(payload["weights"], "weights")
        if set(weights) != set(expected):
            _reject("a gap with no voice lists the twelve shared emotions")
        for emotion in expected:
            slot = _object(weights[emotion], f"weights.{emotion}")
            _exact_keys(slot, _UI_SLOT_KEYS, f"weights.{emotion}")
            if slot["weight"] is not None or slot["gas_line"] != GAS_LINES[emotion]:
                _reject("a gap weight is null and keeps its gas line")
            if slot["element_id"] != ELEMENT_IDS[emotion]:
                _reject(f"weights.{emotion} element_id does not match the picture")
        parsed_weights: dict[str, float | None] = {emotion: None for emotion in expected}
    else:
        slots = _read_slots(payload["weights"], voice_id, ui=True)
        parsed_weights = {emotion: slots[emotion]["weight"] for emotion in slots}  # type: ignore[assignment]
    valence = _optional(payload["valence"], "valence", -1.0, 1.0)
    arousal = _optional(payload["arousal"], "arousal", -1.0, 1.0)
    dominance = _optional(payload["dominance"], "dominance", -1.0, 1.0)
    energy = _optional(payload["energy"], "energy", 0.0, 1.0)
    _t_ms(payload["t_ms"])
    _tau(payload["tau_ms"])
    named = classify_estimate(
        present=payload["present"],
        distribution=payload["distribution"],
        weights=parsed_weights,
        axes=(valence, arousal, dominance, energy),
    )
    if named != estimate:
        _reject(f"estimate {estimate} does not match the numbers ({named})")
    return payload


def encode_ui(message: Mapping[str, object]) -> str:
    """One JSON object, no trailing newline. The socket and stdout add one."""
    checked = validate_ui(dict(message))
    return json.dumps(checked, ensure_ascii=False, separators=(",", ":"), allow_nan=False)
