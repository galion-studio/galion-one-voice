# G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network
# Developed and distributed by Galion Labs
# Perpetual Open MIT License
# Copyright © 2026 Galion Labs. All rights reserved.
"""High-quality-only defaults. Synthetic tones and stub scorers only: no voice audio."""

from __future__ import annotations

import json
import math
import os
import subprocess
import sys
import tempfile
import types
import unittest
from pathlib import Path

import numpy as np

from emotion_v2_dataset.audio import AudioBuffer, dominant_hz, read_wav, write_wav
from emotion_v2_dataset.capture import (
    BRICKS,
    HIGH_QUALITY_CAPTURE,
    CaptureSpec,
    conform,
    count_clipped,
    integrated_lufs,
    load_capture_spec,
    resample,
    true_peak_dbtp,
)
from emotion_v2_dataset.compute import resolve_device
from emotion_v2_dataset.errors import QualityError
from emotion_v2_dataset.manifest import read_jsonl
from emotion_v2_dataset.paths import DATA_DIR, find_repo_root
from emotion_v2_dataset.pipeline import RunConfig, run_pipeline
from emotion_v2_dataset.quality import (
    GATE_MAX_WER,
    GATE_MIN_ECAPA,
    GATE_MIN_EMOTION,
    FunctionScorer,
    QualityThresholds,
    RenderedClip,
    ScoreCard,
    decide,
    high_quality_violations,
    score_clip,
    thresholds_from_toml,
)
from emotion_v2_dataset.taxonomy import CANONICAL_EMOTIONS

PACKAGE = Path(__file__).resolve().parents[1]
REPO = find_repo_root(PACKAGE)
RATE = 48_000


def tone(hz: float, seconds: float, amplitude: float, rate: int = RATE, phase: float = 0.0) -> np.ndarray:
    t = np.arange(int(rate * seconds), dtype=np.float64) / rate
    return amplitude * np.sin(2.0 * np.pi * hz * t + phase)


def passing(wer: float = 0.0, ecapa: float = 0.9, emotion: object = 0.95) -> list:
    return [
        FunctionScorer("wer", lambda _clip: wer),
        FunctionScorer("ecapa", lambda _clip: ecapa),
        FunctionScorer("emotion", (lambda _clip: emotion) if not callable(emotion) else emotion),
    ]


def clip(emotion: str = "Neutral") -> RenderedClip:
    return RenderedClip(AudioBuffer(tone(432.0, 0.1, 0.1), RATE), "line", emotion, "stub-sine", 0.5)


class EmotionListTests(unittest.TestCase):
    def test_emotion_list_comes_from_emotions_toml(self) -> None:
        import tomllib

        if REPO is None:
            self.skipTest("repo root not found")
        data = tomllib.loads((REPO / "emotion-v2" / "emotions.toml").read_text(encoding="utf-8"))
        self.assertEqual(CANONICAL_EMOTIONS, tuple(item["name"] for item in data["emotions"]))

    def test_a_fourteen_class_file_gives_fourteen_per_class_thresholds(self) -> None:
        from emotion_v2_dataset.taxonomy import load_config_emotions

        names = list(CANONICAL_EMOTIONS) + ["Proud", "Bored"]
        body = "".join(f'[[emotions]]\nname = "{n}"\n\n' for n in names)
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "emotions.toml"
            path.write_text(body, encoding="utf-8")
            loaded = load_config_emotions(path)
            self.assertEqual(len(loaded), len(CANONICAL_EMOTIONS) + 2)
            env = os.environ.copy()
            env["GALION_EMOTIONS_TOML"] = str(path)
            env["PYTHONPATH"] = str(PACKAGE) + os.pathsep + env.get("PYTHONPATH", "")
            code = (
                "from emotion_v2_dataset.quality import QualityThresholds, decide, ScoreCard;"
                "t=QualityThresholds();print(len(t.min_emotion_by_class));"
                "print(decide(ScoreCard(0.0,0.9,0.82),t,'Bored').reason)"
            )
            done = subprocess.run([sys.executable, "-c", code], env=env, capture_output=True, text=True, check=False)
            self.assertEqual(done.returncode, 0, done.stderr)
            lines = done.stdout.split("\n")
            self.assertEqual(lines[0], str(len(names)))
            self.assertIn("emotion Bored 0.8200 below 0.8300", lines[1])


