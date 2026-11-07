# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Augmentation, licences, and the quality gate. Synthetic signals only."""

from __future__ import annotations

import unittest

import numpy as np

from emotion_v2_dataset.audio import (
    AudioBuffer,
    measure_snr_db,
    mix_snr,
    simulate_headset,
    simulate_phone,
    synthesize_stub,
)
from emotion_v2_dataset.augment import augment
from emotion_v2_dataset.environments import (
    IR_LICENCE,
    NOISE_LICENCE,
    ENVIRONMENTS,
    get_environment,
)
from emotion_v2_dataset.quality import (
    FunctionScorer,
    QualityThresholds,
    RenderedClip,
    decide,
    score_clip,
)


class AugmentTests(unittest.TestCase):
    def setUp(self) -> None:
        self.dry = synthesize_stub(
            text="The notebook is on the table.",
            emotion="Neutral",
            intensity=0.4,
            pitch_centre_hz=216.0,
        )

    def test_matrix_has_the_eight_environments(self) -> None:
        names = [item.name for item in ENVIRONMENTS]
        self.assertEqual(
            names,
            [
                "dry studio",
                "small room",
                "living room",
                "car",
                "cafe/babble",
                "street",
                "phone/narrowband 8 kHz",
                "headset/bluetooth codec",
            ],
        )

    def test_dry_studio_matches_the_render_and_records_a_licence(self) -> None:
        rendered = augment(self.dry, get_environment("dry studio"), None, seed=1)
        self.assertTrue(np.allclose(rendered.audio.samples, self.dry.samples))
        self.assertEqual(rendered.ir_licence, IR_LICENCE)
        self.assertIn("No recording", rendered.ir_source)
        self.assertIsNone(rendered.snr_db)

    def test_snr_mix_hits_the_requested_level(self) -> None:
        generator = np.random.default_rng(0)
        speech = 0.2 * np.sin(2 * np.pi * 200 * np.arange(8000) / 24000)
        noise = generator.standard_normal(8000)
        mixed = mix_snr(speech, noise, 10.0)
        self.assertAlmostEqual(measure_snr_db(speech, mixed), 10.0, delta=0.2)

    def test_room_changes_the_signal_and_names_cc0_sources(self) -> None:
        rendered = augment(self.dry, get_environment("small room"), 25.0, seed=3)
        self.assertFalse(np.allclose(rendered.audio.samples[: len(self.dry.samples)], self.dry.samples))
        self.assertEqual(rendered.ir_licence, IR_LICENCE)
        self.assertEqual(rendered.noise_licence, NOISE_LICENCE)
        self.assertIn("in-process", rendered.noise_source)

    def test_phone_keeps_the_voice_band_and_drops_6_khz(self) -> None:
        rate = 24000
        time = np.arange(rate) / rate
        tone = 0.4 * np.sin(2 * np.pi * 500 * time) + 0.4 * np.sin(2 * np.pi * 6000 * time)
        phone = simulate_phone(tone, rate)
        self.assertEqual(phone.sample_rate, 8000)
        restored = _upsample(phone.samples, 8000, rate)
        count = min(len(restored), len(tone))
        low = np.sin(2 * np.pi * 500 * time[:count])
        high = np.sin(2 * np.pi * 6000 * time[:count])
        self.assertGreater(_corr(restored[:count], low), 0.8)
        self.assertLess(abs(_corr(restored[:count], high)), 0.25)

    def test_every_environment_augments(self) -> None:
        for environment in ENVIRONMENTS:
            rendered = augment(self.dry, environment, environment.nominal_snr_db, seed=9)
            self.assertGreater(len(rendered.audio.samples), 0)
            self.assertEqual(rendered.environment, environment.name)
            if environment.telephony_hz == 8000:
                self.assertEqual(rendered.audio.sample_rate, 8000)
            else:
                self.assertEqual(rendered.audio.sample_rate, 24000)

    def test_headset_uses_mu_law_and_cuts_10_khz(self) -> None:
        rate = 24000
        time = np.arange(rate) / rate
        voice = 0.4 * np.sin(2 * np.pi * 1000 * time)
        high = 0.4 * np.sin(2 * np.pi * 10000 * time)
        mixed = voice + high
        coded = simulate_headset(mixed, rate)
        self.assertFalse(np.allclose(coded, mixed))
        self.assertGreater(_corr(coded, voice), 0.8)
        self.assertLess(abs(_corr(coded, high)), 0.25)


class QualityTests(unittest.TestCase):
    def test_missing_scorers_pass_until_required(self) -> None:
        card, _unused = score_clip(_clip(), [])
        self.assertTrue(decide(card, QualityThresholds()).accepted)
        refused = decide(card, QualityThresholds(require_scorers=True))
        self.assertFalse(refused.accepted)
        self.assertIn("missing scorers", refused.reason)

    def test_wer_and_emotion_thresholds(self) -> None:
        poor = FunctionScorer("wer", lambda _clip: 0.9)
        card, _unused = score_clip(_clip(), [poor])
        self.assertFalse(decide(card, QualityThresholds()).accepted)
        good = FunctionScorer("emotion", lambda _clip: 0.2)
        card, _unused = score_clip(_clip(), [good])
        decision = decide(card, QualityThresholds())
        self.assertFalse(decision.accepted)
        self.assertIn("emotion", decision.reason)


def _clip() -> RenderedClip:
    audio = AudioBuffer(np.zeros(100), 24000)
    return RenderedClip(
        audio=audio,
        text="hello",
        emotion="Neutral",
        voice_id="stub-sine",
        intensity=0.3,
    )


def _corr(left: np.ndarray, right: np.ndarray) -> float:
    left = left - float(np.mean(left))
    right = right - float(np.mean(right))
    denom = float(np.linalg.norm(left) * np.linalg.norm(right)) + 1e-12
    return float(np.dot(left, right) / denom)


def _upsample(samples: np.ndarray, source_rate: int, target_rate: int) -> np.ndarray:
    count = int(round(len(samples) / source_rate * target_rate))
    source_t = np.linspace(0.0, 1.0, len(samples), endpoint=False)
    target_t = np.linspace(0.0, 1.0, count, endpoint=False)
    return np.interp(target_t, source_t, samples)


if __name__ == "__main__":
    unittest.main()
