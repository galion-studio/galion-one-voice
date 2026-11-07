# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Thresholds, stub posteriors, and the offline SUPERB contract.

Every signal in this module is synthesized. No fixture audio is read from git.
"""

from __future__ import annotations

import math
import os
import unittest
from pathlib import Path

import support

from harness.audio import read_wav, synthesize_tone, write_wav
from harness.backends import (
    SUPERB_ID,
    BackendUnavailable,
    StubBackend,
    SuperbBackend,
    discover_superb_weights,
    resolve_backend,
)
from harness.config import default_scenarios_path, load_scenarios, load_thresholds
from harness.emotions import EMOTIONS, STUB_FREQ_HZ, display_name
from harness.metrics import cosine, ecapa_passes, meets_min, wer_passes

# Locked scenario lines. The files under tools/emotion-v2/scenarios are the
# harness input; this table fails the build if a line changes by accident.
SCENARIOS = {
    "neutral": (
        "The note is on the table. I will read it at four.",
        "The window is open. The room is quiet.",
    ),
    "calm": (
        "We can do this one step at a time.",
        "There is time. Sit with me for a moment.",
    ),
    "happy": (
        "That worked. I am glad it landed.",
        "The light came back. I wanted you to hear it.",
    ),
    "amused": (
        "That one landed. I did not see it coming.",
        "You kept a straight face. I could not.",
    ),
    "excited": (
        "It is ready. Come and hear the first line.",
        "We finished early. I want to play it now.",
    ),
    "tender": (
        "I am here. Take the time you need.",
        "You do not have to explain it. I will wait.",
    ),
    "sad": (
        "I missed it. I wish I had been there.",
        "The house is quiet now. I keep the light on.",
    ),
    "disappointed": (
        "I thought we would finish today. We did not.",
        "I kept the evening open. It went to someone else.",
    ),
    "angry": (
        "That was not fair. I need it corrected.",
        "Do not talk over me. I am not finished.",
    ),
    "fearful": (
        "I do not know what happens next. Stay with me.",
        "The hall is dark. I do not want to go alone.",
    ),
    "surprised": (
        "I did not expect that. Say it once more.",
        "You came back. I thought the door was shut.",
    ),
    "disgusted": (
        "That does not belong here. Take it away.",
        "Leave it outside. I will not keep it in the room.",
    ),
}


class ThresholdsTest(unittest.TestCase):
    def test_twelve_emotions_and_display_names(self) -> None:
        self.assertEqual(
            EMOTIONS,
            (
                "neutral",
                "calm",
                "happy",
                "amused",
                "excited",
                "tender",
                "sad",
                "disappointed",
                "angry",
                "fearful",
                "surprised",
                "disgusted",
            ),
        )
        self.assertEqual(
            [display_name(name) for name in EMOTIONS],
            [
                "Neutral",
                "Calm",
                "Happy",
                "Amused",
                "Excited",
                "Tender",
                "Sad",
                "Disappointed",
                "Angry",
                "Fearful",
                "Surprised",
                "Disgusted",
            ],
        )

    def test_checked_in_targets(self) -> None:
        thresholds = load_thresholds()
        self.assertEqual(thresholds.persona, "samantha")
        self.assertEqual(thresholds.tuning_hz, 432)
        self.assertEqual(thresholds.backend_id, SUPERB_ID)
        self.assertTrue(math.isclose(thresholds.wer_max, 0.018))
        self.assertTrue(math.isclose(thresholds.ecapa_min, 0.7057))
        self.assertTrue(math.isclose(thresholds.emotion_target, 0.83))
        self.assertIsNone(thresholds.embedding_min)
        for emotion in EMOTIONS:
            self.assertTrue(math.isclose(thresholds.target_for(emotion), 0.83))

    def test_guard_boundaries(self) -> None:
        self.assertTrue(wer_passes(0.018, 0.018))
        self.assertTrue(wer_passes(0.0, 0.018))
        self.assertFalse(wer_passes(0.018 + 1e-6, 0.018))
        self.assertFalse(ecapa_passes(0.7057, 0.7057))
        self.assertFalse(ecapa_passes(0.7057 - 1e-4, 0.7057))
        self.assertTrue(ecapa_passes(0.7057 + 1e-6, 0.7057))
        self.assertTrue(meets_min(0.83, 0.83))
        self.assertFalse(meets_min(0.83 - 1e-4, 0.83))

    def test_fixed_scenario_texts(self) -> None:
        root = default_scenarios_path()
        self.assertEqual(set(SCENARIOS), set(EMOTIONS))
        for emotion, expected in SCENARIOS.items():
            loaded = load_scenarios(root, emotion)
            self.assertEqual(tuple(item.text for item in loaded), expected)
            self.assertEqual([item.stem for item in loaded], ["s01", "s02"])


class StubBackendTest(unittest.TestCase):
    def test_posterior_is_deterministic_and_a_distribution(self) -> None:
        stub = StubBackend()
        tone = synthesize_tone(432.0, sample_rate=16000, seconds=0.4)
        first = stub.analyse(tone, 16000)
        second = stub.analyse(list(tone), 16000)
        self.assertEqual(first, second)
        self.assertEqual(first.labels, EMOTIONS)
        self.assertAlmostEqual(sum(first.posterior), 1.0, places=6)
        self.assertTrue(all(value >= 0.0 for value in first.posterior))
        self.assertEqual(len(first.embedding), len(EMOTIONS))

    def test_432_hz_tone_peaks_on_amused(self) -> None:
        stub = StubBackend()
        for emotion, freq in STUB_FREQ_HZ.items():
            analysis = stub.analyse(synthesize_tone(freq), 16000)
            winner = EMOTIONS[max(range(len(EMOTIONS)), key=lambda i: analysis.posterior[i])]
            self.assertEqual(winner, emotion)
            self.assertGreater(analysis.posterior[EMOTIONS.index(emotion)], 0.9)
        amused = stub.analyse(synthesize_tone(STUB_FREQ_HZ["amused"]), 16000)
        self.assertEqual(STUB_FREQ_HZ["amused"], 432.0)
        self.assertEqual(EMOTIONS[max(range(12), key=lambda i: amused.posterior[i])], "amused")

    def test_divergent_tones_are_not_similar(self) -> None:
        stub = StubBackend()
        neutral = stub.analyse(synthesize_tone(STUB_FREQ_HZ["neutral"]), 16000)
        disgusted = stub.analyse(synthesize_tone(STUB_FREQ_HZ["disgusted"]), 16000)
        self.assertLess(cosine(list(neutral.posterior), list(disgusted.posterior)), 0.83)
        self.assertLess(cosine(list(neutral.embedding), list(disgusted.embedding)), 0.83)
        self.assertAlmostEqual(cosine(list(neutral.posterior), list(neutral.posterior)), 1.0)

    def test_wav_roundtrip_keeps_the_peak(self) -> None:
        import tempfile

        stub = StubBackend()
        with tempfile.TemporaryDirectory(prefix="emotion-v2-tone-") as tmp:
            path = Path(tmp) / "tone.wav"
            write_wav(path, synthesize_tone(432.0), 16000)
            samples, rate = read_wav(path)
            self.assertEqual(rate, 16000)
            analysis = stub.analyse(samples, rate)
            winner = EMOTIONS[max(range(12), key=lambda i: analysis.posterior[i])]
            self.assertEqual(winner, "amused")


class SuperbOfflineTest(unittest.TestCase):
    def setUp(self) -> None:
        self._saved = os.environ.get("EMOTION_V2_SUPERB_WEIGHTS")

    def tearDown(self) -> None:
        if self._saved is None:
            os.environ.pop("EMOTION_V2_SUPERB_WEIGHTS", None)
        else:
            os.environ["EMOTION_V2_SUPERB_WEIGHTS"] = self._saved

    def test_loader_source_never_downloads(self) -> None:
        source = (support.EMOTION_V2 / "harness" / "backends.py").read_text(encoding="utf-8")
        self.assertNotIn("snapshot_download", source)
        self.assertNotIn("hf_hub_download", source)
        self.assertNotIn("local_files_only=False", source)
        self.assertIn("local_files_only=True", source)
        self.assertNotIn('from_pretrained("superb', source)
        self.assertNotIn("from_pretrained(SUPERB_ID", source)
        self.assertIn("from_pretrained(directory, local_files_only=True)", source)

    def test_missing_override_does_not_create_a_directory(self) -> None:
        import tempfile

        with tempfile.TemporaryDirectory() as tmp:
            missing = Path(tmp) / "absent-weights"
            os.environ["EMOTION_V2_SUPERB_WEIGHTS"] = str(missing)
            self.assertIsNone(discover_superb_weights())
            self.assertFalse(missing.exists())
            with self.assertRaises(BackendUnavailable):
                resolve_backend("superb")
            self.assertIsInstance(resolve_backend("auto"), StubBackend)
            self.assertIsInstance(resolve_backend("stub"), StubBackend)

    def test_incomplete_directory_is_not_a_weight_cache(self) -> None:
        import tempfile

        with tempfile.TemporaryDirectory() as tmp:
            weights = Path(tmp) / "wav2vec2-base-superb-er"
            weights.mkdir()
            (weights / "config.json").write_text("{}\n", encoding="utf-8")
            os.environ["EMOTION_V2_SUPERB_WEIGHTS"] = str(weights)
            self.assertIsNone(discover_superb_weights())
            (weights / "model.safetensors").write_bytes(b"not-a-real-model")
            found = discover_superb_weights()
            self.assertEqual(found, weights)
            backend = resolve_backend("superb")
            self.assertIsInstance(backend, SuperbBackend)
            self.assertEqual(backend.name, SUPERB_ID)

    def test_workflow_is_cpu_stub_only(self) -> None:
        workflow = (support.REPO / ".github" / "workflows" / "emotion-v2-ci.yml").read_text(
            encoding="utf-8"
        )
        self.assertIn("unittest discover", workflow)
        self.assertIn('EMOTION_V2_BACKEND: stub', workflow)
        self.assertIn('HF_HUB_OFFLINE: "1"', workflow)
        for banned in (
            "huggingface.co",
            "snapshot_download",
            "pip install",
            "wget ",
            "curl ",
            "cron:",
            "symphony-v2-emotion42",
        ):
            self.assertNotIn(banned, workflow)
        tree = support.EMOTION_V2
        forbidden = {".wav", ".wave", ".mp3", ".flac", ".ogg", ".safetensors", ".onnx", ".bin", ".pt", ".ckpt"}
        hits = [path for path in tree.rglob("*") if path.suffix.lower() in forbidden]
        self.assertEqual(hits, [])


if __name__ == "__main__":
    unittest.main()