class GateDefaultTests(unittest.TestCase):
    def test_defaults_are_the_rollout_gate(self) -> None:
        t = QualityThresholds()
        self.assertEqual((t.max_wer, t.min_ecapa, t.min_emotion), (0.018, 0.7057, 0.83))
        self.assertTrue(t.require_scorers)
        self.assertEqual(set(t.min_emotion_by_class), set(CANONICAL_EMOTIONS))
        self.assertEqual(len(t.min_emotion_by_class), len(CANONICAL_EMOTIONS))
        self.assertTrue(all(v == 0.83 for v in t.min_emotion_by_class.values()))
        self.assertEqual(high_quality_violations(t), [])

    def test_defaults_match_thresholds_toml(self) -> None:
        if REPO is None:
            self.skipTest("repo root not found")
        loaded = thresholds_from_toml(REPO / "tools" / "emotion-v2" / "thresholds.toml")
        self.assertEqual(loaded.max_wer, GATE_MAX_WER)
        self.assertEqual(loaded.min_ecapa, GATE_MIN_ECAPA)
        self.assertEqual(dict(loaded.min_emotion_by_class), dict(QualityThresholds().min_emotion_by_class))

    def test_capture_defaults_match_capture_spec_toml(self) -> None:
        if REPO is None:
            self.skipTest("repo root not found")
        spec = load_capture_spec(REPO / "docs" / "emotion-v2" / "capture-spec.toml")
        self.assertEqual(spec, HIGH_QUALITY_CAPTURE)
        self.assertEqual((spec.sample_rate_hz, spec.bit_depth, spec.target_lufs), (48000, 24, -20.0))
        self.assertEqual(spec.max_clipped_samples, 0)

    def test_each_missing_scorer_rejects(self) -> None:
        for name in ("wer", "ecapa", "emotion"):
            scorers = [s for s in passing() if s.name != name]
            card, _ = score_clip(clip(), scorers)
            decision = decide(card, QualityThresholds(), "Neutral")
            self.assertFalse(decision.accepted, name)
            self.assertIn(f"missing scorers: {name}", decision.reason)

    def test_gate_boundaries(self) -> None:
        t = QualityThresholds()
        ok = lambda **kw: decide(ScoreCard(**{"wer": 0.0, "ecapa": 0.9, "emotion": 0.9, **kw}), t, "Calm").accepted
        self.assertTrue(ok(wer=0.018))
        self.assertFalse(ok(wer=0.0181))
        self.assertFalse(ok(ecapa=0.7057))  # strictly greater than the baseline
        self.assertTrue(ok(ecapa=0.7058))
        self.assertTrue(ok(emotion=0.83))
        self.assertFalse(ok(emotion=0.8299))

    def test_emotion_threshold_is_per_class_for_all_twelve(self) -> None:
        by_class = {name: 0.83 + 0.01 * index for index, name in enumerate(CANONICAL_EMOTIONS)}
        t = QualityThresholds(min_emotion_by_class=by_class)
        self.assertEqual(high_quality_violations(t), [])
        for name, limit in by_class.items():
            at = decide(ScoreCard(0.0, 0.9, limit), t, name)
            below = decide(ScoreCard(0.0, 0.9, limit - 0.001), t, name)
            self.assertTrue(at.accepted, name)
            self.assertFalse(below.accepted, name)
            self.assertIn(f"emotion {name}", below.reason)

    def test_loosening_any_class_is_not_high_quality(self) -> None:
        for name in CANONICAL_EMOTIONS:
            by_class = dict(QualityThresholds().min_emotion_by_class)
            by_class[name] = 0.5
            problems = high_quality_violations(QualityThresholds(min_emotion_by_class=by_class))
            self.assertEqual(len(problems), 1, name)
            self.assertIn(name, problems[0])
        self.assertTrue(high_quality_violations(QualityThresholds(max_wer=0.35)))
        self.assertTrue(high_quality_violations(QualityThresholds(min_ecapa=0.5)))
        self.assertTrue(high_quality_violations(QualityThresholds(require_scorers=False)))

    def test_twelve_class_posterior_scorer_uses_the_clip_class(self) -> None:
        posterior = {name.lower(): 0.01 for name in CANONICAL_EMOTIONS}
        posterior["sad"] = 0.91
        scorer = FunctionScorer("emotion", lambda _clip: posterior)
        card, _ = score_clip(clip("Sad"), [scorer])
        self.assertAlmostEqual(card.emotion, 0.91)
        card, _ = score_clip(clip("Happy"), [scorer])
        self.assertAlmostEqual(card.emotion, 0.01)
        with self.assertRaises(QualityError):
            score_clip(clip("Sad"), [FunctionScorer("emotion", lambda _clip: {"happy": 0.9})])


