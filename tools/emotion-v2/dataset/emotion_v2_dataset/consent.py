# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Per-voice consent gate for training-data generation.

Real-voice generation, and any use of the Chatterbox backend, requires a
JSON consent record with:

* ``voice_id`` — the same id as the voice profile
* ``owner`` — who granted consent
* ``consent_scope`` — a list that includes ``training-data-generation``
* ``signed_date`` — ISO date ``YYYY-MM-DD``, not in the future

The clone-time scope ``local-zero-shot-conditioning`` (see ``galion-consent``)
does not authorize this pipeline. A failed check raises :class:`ConsentError`
before any engine is called.

This gate does not read audio and does not write a signature of its own.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from datetime import date, datetime
from pathlib import Path
from typing import Any

from emotion_v2_dataset.errors import ConsentError

TRAINING_SCOPE = "training-data-generation"

# Same celebrity block as galion-consent: LOCKS notes "No celebrity clone."
BLOCKED_VOICE_SUBSTRINGS: tuple[str, ...] = ("armstrong",)

PLACEHOLDERS: frozenset[str] = frozenset(
    {
        "todo",
        "tbd",
        "placeholder",
        "unknown",
        "n/a",
        "na",
        "none",
        "null",
        "fixme",
        "xxx",
        "changeme",
        "owner",
        "name",
    }
)


@dataclass(frozen=True)
class ConsentManifest:
    voice_id: str
    owner: str
    consent_scope: tuple[str, ...]
    signed_date: date
    path: str | None = None

    def allows_training(self) -> bool:
        return TRAINING_SCOPE in self.consent_scope


def load_consent(path: Path, *, today: date | None = None) -> ConsentManifest:
    """Load and validate a consent file. Raises :class:`ConsentError`."""
    file = Path(path)
    if not file.is_file():
        raise ConsentError(f"consent file is missing: {file}")
    try:
        raw = json.loads(file.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise ConsentError(f"consent file is not JSON: {file.name}") from exc
    if not isinstance(raw, dict):
        raise ConsentError("consent file must be a JSON object")
    manifest = parse_consent(raw, path=str(file))
    require_training_consent(manifest, manifest.voice_id, today=today)
    return manifest


def parse_consent(raw: dict[str, Any], *, path: str | None = None) -> ConsentManifest:
    voice_id = _required_text(raw, "voice_id")
    owner = _required_text(raw, "owner")
    signed = _required_text(raw, "signed_date")
    scope = _scope(raw.get("consent_scope"))
    _reject_placeholder("voice_id", voice_id)
    _reject_placeholder("owner", owner)
    _reject_blocked(voice_id)
    _reject_blocked(owner)
    try:
        signed_date = date.fromisoformat(signed)
    except ValueError as exc:
        raise ConsentError("signed_date must be an ISO date YYYY-MM-DD") from exc
    return ConsentManifest(
        voice_id=voice_id,
        owner=owner,
        consent_scope=scope,
        signed_date=signed_date,
        path=path,
    )


def require_training_consent(
    manifest: ConsentManifest | None,
    voice_id: str,
    *,
    today: date | None = None,
) -> ConsentManifest:
    """Refuse unless ``manifest`` authorizes training data for ``voice_id``."""
    if manifest is None:
        raise ConsentError(
            "refusing generation: no consent manifest. Real-voice and "
            "Chatterbox renders require voice_id, owner, consent_scope "
            f"including {TRAINING_SCOPE!r}, and signed_date"
        )
    if manifest.voice_id != voice_id:
        raise ConsentError(
            "consent voice_id does not match the requested voice"
        )
    _reject_blocked(voice_id)
    if TRAINING_SCOPE not in manifest.consent_scope:
        raise ConsentError(
            f"consent_scope must include {TRAINING_SCOPE!r}; "
            "other scopes, including local-zero-shot-conditioning, do not apply"
        )
    current = today or datetime.now().date()
    if manifest.signed_date > current:
        raise ConsentError("signed_date is in the future")
    return manifest


def _required_text(raw: dict[str, Any], key: str) -> str:
    value = raw.get(key)
    if not isinstance(value, str) or not value.strip():
        raise ConsentError(f"consent field {key} is missing")
    return value.strip()


def _scope(value: object) -> tuple[str, ...]:
    if isinstance(value, str):
        parts = [part.strip() for part in value.split(",")]
        items = tuple(part for part in parts if part)
    elif isinstance(value, list) and all(isinstance(item, str) for item in value):
        items = tuple(item.strip() for item in value if item.strip())
    else:
        raise ConsentError("consent_scope must be a list of strings")
    if not items:
        raise ConsentError("consent_scope is empty")
    for item in items:
        _reject_placeholder("consent_scope", item)
    return items


def _reject_placeholder(field: str, value: str) -> None:
    if value.casefold() in PLACEHOLDERS:
        raise ConsentError(f"consent field {field} is a placeholder")


def _reject_blocked(value: str) -> None:
    folded = value.casefold()
    for blocked in BLOCKED_VOICE_SUBSTRINGS:
        if blocked in folded:
            raise ConsentError("refusing a blocked voice id")
