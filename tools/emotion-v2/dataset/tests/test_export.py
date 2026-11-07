# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""PersonaPlex adapter. Files stay in a temporary directory."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

import numpy as np

from emotion_v2_dataset.audio import AudioBuffer, read_wav, write_wav
from emotion_v2_dataset.exporters import export_personaplex, uniform_alignments
from emotion_v2_dataset.formats import MOSHI_SPEAKER, PERSONAPLEX_SOURCES
from emotion_v2_dataset.manifest import REQUIRED_KEYS, ManifestRow, read_jsonl


class PersonaPlexTests(unittest.TestCase):
    def test_sources_cover_nvidia_and_moshi(self) -> None:
        ids = {item["id"] for item in PERSONAPLEX_SOURCES}
        self.assertIn("nvidia-readme", ids)
        self.assertIn("moshi-finetune-readme", ids)
        self.assertIn("moshi-annotate", ids)
        self.assertIn("nvidia-issue-5", ids)

    def test_moshi_adapter_writes_stereo_manifest_and_main_speaker(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            run = Path(tmp)
            row = _row(run)
            result = export_personaplex([row], run, adapter="moshi", eval_fraction=0.0)
            train = read_jsonl(run / result.train_jsonl)
            evaluation = read_jsonl(run / result.eval_jsonl)
            self.assertEqual(evaluation, [])
            self.assertEqual(set(train[0]), {"path", "duration"})
            self.assertEqual(train[0]["path"], "stereo/neutral-01__dry-studio__snr-none.wav")
            wav = run / "personaplex" / train[0]["path"]
            samples, rate, channels = read_wav(wav)
            self.assertEqual(channels, 2)
            self.assertEqual(rate, 24000)
            self.assertEqual(int(np.max(np.abs(samples[:, 1]))), 0)
            self.assertGreater(float(np.max(np.abs(samples[:, 0]))), 0.01)
            duration = len(samples) / rate
            self.assertAlmostEqual(train[0]["duration"], duration, places=6)
            sidecar = json.loads(wav.with_suffix(".json").read_text(encoding="utf-8"))
            self.assertEqual(set(sidecar), {"alignments"})
            self.assertEqual(sidecar["alignments"][0][2], MOSHI_SPEAKER)
            self.assertAlmostEqual(sidecar["alignments"][-1][1][1], duration, places=6)
            prompts = read_jsonl(run / "personaplex" / "prompts.jsonl")
            self.assertNotIn("duration", prompts[0])

    def test_community_adapter_is_separate(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            run = Path(tmp)
            row = _row(run)
            export_personaplex([row], run, adapter="community")
            sidecar = json.loads(
                next((run / "personaplex" / "stereo").glob("*.json")).read_text(encoding="utf-8")
            )
            self.assertIn("text_prompt", sidecar)
            self.assertEqual(sidecar["alignments"][0][2], "SPEAKER_BROKER")

    def test_uniform_alignment_covers_the_words(self) -> None:
        rows = uniform_alignments("one two three", 1.5, MOSHI_SPEAKER)
        self.assertEqual([row[0] for row in rows], ["one", "two", "three"])
        self.assertEqual(rows[-1][1][1], 1.5)


def _row(run: Path) -> ManifestRow:
    audio = AudioBuffer(0.2 * np.sin(2 * np.pi * 216 * np.arange(24000) / 24000), 24000)
    relative = Path("audio/neutral/neutral-01__dry-studio__snr-none.wav")
    write_wav(run / relative, audio)
    return ManifestRow(
        audio_path=relative.as_posix(),
        text="one two three",
        emotion="Neutral",
        intensity=0.35,
        environment="dry studio",
        snr_db=None,
        voice_id="stub-sine",
        scores={"wer": None, "ecapa": None, "emotion": None},
        sha256="unused",
        sample_rate_hz=24000,
        pitch_standard_hz=432.0,
        pitch_centre_hz=216.0,
        render_backend="stub",
        synthetic_signal=True,
        scenario_id="neutral-01",
        ir_source="in-process unit impulse",
        ir_licence="CC0-1.0",
        noise_source="none",
        noise_licence="n/a",
        duration_s=1.0,
    )


class ManifestKeyTests(unittest.TestCase):
    def test_required_keys_match_the_brief(self) -> None:
        self.assertEqual(
            REQUIRED_KEYS,
            (
                "audio_path",
                "text",
                "emotion",
                "intensity",
                "environment",
                "snr_db",
                "voice_id",
                "scores",
                "sha256",
            ),
        )


if __name__ == "__main__":
    unittest.main()