class CaptureMeasureTests(unittest.TestCase):
    def test_full_scale_997_hz_sine_is_about_minus_3_lufs(self) -> None:
        value = integrated_lufs(tone(997.0, 3.0, 1.0), RATE)
        self.assertAlmostEqual(value, -3.01, delta=0.05)

    def test_bricks_are_the_pinned_open_tools(self) -> None:
        from importlib.metadata import version

        self.assertEqual(set(BRICKS), {"pyloudnorm", "samplerate"})
        self.assertTrue(all("MIT" in v for v in BRICKS.values()))
        self.assertEqual(version("pyloudnorm"), "0.2.0")
        self.assertEqual(version("samplerate"), "0.2.4")

    def test_true_peak_finds_the_intersample_peak(self) -> None:
        x = tone(RATE / 4.0, 0.1, 1.0, phase=math.pi / 4.0)
        self.assertLess(float(np.max(np.abs(x))), 0.72)
        self.assertAlmostEqual(true_peak_dbtp(x, 4, RATE), 0.0, delta=0.2)

    def test_resample_keeps_432_hz(self) -> None:
        up = resample(AudioBuffer(tone(432.0, 1.0, 0.3, rate=24000), 24000), RATE)
        self.assertEqual(up.sample_rate, RATE)
        self.assertLessEqual(abs(len(up.samples) - RATE), 2)
        self.assertAlmostEqual(dominant_hz(up.samples, RATE, fmin=300.0, fmax=600.0), 432.0, delta=1.5)

    def test_conform_levels_a_24k_tone_to_48k_minus_20_lufs(self) -> None:
        result = conform(AudioBuffer(tone(432.0, 2.0, 0.05, rate=24000), 24000), HIGH_QUALITY_CAPTURE)
        self.assertTrue(result.accepted, result.reason)
        self.assertEqual(result.audio.sample_rate, 48000)
        self.assertAlmostEqual(result.loudness_lufs, -20.0, delta=0.05)
        self.assertLessEqual(result.true_peak_dbtp, -1.0)
        self.assertEqual(result.clipped_samples, 0)

    def test_clipped_input_is_rejected(self) -> None:
        x = np.clip(tone(432.0, 1.0, 1.6), -1.0, 1.0)
        result = conform(AudioBuffer(x, RATE), HIGH_QUALITY_CAPTURE)
        self.assertFalse(result.accepted)
        self.assertIn("clipped", result.reason)
        self.assertGreater(count_clipped(x), 0)

    def test_silence_is_rejected(self) -> None:
        result = conform(AudioBuffer(np.zeros(RATE), RATE), HIGH_QUALITY_CAPTURE)
        self.assertFalse(result.accepted)
        self.assertIn("absolute gate", result.reason)

    def test_peaky_signal_that_cannot_reach_minus_20_lufs_is_rejected(self) -> None:
        x = tone(432.0, 2.0, 0.001)
        x[::4800] = 0.9  # sparse clicks: levelling to -20 LUFS pushes them over -1 dBTP
        result = conform(AudioBuffer(x, RATE), HIGH_QUALITY_CAPTURE)
        self.assertFalse(result.accepted)
        self.assertRegex(result.reason, "true peak|clipped")

    def test_no_levelling_reports_loudness_outside_tolerance(self) -> None:
        spec = CaptureSpec(normalize_loudness=False)
        result = conform(AudioBuffer(tone(432.0, 2.0, 0.5), RATE), spec)
        self.assertFalse(result.accepted)
        self.assertIn("LUFS outside", result.reason)

    def test_24_bit_wav_round_trip(self) -> None:
        x = tone(432.0, 0.5, 0.25)
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "tone.wav"
            write_wav(path, AudioBuffer(x, RATE), bit_depth=24)
            back, rate, channels = read_wav(path)
            self.assertEqual((rate, channels), (RATE, 1))
            self.assertEqual(path.stat().st_size, 44 + 3 * len(x))
            np.testing.assert_allclose(back, x, atol=2.0 / 8388607.0)


