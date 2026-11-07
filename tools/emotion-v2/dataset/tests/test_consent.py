# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Consent gate. No audio, no weights, no network."""

from __future__ import annotations

import json
import tempfile
import unittest
from datetime import date
from pathlib import Path

from emotion_v2_dataset.audio import AudioBuffer, synthesize_stub
from emotion_v2_dataset.backends import ChatterboxBackend, RenderJob
from emotion_v2_dataset.consent import (
    TRAINING_SCOPE,
    load_consent,
    parse_consent,
    require_training_consent,
)
from emotion_v2_dataset.errors import ConsentError, RealVoiceRefused
from emotion_v2_dataset.paths import DATA_DIR


TODAY = date(2026, 11, 7)


def _record(**overrides: object) -> dict:
    raw: dict[str, object] = {
        "voice_id": "stub-sine",
        "owner": "synthetic fixture",
        "consent_scope": [TRAINING_SCOPE],
        "signed_date": "2026-11-01",
    }
    raw.update(overrides)
    return raw


def _job() -> RenderJob:
    return RenderJob(
        voice_id="stub-sine",
        kind="synthetic",
        text="The notebook is on the table.",
        emotion="Neutral",
        intensity=0.4,
        pitch_standard_hz=432.0,
        pitch_centre_hz=216.0,
        scenario_id="neutral-01",
    )


class ConsentRecordTests(unittest.TestCase):
    def test_example_file_authorizes_the_stub_id(self) -> None:
        manifest = load_consent(DATA_DIR / "consent.example.json", today=TODAY)
        self.assertEqual(manifest.voice_id, "stub-sine")
        self.assertIn(TRAINING_SCOPE, manifest.consent_scope)

    def test_clone_scope_does_not_authorize_training_data(self) -> None:
        manifest = parse_consent(
            _record(consent_scope=["local-zero-shot-conditioning"])
        )
        with self.assertRaises(ConsentError):
            require_training_consent(manifest, "stub-sine", today=TODAY)

    def test_missing_manifest_refuses(self) -> None:
        with self.assertRaises(ConsentError):
            require_training_consent(None, "stub-sine", today=TODAY)

    def test_voice_mismatch_refuses(self) -> None:
        manifest = parse_consent(_record())
        with self.assertRaises(ConsentError):
            require_training_consent(manifest, "other-voice", today=TODAY)

    def test_future_date_and_placeholders_refuse(self) -> None:
        future = parse_consent(_record(signed_date="2026-12-01"))
        with self.assertRaises(ConsentError):
            require_training_consent(future, "stub-sine", today=TODAY)
        with self.assertRaises(ConsentError):
            parse_consent(_record(owner="TODO"))
        with self.assertRaises(ConsentError):
            parse_consent(_record(signed_date="tomorrow"))

    def test_blocked_voice_refuses_even_with_the_scope(self) -> None:
        with self.assertRaises(ConsentError):
            parse_consent(_record(voice_id="armstrong-test", owner="a studio"))

    def test_comma_separated_scope_is_accepted(self) -> None:
        manifest = parse_consent(
            _record(consent_scope=f"preview, {TRAINING_SCOPE}")
        )
        require_training_consent(manifest, "stub-sine", today=TODAY)


class ChatterboxGateTests(unittest.TestCase):
    def test_engine_is_not_called_without_consent(self) -> None:
        engine = _CountingEngine()
        backend = ChatterboxBackend(None, engine=engine, today=TODAY)
        with self.assertRaises(ConsentError):
            backend.render(_job())
        self.assertEqual(engine.calls, 0)

    def test_valid_consent_without_an_engine_does_not_invent_a_voice(self) -> None:
        manifest = parse_consent(_record())
        backend = ChatterboxBackend(manifest, engine=None, today=TODAY)
        with self.assertRaises(RealVoiceRefused):
            backend.render(_job())

    def test_valid_consent_calls_only_the_injected_engine(self) -> None:
        manifest = parse_consent(_record())
        engine = _CountingEngine()
        backend = ChatterboxBackend(manifest, engine=engine, today=TODAY)
        audio = backend.render(_job())
        self.assertEqual(engine.calls, 1)
        self.assertGreater(len(audio.samples), 0)

    def test_missing_file_refuses(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            missing = Path(tmp) / "no-consent.json"
            with self.assertRaises(ConsentError):
                load_consent(missing, today=TODAY)


class _CountingEngine:
    def __init__(self) -> None:
        self.calls = 0

    def render(self, job: RenderJob) -> AudioBuffer:
        self.calls += 1
        return synthesize_stub(
            text=job.text,
            emotion=job.emotion,
            intensity=job.intensity,
            pitch_centre_hz=job.pitch_centre_hz,
            pitch_standard_hz=job.pitch_standard_hz,
        )


class ConsentFileRoundTripTests(unittest.TestCase):
    def test_written_json_reloads(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "consent.json"
            path.write_text(json.dumps(_record()), encoding="utf-8")
            manifest = load_consent(path, today=TODAY)
            self.assertEqual(manifest.owner, "synthetic fixture")


if __name__ == "__main__":
    unittest.main()
