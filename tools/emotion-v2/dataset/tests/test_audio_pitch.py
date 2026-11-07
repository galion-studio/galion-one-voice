# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Pitch standard and stub synthesis. Synthetic signals only."""

from __future__ import annotations

import unittest

from emotion_v2_dataset.audio import dominant_hz, synthesize_stub
from emotion_v2_dataset.errors import PitchPolicyError, ProfileError
from emotion_v2_dataset.profile import (
    FORBIDDEN_RETUNE_HZ,
    PITCH_STANDARD_HZ,
    forbid_retune_target,
    parse_profile,
    shift_ratio,
)
from emotion_v2_dataset.taxonomy import CANONICAL_EMOTIONS, EMOTION_CENTS


class PitchPolicyTests(unittest.TestCase):
    def test_standard_must_be_432(self) -> None:
        with self.assertRaises(PitchPolicyError):
            parse_profile(
                {
                    "voice_id": "stub-sine",
                    "kind": "synthetic",
                    "pitch_standard_hz": FORBIDDEN_RETUNE_HZ,
                    "pitch_centre_hz": 216,
                }
            )

    def test_retune_target_440_is_refused(self) -> None:
        with self.assertRaises(PitchPolicyError):
            forbid_retune_target(440)
        with self.assertRaises(PitchPolicyError):
            shift_ratio(216.0, 440.0)
        with self.assertRaises(PitchPolicyError):
            parse_profile(
                {
                    "voice_id": "stub-sine",
                    "kind": "synthetic",
                    "pitch_standard_hz": PITCH_STANDARD_HZ,
                    "pitch_centre_hz": 216,
                    "retune_hz": 440,
                }
            )
        with self.assertRaises(PitchPolicyError):
            parse_profile(
                {
                    "voice_id": "stub-sine",
                    "kind": "synthetic",
                    "pitch_standard_hz": PITCH_STANDARD_HZ,
                    "pitch_centre_hz": 216,
                    "retune_hz": 430,
                }
            )

    def test_a_speaking_fundamental_near_440_is_not_a_retune(self) -> None:
        profile = parse_profile(
            {
                "voice_id": "high-sine",
                "kind": "synthetic",
                "pitch_standard_hz": 432,
                "pitch_centre_hz": 440,
            }
        )
        self.assertEqual(profile.pitch_standard_hz, 432.0)
        self.assertIsNone(profile.retune_hz)

    def test_non_concert_shift_is_allowed(self) -> None:
        self.assertAlmostEqual(shift_ratio(216.0, 430.0), 430.0 / 216.0)

    def test_unknown_emotion_in_references_is_refused(self) -> None:
        with self.assertRaises(ProfileError):
            parse_profile(
                {
                    "voice_id": "stub-sine",
                    "kind": "synthetic",
                    "pitch_standard_hz": 432,
                    "pitch_centre_hz": 216,
                    "references": {"Joy": "refs/joy"},
                }
            )


class StubSignalTests(unittest.TestCase):
    def test_neutral_sits_on_the_declared_centre(self) -> None:
        audio = synthesize_stub(
            text="The notebook is on the table.",
            emotion="Neutral",
            intensity=0.5,
            pitch_centre_hz=216.0,
        )
        estimated = dominant_hz(audio.samples, audio.sample_rate)
        self.assertAlmostEqual(estimated, 216.0, delta=8.0)
        self.assertGreater(abs(estimated - 440.0), 100.0)

    def test_emotion_offset_is_cents_from_the_centre(self) -> None:
        intensity = 1.0
        audio = synthesize_stub(
            text="We leave at dawn and I cannot wait.",
            emotion="Excited",
            intensity=intensity,
            pitch_centre_hz=216.0,
        )
        expected = 216.0 * (2.0 ** (EMOTION_CENTS["Excited"] * intensity / 1200.0))
        estimated = dominant_hz(audio.samples, audio.sample_rate)
        self.assertAlmostEqual(estimated, expected, delta=10.0)
        self.assertLess(estimated, 300.0)

    def test_every_emotion_renders_without_a_reference(self) -> None:
        for emotion in CANONICAL_EMOTIONS:
            audio = synthesize_stub(
                text="A short synthetic line.",
                emotion=emotion,
                intensity=0.5,
                pitch_centre_hz=216.0,
            )
            self.assertEqual(audio.sample_rate, 24000)
            self.assertGreater(audio.duration_s, 0.4)


if __name__ == "__main__":
    unittest.main()