class PipelineHighQualityTests(unittest.TestCase):
    def _run(self, root: Path, **kwargs):
        base = dict(
            profile_path=DATA_DIR / "stub_profile.json",
            scenarios_path=DATA_DIR / "scenarios.json",
            output_root=root,
            run_id="hq",
            max_scenarios_per_emotion=1,
            environments=("dry studio", "cafe/babble", "phone/narrowband 8 kHz"),
            repo_root=REPO,
        )
        base.update(kwargs)
        return run_pipeline(RunConfig(**base))

    def test_default_run_without_scorers_keeps_nothing(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            result = self._run(Path(tmp), emotions=("Neutral", "Angry"))
            self.assertEqual(result.rows, [])
            self.assertEqual(len(result.rejected), 2)
            self.assertTrue(all("missing scorers" in r["reason"] for r in result.rejected))
            self.assertEqual(result.record["quality_mode"], "high-quality")
            self.assertFalse((result.run_dir / "audio").exists())

    def test_all_twelve_emotions_meet_the_capture_spec(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            result = self._run(Path(tmp), scorers=passing())
            self.assertEqual(result.record["counts"]["rendered"], len(CANONICAL_EMOTIONS))
            self.assertEqual({r.emotion for r in result.rows}, set(CANONICAL_EMOTIONS))
            self.assertEqual(
                len(result.rows) + result.record["counts"]["rejected_capture"], 3 * len(CANONICAL_EMOTIONS)
            )
            self.assertGreater(len(result.rows), 0)
            for row in read_jsonl(result.manifest_path):
                self.assertEqual(row["sample_rate_hz"], 48000)
                self.assertEqual(row["bit_depth"], 24)
                self.assertEqual(row["quality_mode"], "high-quality")
                self.assertLessEqual(abs(row["loudness_lufs"] + 20.0), 2.0)
                self.assertLessEqual(row["true_peak_dbtp"], -1.0)
                samples, rate, channels = read_wav(result.run_dir / row["audio_path"])
                self.assertEqual((rate, channels), (48000, 1))
                self.assertEqual(count_clipped(samples), 0)
                measured = integrated_lufs(samples, rate)
                self.assertLessEqual(abs(measured + 20.0), 2.0)
            for item in result.rejected:
                self.assertTrue(item["reason"].startswith("capture:"), item["reason"])
            record = json.loads((result.run_dir / "run.json").read_text(encoding="utf-8"))
            self.assertEqual(record["capture_spec"]["sample_rate_hz"], 48000)
            self.assertEqual(len(record["thresholds"]["min_emotion_by_class"]), len(CANONICAL_EMOTIONS))

    def test_per_class_emotion_failure_only_drops_that_class(self) -> None:
        def emotion(c: RenderedClip) -> float:
            return 0.80 if c.emotion == "Angry" else 0.90

        with tempfile.TemporaryDirectory() as tmp:
            result = self._run(Path(tmp), scorers=passing(emotion=emotion), environments=("dry studio",))
            kept = {r.emotion for r in result.rows}
            self.assertNotIn("Angry", kept)
            self.assertEqual(kept, set(CANONICAL_EMOTIONS) - {"Angry"})
            angry = [r for r in result.rejected if r["emotion"] == "Angry"]
            self.assertEqual(len(angry), 1)
            self.assertIn("emotion Angry 0.8000 below 0.8300", angry[0]["reason"])

    def test_scorer_module_is_pluggable(self) -> None:
        seen: dict = {}

        def build_scorers(config: dict) -> list:
            seen.update(config)
            posterior = {name: 0.9 for name in CANONICAL_EMOTIONS}
            return passing(emotion=lambda _c: posterior)

        fake = types.SimpleNamespace(build_scorers=build_scorers)

        def importer(name: str):
            if name == "emotion_scorer_n":
                return fake
            raise ImportError(name)

        with tempfile.TemporaryDirectory() as tmp:
            result = self._run(
                Path(tmp),
                emotions=("Tender",),
                environments=("dry studio",),
                importer=importer,
                scorer_modules=("emotion_scorer_n",),
            )
            self.assertEqual(len(result.rows), 1)
            self.assertEqual(result.record["plugins"]["ci"], "emotion_scorer_n")
            self.assertEqual(seen["device"], "cpu")
            self.assertGreaterEqual(seen["num_threads"], 1)
            with self.assertRaises(QualityError):
                self._run(Path(tmp), run_id="missing", importer=importer, scorer_modules=("nope",))

    def test_loose_thresholds_are_refused_in_high_quality_mode(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaises(QualityError):
                self._run(Path(tmp), thresholds=QualityThresholds(max_wer=0.35), scorers=passing())
            with self.assertRaises(QualityError):
                self._run(Path(tmp), capture=CaptureSpec(sample_rate_hz=24000), scorers=passing())
            self.assertFalse((Path(tmp) / "stub-sine").exists())

    def test_synthetic_opt_out_is_refused_for_real_profiles(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            profile = Path(tmp) / "real.json"
            profile.write_text(
                json.dumps(
                    {
                        "voice_id": "studio-fixture",
                        "kind": "real",
                        "pitch_standard_hz": 432,
                        "pitch_centre_hz": 180,
                        "references": {e: f"references/{e}" for e in CANONICAL_EMOTIONS},
                    }
                ),
                encoding="utf-8",
            )
            consent = Path(tmp) / "consent.json"
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
            from datetime import date

            with self.assertRaises(QualityError):
                self._run(
                    Path(tmp) / "out",
                    profile_path=profile,
                    consent_path=consent,
                    today=date(2026, 11, 7),
                    allow_unscored_synthetic_test=True,
                )
            self.assertFalse((Path(tmp) / "out").exists())


class ComputeDeviceTests(unittest.TestCase):
    def test_production_default_is_cpu_with_all_threads(self) -> None:
        def importer(_name: str):
            raise AssertionError("cpu mode must not probe for a GPU")

        choice = resolve_device(importer=importer)
        self.assertEqual(choice.device, "cpu")
        self.assertEqual(choice.num_threads, os.process_cpu_count() if hasattr(os, "process_cpu_count") else os.cpu_count())

    def test_auto_picks_cuda_when_available_and_falls_back(self) -> None:
        gpu = types.SimpleNamespace(cuda=types.SimpleNamespace(is_available=lambda: True, get_device_name=lambda _i: "Fake GPU"))
        none = types.SimpleNamespace(cuda=types.SimpleNamespace(is_available=lambda: False))
        self.assertEqual(resolve_device("auto", importer=lambda _n: gpu).device, "cuda")
        fallback = resolve_device("auto", importer=lambda _n: none, threads=16)
        self.assertEqual((fallback.device, fallback.num_threads), ("cpu", 16))

        def missing(_n: str):
            raise ImportError("torch")

        self.assertEqual(resolve_device("auto", importer=missing).device, "cpu")
        with self.assertRaises(QualityError):
            resolve_device("cuda", importer=missing)


class CliHighQualityTests(unittest.TestCase):
    def _cli(self, tmp: str, *extra: str) -> subprocess.CompletedProcess:
        env = os.environ.copy()
        env["PYTHONPATH"] = str(PACKAGE) + os.pathsep + env.get("PYTHONPATH", "")
        return subprocess.run(
            [sys.executable, "-m", "emotion_v2_dataset", "run", "--run-id", "cli-hq", "--output-root", tmp,
             "--emotions", "Calm", "--environments", "dry studio", "--max-scenarios-per-emotion", "1", *extra],
            cwd=str(PACKAGE), env=env, capture_output=True, text=True, check=False,
        )

    def test_cli_default_rejects_unscored_clips(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            done = self._cli(tmp)
            self.assertEqual(done.returncode, 0, done.stderr)
            self.assertIn("wrote 0 rows", done.stdout)
            rejected = read_jsonl(Path(tmp) / "stub-sine" / "cli-hq" / "rejected.jsonl")
            self.assertEqual(len(rejected), 1)
            self.assertIn("missing scorers", rejected[0]["reason"])

    def test_cli_refuses_loose_thresholds_and_opt_out_on_chatterbox(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            done = self._cli(tmp, "--max-wer", "0.35")
            self.assertEqual(done.returncode, 2)
            self.assertIn("high-quality-only", done.stderr)
            done = self._cli(tmp, "--allow-unscored-synthetic-test", "--backend", "chatterbox",
                             "--consent", str(DATA_DIR / "consent.example.json"), "--today", "2026-11-07")
            self.assertEqual(done.returncode, 2)
            self.assertIn("synthetic stub tests only", done.stderr)


if __name__ == "__main__":
    unittest.main()
