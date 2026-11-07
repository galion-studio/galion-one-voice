# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Score the twelve emotions on synthetic tones and exercise the CLI.

No committed audio. Renders and references are sines written into a temporary
directory and deleted with it.
"""

from __future__ import annotations

import json
import os
import stat
import subprocess
import sys
import tempfile
import unittest
import urllib.request
from pathlib import Path

import support

from harness.audio import synthesize_tone, write_wav
from harness.backends import Analysis, StubBackend
from harness.config import load_thresholds
from harness.emotions import EMOTIONS, STUB_FREQ_HZ, display_name
from harness.report import render_markdown, write_reports
from harness.score import score_all, score_emotion

REPO = support.REPO
SCENARIOS = support.EMOTION_V2 / "scenarios"


class ScriptedBackend:
    """A pluggable stand-in. The sample rate selects the posterior."""

    name = "scripted"

    def labels(self) -> tuple[str, ...]:
        return ("left", "right")

    def analyse(self, samples: list[float], sample_rate: int) -> Analysis:
        del samples
        if sample_rate == 16000:
            posterior = (0.95, 0.05)
            embedding = (1.0, 0.0)
        elif sample_rate == 8000:
            posterior = (0.05, 0.95)
            embedding = (0.0, 1.0)
        else:
            raise ValueError(f"unexpected rate {sample_rate}")
        return Analysis(self.labels(), posterior, embedding)


class SplitBackend:
    """Posterior matches across rates. The embedding does not."""

    name = "split"

    def labels(self) -> tuple[str, ...]:
        return ("only",)

    def analyse(self, samples: list[float], sample_rate: int) -> Analysis:
        del samples
        embedding = (1.0, 0.0) if sample_rate == 16000 else (0.0, 1.0)
        return Analysis(self.labels(), (1.0,), embedding)


def _write_tree(root: Path, freq_for, sample_rate: int = 16000) -> None:
    for emotion in EMOTIONS:
        write_wav(
            root / emotion / "line.wav",
            synthesize_tone(freq_for(emotion), sample_rate=sample_rate, seconds=0.25),
            sample_rate,
        )


def _scorer(path: Path) -> None:
    path.write_text(
        "#!/usr/bin/env python3\n"
        "import json, os, sys\n"
        "args = sys.argv\n"
        "out = args[args.index('--out') + 1]\n"
        "log = os.environ['SCORER_LOG']\n"
        "with open(log, 'a', encoding='utf-8') as handle:\n"
        "    handle.write(json.dumps(args) + '\\n')\n"
        "if '--text-dir' in args:\n"
        "    value = float(os.environ.get('WER_VALUE', '0.01'))\n"
        "    json.dump({'summary': {'mean': value}}, open(out, 'w', encoding='utf-8'))\n"
        "else:\n"
        "    value = float(os.environ.get('ECAPA_VALUE', '0.80'))\n"
        "    json.dump({'mean': value}, open(out, 'w', encoding='utf-8'))\n",
        encoding="utf-8",
    )
    path.chmod(path.stat().st_mode | stat.S_IEXEC)


class ScoreTest(unittest.TestCase):
    def test_matching_tones_pass_and_write_reports(self) -> None:
        thresholds = load_thresholds()
        with tempfile.TemporaryDirectory(prefix="emotion-v2-score-") as tmp:
            root = Path(tmp)
            renders = root / "renders"
            references = root / "references"
            _write_tree(renders, lambda emotion: STUB_FREQ_HZ[emotion])
            _write_tree(references, lambda emotion: STUB_FREQ_HZ[emotion])
            summary = score_all(
                renders,
                references,
                SCENARIOS,
                thresholds,
                StubBackend(),
            )
            self.assertTrue(summary.passed)
            self.assertEqual(summary.tuning_hz, 432)
            self.assertEqual(summary.backend, "stub")
            self.assertEqual([row.emotion for row in summary.scores], list(EMOTIONS))
            for row in summary.scores:
                self.assertGreaterEqual(row.posterior_vs_reference or 0.0, 0.83)
                self.assertGreaterEqual(row.embedding_similarity or 0.0, 0.83)
                self.assertTrue(row.emotion_pass)
                self.assertEqual(row.wer.status, "absent")
                self.assertIsNone(row.wer.passed)
                self.assertEqual(row.ecapa.status, "absent")
                self.assertEqual(len(row.scenarios), 2)
                self.assertIsNone(row.error)
            out = root / "out"
            write_reports(out, summary)
            for emotion in EMOTIONS:
                payload = json.loads((out / f"{emotion}.json").read_text(encoding="utf-8"))
                self.assertEqual(payload["schema"], "galion-emotion-v2-score/1")
                self.assertEqual(payload["tuning_hz"], 432)
                self.assertEqual(payload["target"], 0.83)
                self.assertTrue(payload["pass"])
                self.assertEqual(payload["display"], display_name(emotion))
            table = (out / "summary.md").read_text(encoding="utf-8")
            self.assertIn("432 Hz", table)
            self.assertIn("Target 0.83 on every emotion.", table)
            for emotion in EMOTIONS:
                self.assertIn(display_name(emotion), table)
            self.assertEqual(table.count("| pass |"), 12)
            summary_json = json.loads((out / "summary.json").read_text(encoding="utf-8"))
            self.assertTrue(summary_json["pass"])
            self.assertEqual(len(summary_json["emotions"]), 12)

    def test_divergent_reference_fails_that_emotion_only(self) -> None:
        with tempfile.TemporaryDirectory(prefix="emotion-v2-div-") as tmp:
            root = Path(tmp)
            renders = root / "renders"
            references = root / "references"
            _write_tree(renders, lambda emotion: STUB_FREQ_HZ[emotion])
            _write_tree(
                references,
                lambda emotion: STUB_FREQ_HZ["disgusted"] if emotion == "happy" else STUB_FREQ_HZ[emotion],
            )
            summary = score_all(renders, references, SCENARIOS, load_thresholds(), StubBackend())
            by_name = {row.emotion: row for row in summary.scores}
            self.assertFalse(summary.passed)
            self.assertFalse(by_name["happy"].emotion_pass)
            self.assertLess(by_name["happy"].posterior_vs_reference or 1.0, 0.83)
            self.assertTrue(all(by_name[name].passed for name in EMOTIONS if name != "happy"))
            table = render_markdown(summary)
            self.assertIn("| Happy |", table)
            self.assertIn("| fail |", table)

    def test_scripted_backend_is_pluggable(self) -> None:
        with tempfile.TemporaryDirectory(prefix="emotion-v2-scripted-") as tmp:
            root = Path(tmp)
            render = root / "render"
            reference = root / "reference"
            scenario = root / "scenarios"
            scenario.mkdir()
            (scenario / "s01.txt").write_text("Hello from the harness.\n", encoding="utf-8")
            write_wav(render / "a.wav", synthesize_tone(440.0, sample_rate=16000, seconds=0.05), 16000)
            write_wav(reference / "ref.wav", synthesize_tone(440.0, sample_rate=16000, seconds=0.05), 16000)
            passed = score_emotion(
                "neutral",
                render,
                reference,
                scenario,
                load_thresholds(),
                ScriptedBackend(),
            )
            self.assertTrue(passed.passed)
            self.assertAlmostEqual(passed.posterior_vs_reference or 0.0, 1.0, places=6)
            write_wav(reference / "ref.wav", synthesize_tone(440.0, sample_rate=8000, seconds=0.05), 8000)
            failed = score_emotion(
                "neutral",
                render,
                reference,
                scenario,
                load_thresholds(),
                ScriptedBackend(),
            )
            self.assertFalse(failed.passed)
            self.assertIsNotNone(failed.posterior_vs_reference)
            self.assertLess(failed.posterior_vs_reference or 0.0, 0.83)
            self.assertIsNotNone(failed.embedding_similarity)
            self.assertAlmostEqual(failed.embedding_similarity or 0.0, 0.0, places=6)

    def test_missing_reference_fails_without_network(self) -> None:
        def refuse(*_args, **_kwargs):
            raise AssertionError("harness tried to use the network")

        original = urllib.request.urlopen
        urllib.request.urlopen = refuse
        try:
            with tempfile.TemporaryDirectory(prefix="emotion-v2-missing-") as tmp:
                root = Path(tmp)
                renders = root / "renders"
                _write_tree(renders, lambda emotion: STUB_FREQ_HZ[emotion])
                summary = score_all(
                    renders,
                    root / "references",
                    SCENARIOS,
                    load_thresholds(),
                    StubBackend(),
                )
                self.assertFalse(summary.passed)
                self.assertTrue(all(row.error for row in summary.scores))
        finally:
            urllib.request.urlopen = original

    def test_require_guards_fails_when_scorers_are_absent(self) -> None:
        saved = {key: os.environ.get(key) for key in ("GALION_WER", "GALION_ECAPA")}
        os.environ.pop("GALION_WER", None)
        os.environ.pop("GALION_ECAPA", None)
        try:
            with tempfile.TemporaryDirectory(prefix="emotion-v2-guards-") as tmp:
                root = Path(tmp)
                renders = root / "renders" / "neutral"
                references = root / "references" / "neutral"
                write_wav(renders / "line.wav", synthesize_tone(STUB_FREQ_HZ["neutral"]), 16000)
                write_wav(references / "line.wav", synthesize_tone(STUB_FREQ_HZ["neutral"]), 16000)
                row = score_emotion(
                    "neutral",
                    renders,
                    references,
                    SCENARIOS / "neutral",
                    load_thresholds(),
                    StubBackend(),
                    require_guards=True,
                )
                self.assertTrue(row.emotion_pass)
                self.assertEqual(row.wer.status, "absent")
                self.assertFalse(row.passed)
        finally:
            for key, value in saved.items():
                if value is None:
                    os.environ.pop(key, None)
                else:
                    os.environ[key] = value

    def test_existing_scorers_are_called_and_gated(self) -> None:
        with tempfile.TemporaryDirectory(prefix="emotion-v2-bins-") as tmp:
            root = Path(tmp)
            log = root / "calls.jsonl"
            binary = root / "scorer.py"
            _scorer(binary)
            renders = root / "renders"
            references = root / "references"
            write_wav(renders / "s01.wav", synthesize_tone(STUB_FREQ_HZ["tender"]), 16000)
            write_wav(references / "ref.wav", synthesize_tone(STUB_FREQ_HZ["tender"]), 16000)
            speaker = root / "speaker.wav"
            write_wav(speaker, synthesize_tone(210.0, seconds=0.05), 16000)
            os.environ["SCORER_LOG"] = str(log)
            os.environ["WER_VALUE"] = "0.018"
            os.environ["ECAPA_VALUE"] = "0.80"
            try:
                ok = score_emotion(
                    "tender",
                    renders,
                    references,
                    SCENARIOS / "tender",
                    load_thresholds(),
                    StubBackend(),
                    speaker_ref=speaker,
                    wer_bin=binary,
                    ecapa_bin=binary,
                )
            finally:
                os.environ.pop("SCORER_LOG", None)
                os.environ.pop("WER_VALUE", None)
                os.environ.pop("ECAPA_VALUE", None)
            self.assertTrue(ok.passed)
            self.assertTrue(ok.wer.passed)
            self.assertTrue(ok.ecapa.passed)
            self.assertAlmostEqual(ok.wer.value or 0.0, 0.018)
            calls = [json.loads(line) for line in log.read_text(encoding="utf-8").splitlines()]
            wer_call = next(call for call in calls if "--text-dir" in call)
            ecapa_call = next(call for call in calls if "--ref" in call)
            self.assertIn("--jobs", wer_call)
            self.assertIn(str(SCENARIOS / "tender"), wer_call)
            self.assertIn(str(renders / "s01.wav"), wer_call)
            self.assertIn("--threads", ecapa_call)
            self.assertEqual(ecapa_call[ecapa_call.index("--ref") + 1], str(speaker))
            os.environ["SCORER_LOG"] = str(log)
            os.environ["WER_VALUE"] = "0.02"
            os.environ["ECAPA_VALUE"] = "0.7057"
            try:
                bad = score_emotion(
                    "tender",
                    renders,
                    references,
                    SCENARIOS / "tender",
                    load_thresholds(),
                    StubBackend(),
                    speaker_ref=speaker,
                    wer_bin=binary,
                    ecapa_bin=binary,
                )
            finally:
                os.environ.pop("SCORER_LOG", None)
                os.environ.pop("WER_VALUE", None)
                os.environ.pop("ECAPA_VALUE", None)
            self.assertFalse(bad.wer.passed)
            self.assertFalse(bad.ecapa.passed)
            self.assertFalse(bad.passed)
            self.assertTrue(bad.emotion_pass)

    def test_embedding_min_gates_without_failing_the_posterior(self) -> None:
        base = load_thresholds()
        gated = type(base)(
            schema=base.schema,
            persona=base.persona,
            tuning_hz=base.tuning_hz,
            wer_max=base.wer_max,
            ecapa_min=base.ecapa_min,
            emotion_target=base.emotion_target,
            emotion_targets=dict(base.emotion_targets),
            embedding_min=0.5,
            backend_id=base.backend_id,
            source=base.source,
        )
        with tempfile.TemporaryDirectory(prefix="emotion-v2-emb-") as tmp:
            root = Path(tmp)
            render = root / "render"
            reference = root / "reference"
            scenario = root / "scenarios"
            scenario.mkdir()
            (scenario / "s01.txt").write_text("A fixed line.\n", encoding="utf-8")
            write_wav(render / "a.wav", synthesize_tone(432.0, seconds=0.05), 16000)
            write_wav(reference / "ref.wav", synthesize_tone(432.0, sample_rate=8000, seconds=0.05), 8000)
            row = score_emotion("calm", render, reference, scenario, gated, SplitBackend())
            self.assertTrue(row.emotion_pass)
            self.assertAlmostEqual(row.posterior_vs_reference or 0.0, 1.0, places=6)
            self.assertFalse(row.embedding_pass)
            self.assertFalse(row.passed)


class CliTest(unittest.TestCase):
    def test_cli_stub_scores_twelve_synthetic_emotions(self) -> None:
        with tempfile.TemporaryDirectory(prefix="emotion-v2-cli-") as tmp:
            root = Path(tmp)
            renders = root / "renders"
            references = root / "references"
            _write_tree(renders, lambda emotion: STUB_FREQ_HZ[emotion])
            _write_tree(references, lambda emotion: STUB_FREQ_HZ[emotion])
            out = root / "out"
            env = os.environ.copy()
            env["PYTHONPATH"] = str(support.EMOTION_V2)
            env["EMOTION_V2_BACKEND"] = "stub"
            env["HF_HUB_OFFLINE"] = "1"
            env["TRANSFORMERS_OFFLINE"] = "1"
            env["EMOTION_V2_SUPERB_WEIGHTS"] = str(root / "no-weights")
            completed = subprocess.run(
                [
                    sys.executable,
                    "-m",
                    "harness",
                    "score",
                    "--backend",
                    "stub",
                    "--render-dir",
                    str(renders),
                    "--reference-dir",
                    str(references),
                    "--out-dir",
                    str(out),
                ],
                cwd=REPO,
                env=env,
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(completed.returncode, 0, completed.stderr)
            self.assertIn("PASS", completed.stdout)
            self.assertTrue((out / "summary.md").is_file())
            self.assertTrue((out / "amused.json").is_file())
            refused = subprocess.run(
                [
                    sys.executable,
                    "-m",
                    "harness",
                    "score",
                    "--backend",
                    "superb",
                    "--render-dir",
                    str(renders),
                    "--reference-dir",
                    str(references),
                    "--out-dir",
                    str(root / "unused"),
                ],
                cwd=REPO,
                env=env,
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(refused.returncode, 2, refused.stderr)
            self.assertIn("refusing to download", refused.stderr)
            self.assertFalse((root / "no-weights").exists())


if __name__ == "__main__":
    unittest.main()
