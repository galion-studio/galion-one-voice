# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""End-to-end stub run. Output stays in a temporary directory. No real voice."""

from __future__ import annotations

import hashlib
import json
import os
import subprocess
import sys
import tempfile
import unittest
from datetime import date
from pathlib import Path

import numpy as np

from emotion_v2_dataset.audio import read_wav, sha256_file
from emotion_v2_dataset.errors import ConsentError, PipelineError, ProfileError
from emotion_v2_dataset.manifest import REQUIRED_KEYS, read_jsonl
from emotion_v2_dataset.paths import DATA_DIR, default_output_root, find_repo_root
from emotion_v2_dataset.pipeline import RunConfig, run_pipeline
from emotion_v2_dataset.plugins import load_scorer_plugins, resolve_references, taxonomy_source
from emotion_v2_dataset.profile import load_profile
from emotion_v2_dataset.quality import QualityThresholds
from emotion_v2_dataset.scenarios import load_scenarios
from emotion_v2_dataset.taxonomy import CANONICAL_EMOTIONS

PACKAGE = Path(__file__).resolve().parents[1]
REPO = find_repo_root(PACKAGE)
TODAY = date(2026, 11, 7)
MARKER = b"REAL-VOICE-MARKER-NOT-AUDIO"


class EndToEndTests(unittest.TestCase):
    def test_stub_run_writes_manifest_and_personaplex(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            first = _run(Path(tmp), "run-a")
            second = _run(Path(tmp), "run-b")
            self.assertEqual(
                [row.sha256 for row in first.rows],
                [row.sha256 for row in second.rows],
            )
            self.assertEqual(len(first.rows), 6)
            manifest = read_jsonl(first.manifest_path)
            self.assertEqual(len(manifest), 6)
            for row in manifest:
                for key in REQUIRED_KEYS:
                    self.assertIn(key, row)
                audio = first.run_dir / row["audio_path"]
                self.assertEqual(row["sha256"], sha256_file(audio))
                self.assertEqual(len(row["sha256"]), 64)
                self.assertEqual(row["voice_id"], "stub-sine")
                self.assertEqual(row["pitch_standard_hz"], 432)
                self.assertEqual(row["scores"], {"wer": None, "ecapa": None, "emotion": None})
                self.assertTrue(row["synthetic_signal"])
                self.assertEqual(row["render_backend"], "stub")
                self.assertIn("CC0-1.0", row["ir_licence"])
            environments = {row["environment"] for row in manifest}
            self.assertEqual(
                environments,
                {"dry studio", "cafe/babble", "phone/narrowband 8 kHz"},
            )
            phone = next(row for row in manifest if row["telephony_hz"] == 8000)
            _samples, rate, channels = read_wav(first.run_dir / phone["audio_path"])
            self.assertEqual(channels, 1)
            self.assertEqual(rate, 8000)
            dry = next(row for row in manifest if row["environment"] == "dry studio")
            _samples, rate, _channels = read_wav(first.run_dir / dry["audio_path"])
            self.assertEqual(rate, 24000)
            train = read_jsonl(first.run_dir / "personaplex" / "train.jsonl")
            self.assertEqual(len(train), 6)
            self.assertEqual(read_jsonl(first.run_dir / "personaplex" / "eval.jsonl"), [])
            record = json.loads((first.run_dir / "run.json").read_text(encoding="utf-8"))
            self.assertEqual(record["plugins"]["taxonomy"], "local")
            self.assertEqual(record["plugins"]["capture"], "local")
            self.assertFalse(record["consent_checked"])
            self.assertIn("moshi-finetune", " ".join(record["personaplex"]["assumptions"]))
            self.assertNotIn(MARKER, (first.run_dir / dry["audio_path"]).read_bytes())

    def test_reference_bytes_are_not_read_into_the_dataset(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            marker = root / "reference.wav"
            marker.write_bytes(MARKER)
            profile = root / "profile.json"
            profile.write_text(
                json.dumps(
                    {
                        "voice_id": "stub-sine",
                        "kind": "synthetic",
                        "pitch_standard_hz": 432,
                        "pitch_centre_hz": 216,
                        "references": {"Neutral": str(marker)},
                    }
                ),
                encoding="utf-8",
            )
            result = run_pipeline(
                RunConfig(
                    profile_path=profile,
                    scenarios_path=DATA_DIR / "scenarios.json",
                    output_root=root / "out",
                    run_id="refs",
                    emotions=("Neutral",),
                    environments=("dry studio",),
                    max_scenarios_per_emotion=1,
                    repo_root=REPO,
                )
            )
            blob = b"".join(path.read_bytes() for path in result.run_dir.rglob("*") if path.is_file())
            self.assertNotIn(MARKER, blob)
            self.assertEqual(marker.read_bytes(), MARKER)

    def test_real_voice_without_consent_writes_nothing(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            profile = _real_profile(root)
            output = root / "out"
            with self.assertRaises(ConsentError):
                run_pipeline(
                    RunConfig(
                        profile_path=profile,
                        scenarios_path=DATA_DIR / "scenarios.json",
                        output_root=output,
                        run_id="real",
                        emotions=("Neutral",),
                        environments=("dry studio",),
                        max_scenarios_per_emotion=1,
                        today=TODAY,
                        repo_root=REPO,
                    )
                )
            self.assertFalse(output.exists())

    def test_real_voice_with_consent_stays_on_the_stub(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            profile = _real_profile(root)
            consent = root / "consent.json"
            consent.write_text(
                json.dumps(
                    {
                        "voice_id": "studio-fixture",
                        "owner": "synthetic fixture",
                        "consent_scope": ["training-data-generation"],
                        "signed_date": "2026-11-01",
                    }
                ),
                encoding="utf-8",
            )
            result = run_pipeline(
                RunConfig(
                    profile_path=profile,
                    scenarios_path=DATA_DIR / "scenarios.json",
                    output_root=root / "out",
                    run_id="real-ok",
                    consent_path=consent,
                    emotions=("Neutral",),
                    environments=("dry studio",),
                    max_scenarios_per_emotion=1,
                    today=TODAY,
                    repo_root=REPO,
                )
            )
            self.assertTrue(result.record["consent_checked"])
            self.assertTrue(result.rows[0].synthetic_signal)
            self.assertEqual(result.rows[0].render_backend, "stub")

    def test_quality_rejection_skips_augmentation(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)

            class RejectWer:
                name = "wer"

                def score(self, _clip: object) -> float:
                    return 0.99

            result = run_pipeline(
                RunConfig(
                    profile_path=DATA_DIR / "stub_profile.json",
                    scenarios_path=DATA_DIR / "scenarios.json",
                    output_root=root / "out",
                    run_id="reject",
                    emotions=("Calm",),
                    environments=("car", "street"),
                    max_scenarios_per_emotion=1,
                    scorers=[RejectWer()],
                    thresholds=QualityThresholds(max_wer=0.35),
                    repo_root=REPO,
                )
            )
            self.assertEqual(result.rows, [])
            self.assertEqual(len(result.rejected), 1)
            self.assertEqual(read_jsonl(result.run_dir / "personaplex" / "train.jsonl"), [])
            self.assertFalse((result.run_dir / "audio").exists())

    def test_output_inside_the_repo_outside_datasets_is_refused(self) -> None:
        if REPO is None:
            self.skipTest("repo root not found")
        with self.assertRaises(PipelineError):
            run_pipeline(
                RunConfig(
                    profile_path=DATA_DIR / "stub_profile.json",
                    scenarios_path=DATA_DIR / "scenarios.json",
                    output_root=REPO / "tools" / "emotion-v2" / "dataset",
                    run_id="illegal",
                    emotions=("Neutral",),
                    environments=("dry studio",),
                    max_scenarios_per_emotion=1,
                    repo_root=REPO,
                )
            )
        self.assertFalse((REPO / "tools" / "emotion-v2" / "dataset" / "stub-sine").exists())

    def test_cli_stub_run(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            env = os.environ.copy()
            env["PYTHONPATH"] = str(PACKAGE) + os.pathsep + env.get("PYTHONPATH", "")
            completed = subprocess.run(
                [
                    sys.executable,
                    "-m",
                    "emotion_v2_dataset",
                    "run",
                    "--run-id",
                    "cli",
                    "--output-root",
                    tmp,
                    "--emotions",
                    "Sad",
                    "--environments",
                    "dry studio",
                    "--max-scenarios-per-emotion",
                    "1",
                ],
                cwd=str(PACKAGE),
                env=env,
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(completed.returncode, 0, completed.stderr)
            manifest = Path(tmp) / "stub-sine" / "cli" / "manifest.jsonl"
            self.assertTrue(manifest.is_file())
            self.assertEqual(len(read_jsonl(manifest)), 1)

    def test_cli_chatterbox_without_engine_refuses(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            env = os.environ.copy()
            env["PYTHONPATH"] = str(PACKAGE) + os.pathsep + env.get("PYTHONPATH", "")
            completed = subprocess.run(
                [
                    sys.executable,
                    "-m",
                    "emotion_v2_dataset",
                    "run",
                    "--backend",
                    "chatterbox",
                    "--consent",
                    str(DATA_DIR / "consent.example.json"),
                    "--run-id",
                    "no-engine",
                    "--output-root",
                    tmp,
                    "--today",
                    "2026-11-07",
                    "--emotions",
                    "Neutral",
                    "--environments",
                    "dry studio",
                    "--max-scenarios-per-emotion",
                    "1",
                ],
                cwd=str(PACKAGE),
                env=env,
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(completed.returncode, 2, completed.stderr)
            self.assertIn("does not", completed.stderr)
            self.assertFalse((Path(tmp) / "stub-sine").exists())


class BankAndPluginTests(unittest.TestCase):
    def test_bank_covers_twelve_emotions_with_two_lines(self) -> None:
        bank = load_scenarios(DATA_DIR / "scenarios.json")
        self.assertEqual(tuple(bank), CANONICAL_EMOTIONS)
        for emotion, lines in bank.items():
            self.assertGreaterEqual(len(lines), 2, emotion)
            for line in lines:
                self.assertTrue(line.text)
                self.assertGreaterEqual(line.intensity, 0.0)
                self.assertLessEqual(line.intensity, 1.0)
        profile = load_profile(DATA_DIR / "stub_profile.json")
        self.assertEqual(profile.kind, "synthetic")
        self.assertEqual(profile.pitch_standard_hz, 432)

    def test_nominal_matrix_size(self) -> None:
        bank = load_scenarios(DATA_DIR / "scenarios.json")
        lines = sum(len(items) for items in bank.values())
        self.assertEqual(lines * len(CANONICAL_EMOTIONS) // 12, lines)
        self.assertEqual(lines * 8, 12 * 2 * 8)

    def test_gitignore_covers_the_default_output(self) -> None:
        if REPO is None:
            self.skipTest("repo root not found")
        text = (REPO / ".gitignore").read_text(encoding="utf-8")
        self.assertIn("datasets/emotion-v2/", text)
        root = default_output_root(REPO)
        self.assertEqual(root, REPO / "datasets" / "emotion-v2")

    def test_package_does_not_download_or_shell_out(self) -> None:
        source = "\n".join(
            path.read_text(encoding="utf-8")
            for path in (PACKAGE / "emotion_v2_dataset").rglob("*.py")
        )
        for banned in (
            "urlretrieve",
            "urlopen",
            "huggingface_hub",
            "import torch",
            "import chatterbox",
            "chatterbox_worker",
            "requests.get",
            "subprocess",
            "os.system",
        ):
            self.assertNotIn(banned, source)

    def test_missing_plugins_stay_local_and_a_capture_plugin_can_fill_refs(self) -> None:
        emotions, source = taxonomy_source(lambda _name: (_ for _ in ()).throw(ImportError("no")))
        self.assertEqual(source, "local")
        self.assertEqual(emotions, CANONICAL_EMOTIONS)

        def importer(name: str) -> object:
            if name == "emotion_v2_taxonomy":
                return _TaxonomyMismatch()
            if name == "emotion_v2_capture":
                return _Capture()
            raise ImportError(name)

        _emotions, source = taxonomy_source(importer)
        self.assertEqual(source, "local-mismatch")
        refs, capture = resolve_references("studio-fixture", {}, importer)
        self.assertEqual(capture, "emotion_v2_capture")
        self.assertEqual(set(refs), set(CANONICAL_EMOTIONS))
        scorers, ci = load_scorer_plugins(lambda _name: (_ for _ in ()).throw(ImportError("no")))
        self.assertEqual(scorers, [])
        self.assertEqual(ci, "local")

    def test_real_profile_without_reference_dirs_refuses(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "profile.json"
            path.write_text(
                json.dumps(
                    {
                        "voice_id": "studio-fixture",
                        "kind": "real",
                        "pitch_standard_hz": 432,
                        "pitch_centre_hz": 180,
                        "references": {},
                    }
                ),
                encoding="utf-8",
            )
            with self.assertRaises(ProfileError):
                run_pipeline(
                    RunConfig(
                        profile_path=path,
                        scenarios_path=DATA_DIR / "scenarios.json",
                        output_root=Path(tmp) / "out",
                        run_id="missing-refs",
                        consent_path=_write_consent(Path(tmp)),
                        today=TODAY,
                        emotions=("Neutral",),
                        max_scenarios_per_emotion=1,
                        repo_root=REPO,
                    )
                )


def _run(root: Path, run_id: str):
    return run_pipeline(
        RunConfig(
            profile_path=DATA_DIR / "stub_profile.json",
            scenarios_path=DATA_DIR / "scenarios.json",
            output_root=root,
            run_id=run_id,
            emotions=("Neutral", "Happy"),
            environments=("dry studio", "cafe/babble", "phone/narrowband 8 kHz"),
            max_scenarios_per_emotion=1,
            repo_root=REPO,
        )
    )


def _real_profile(root: Path) -> Path:
    references = {emotion: f"references/{emotion}" for emotion in CANONICAL_EMOTIONS}
    path = root / "real.json"
    path.write_text(
        json.dumps(
            {
                "voice_id": "studio-fixture",
                "kind": "real",
                "pitch_standard_hz": 432,
                "pitch_centre_hz": 180,
                "references": references,
            }
        ),
        encoding="utf-8",
    )
    return path


def _write_consent(root: Path) -> Path:
    path = root / "consent.json"
    path.write_text(
        json.dumps(
            {
                "voice_id": "studio-fixture",
                "owner": "synthetic fixture",
                "consent_scope": ["training-data-generation"],
                "signed_date": "2026-11-01",
            }
        ),
        encoding="utf-8",
    )
    return path


class _TaxonomyMismatch:
    def load_taxonomy(self) -> list[str]:
        return ["Neutral"]


class _Capture:
    def resolve_references(self, voice_id: str) -> dict[str, str]:
        del voice_id
        return {emotion: f"captured/{emotion}" for emotion in CANONICAL_EMOTIONS}


class HashStabilityTests(unittest.TestCase):
    def test_sha256_helper_matches_hashlib(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "note.txt"
            path.write_bytes(b"abc")
            self.assertEqual(sha256_file(path), hashlib.sha256(b"abc").hexdigest())


if __name__ == "__main__":
    unittest.main()
