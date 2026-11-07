# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Pluggable render backends.

The default backend is ``stub``: harmonic sines and noise, no reference
audio, no consent required for a synthetic profile.

``chatterbox`` is the real-voice slot. It refuses to call an engine unless a
consent manifest authorizes ``training-data-generation`` for this ``voice_id``.
This package does not import Chatterbox, does not download weights, and does
not ship an engine. The CLI therefore cannot clone a voice. A later operator
can pass an engine object to :class:`ChatterboxBackend` after the gate passes.
Tests pass a callable that returns a sine.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from datetime import date
from typing import Protocol

from emotion_v2_dataset.audio import AudioBuffer, synthesize_stub
from emotion_v2_dataset.consent import ConsentManifest, require_training_consent
from emotion_v2_dataset.errors import ConsentError, PipelineError, RealVoiceRefused


@dataclass(frozen=True)
class RenderJob:
    voice_id: str
    kind: str
    text: str
    emotion: str
    intensity: float
    pitch_standard_hz: float
    pitch_centre_hz: float
    scenario_id: str
    consent_ok: bool = False
    references: dict[str, str] = field(default_factory=dict)


class RenderEngine(Protocol):
    def render(self, job: RenderJob) -> AudioBuffer:
        """Return dry mono audio. Must not be a copy of a reference clip."""


class Backend(Protocol):
    name: str
    synthetic_signal: bool

    def render(self, job: RenderJob) -> AudioBuffer:
        """Render one dry line."""


class StubBackend:
    """Default backend. Synthetic signal only."""

    name = "stub"
    synthetic_signal = True

    def render(self, job: RenderJob) -> AudioBuffer:
        if job.kind == "real" and not job.consent_ok:
            raise ConsentError(
                "stub refuses a real voice profile without training consent"
            )
        # Reference paths stay on the job and are not opened. Reading them
        # could copy a person's recording into the dataset.
        _ = job.references
        return synthesize_stub(
            text=job.text,
            emotion=job.emotion,
            intensity=job.intensity,
            pitch_centre_hz=job.pitch_centre_hz,
            pitch_standard_hz=job.pitch_standard_hz,
        )


class ChatterboxBackend:
    """Consent-gated adapter. No weights and no bundled cloner."""

    name = "chatterbox"
    synthetic_signal = False

    def __init__(
        self,
        consent: ConsentManifest | None,
        engine: RenderEngine | None = None,
        *,
        today: date | None = None,
    ) -> None:
        self.consent = consent
        self.engine = engine
        self.today = today

    def render(self, job: RenderJob) -> AudioBuffer:
        # The engine is not touched until the record validates.
        require_training_consent(self.consent, job.voice_id, today=self.today)
        if self.engine is None:
            raise RealVoiceRefused(
                "Chatterbox backend is consent-gated and has no local engine. "
                "This package does not download weights and does not synthesize "
                "or clone a voice."
            )
        rendered = self.engine.render(job)
        if not isinstance(rendered, AudioBuffer):
            raise PipelineError("Chatterbox engine must return an AudioBuffer")
        return rendered


def make_backend(
    name: str,
    *,
    consent: ConsentManifest | None,
    engine: RenderEngine | None,
    today: date | None,
) -> Backend:
    if name == "stub":
        return StubBackend()
    if name == "chatterbox":
        return ChatterboxBackend(consent, engine=engine, today=today)
    raise PipelineError("backend must be 'stub' or 'chatterbox'")
