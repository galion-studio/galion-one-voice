# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Emotion-sweep gate. Real clips are read. Scores are not invented.

Temporary sines exist only inside the test process and are not committed.
They are not voices. The listening-set run forces the scorers absent.
"""

from __future__ import annotations

import hashlib
import json
import os
import stat
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

import support

from harness.audio import synthesize_tone, write_wav
from harness.sweep import (
    ECAPA_MIN,
    EMOTION_FORMULA,
    EMOTION_MIN,
    WER_MAX,
    SweepError,
    execute,
    histogram_intersection,
    judge_rollout,
    pair_emotion,
    read_reply_line,
    render_markdown,
)
from harness.sweep import Metric

REPO = support.REPO
CHECKED_IN = REPO / "docs/galion-voice-v2/evals/emotion-sweep/offline-eval"
REPLY = "I have moved the meeting to Thursday at four, and the notes are in your inbox."


def _metric(name: str, status: str, value: float | None, threshold: float, comparison: str, passed):
    return Metric(name, status, value, threshold, comparison, passed, status)


def _ok_wer(value: float) -> Metric:
    from harness.metrics import wer_passes

    return _metric("wer", "ok", value, WER_MAX, "<=", wer_passes(value, WER_MAX))


def _ok_ecapa(value: float) -> Metric:
    from harness.metrics import ecapa_passes

    return _metric("ecapa", "ok", value, ECAPA_MIN, ">", ecapa_passes(value, ECAPA_MIN))


def _ok_emotion(value: float) -> Metric:
    from harness.metrics import meets_min

    return _metric("emotion_match", "ok", value, EMOTION_MIN, ">=", meets_min(value, EMOTION_MIN))


def _absent(name: str, threshold: float, comparison: str) -> Metric:
    return _metric(name, "absent", None, threshold, comparison, None)


def _write_script(path: Path) -> None:
    path.write_text(
        "#!/usr/bin/env python3\n"
        "import json, os, sys\n"
        "args = sys.argv\n"
        "out = args[args.index('--out') + 1]\n"
        "if '--text-dir' in args:\n"
        "    value = float(os.environ.get('WER_VALUE', '0.018'))\n"
        "    json.dump({'summary': {'mean': value}, 'files': [{'wer': value}]}, open(out, 'w'))\n"
        "else:\n"
        "    value = float(os.environ.get('ECAPA_VALUE', '0.80'))\n"
        "    json.dump({'mean': value, 'scores': {'clip': value}}, open(out, 'w'))\n",
        encoding="utf-8",
    )
    path.chmod(path.stat().st_mode | stat.S_IEXEC)


def _mini_tree(root: Path) -> tuple[Path, Path, Path]:
    clips = root / "clips"
    clips.mkdir()
    for name in (
        "samantha-neutral-rest.wav",
        "samantha-reference-rest.wav",
        "samantha-v1-baseline.wav",
        "user-neutral.wav",
    ):
        write_wav(clips / name, synthesize_tone(440.0, seconds=0.05), 16000)
    csv_path = root / "sweep.csv"
    csv_path.write_text(
        "persona,user_emotion,mood_history,mood_label,file\n"
        "samantha,neutral,rest,warm,samantha-neutral-rest.wav\n",
        encoding="utf-8",
    )
    note = root / "SWEEP.md"
    note.write_text(
        'Reply line for every persona case: "The note is on the table."\n',
        encoding="utf-8",
    )
    return clips, csv_path, note


def _offline_env() -> dict[str, str | None]:
    keys = ("GALION_WER", "GALION_ECAPA", "EMOTION_V2_SUPERB_WEIGHTS", "EMOTION_V2_BACKEND")
    return {key: os.environ.get(key) for key in keys}


def _push_offline(saved_root: Path) -> None:
    os.environ.pop("GALION_WER", None)
    os.environ.pop("GALION_ECAPA", None)
    os.environ["EMOTION_V2_SUPERB_WEIGHTS"] = str(saved_root / "no-weights")
    os.environ["EMOTION_V2_BACKEND"] = "auto"


def _restore_env(saved: dict[str, str | None]) -> None:
    for key, value in saved.items():
        if value is None:
            os.environ.pop(key, None)
        else:
            os.environ[key] = value


class GateTest(unittest.TestCase):
    def test_locked_numbers(self) -> None:
        self.assertEqual(WER_MAX, 0.018)
        self.assertEqual(ECAPA_MIN, 0.7057)
        self.assertEqual(EMOTION_MIN, 0.83)
        self.assertEqual(EMOTION_FORMULA, "0.70*category_blend+0.30*emb_sim")

    def test_formula_matches_the_emotion_match_tool(self) -> None:
        tool = REPO / "tools/cloning-v2/emotion-match"
        sys.path.insert(0, str(tool))
        import scorelib

        self.assertEqual(scorelib.FORMULA, EMOTION_FORMULA)
        self.assertEqual(scorelib.GATE, EMOTION_MIN)
        category, sim, score = pair_emotion([0.7, 0.3], [3.0, 0.0], [0.4, 0.6], [3.0, 4.0])
        other_category, other_sim, other = scorelib.pair_score([0.7, 0.3], [3.0, 0.0], [0.4, 0.6], [3.0, 4.0])
        self.assertAlmostEqual(category, other_category)
        self.assertAlmostEqual(sim, other_sim)
        self.assertAlmostEqual(score, other)
        self.assertAlmostEqual(histogram_intersection([1.0, 0.0], [0.25, 0.75]), 0.25)

    def test_boundaries_do_not_treat_the_baseline_as_a_pass(self) -> None:
        passing = judge_rollout(_ok_wer(0.018), _ok_ecapa(ECAPA_MIN + 1e-6), _ok_emotion(0.83))
        self.assertEqual(passing["status"], "measured")
        self.assertTrue(passing["pass"])
        self.assertFalse(passing["claimed"])
        ceiling = judge_rollout(_ok_wer(0.018 + 1e-6), _ok_ecapa(0.8), _ok_emotion(1.0))
        self.assertFalse(ceiling["pass"])
        baseline = judge_rollout(_ok_wer(0.0), _ok_ecapa(ECAPA_MIN), _ok_emotion(1.0))
        self.assertEqual(baseline["status"], "measured")
        self.assertFalse(baseline["pass"])
        short = judge_rollout(_ok_wer(0.0), _ok_ecapa(0.9), _ok_emotion(0.83 - 1e-4))
        self.assertFalse(short["pass"])

    def test_absent_is_not_a_pass_and_carries_no_number(self) -> None:
        gate = judge_rollout(
            _absent("wer", WER_MAX, "<="),
            _absent("ecapa", ECAPA_MIN, ">"),
            _absent("emotion_match", EMOTION_MIN, ">="),
        )
        self.assertEqual(gate["status"], "unmeasured")
        self.assertIsNone(gate["pass"])
        self.assertFalse(gate["claimed"])
        self.assertIn("unmeasured", str(gate["reason"]))


class ListeningSetTest(unittest.TestCase):
    def test_real_clips_stay_unmeasured_without_scorers(self) -> None:
        saved = _offline_env()
        with tempfile.TemporaryDirectory(prefix="emotion-sweep-real-") as tmp:
            root = Path(tmp)
            _push_offline(root)
            try:
                code, report = execute(out_dir=root / "out", backend="auto")
                table = (root / "out" / "summary.md").read_text(encoding="utf-8")
                written = sorted(path.name for path in (root / "out").iterdir())
            finally:
                _restore_env(saved)
        self.assertEqual(code, 0)
        self.assertEqual(report["schema"], "galion-voice-v2-emotion-sweep-eval/1")
        self.assertFalse(report["audio_generated"])
        self.assertFalse(report["scores_fabricated"])
        self.assertFalse(report["rollout_claimed"])
        self.assertFalse(report["release"])
        self.assertEqual(report["reply_line"], REPLY)
        self.assertEqual(read_reply_line(REPO / "docs/galion-voice-v2/evals/emotion-sweep/SWEEP.md"), REPLY)
        inventory = report["inventory"]
        self.assertEqual(inventory["expected"], 46)
        self.assertEqual(inventory["present"], 46)
        self.assertEqual(inventory["missing"], [])
        self.assertEqual(inventory["unexpected"], [])
        self.assertEqual(inventory["persona_cases"], 36)
        self.assertEqual(inventory["references"], 2)
        self.assertEqual(inventory["v1_baselines"], 2)
        self.assertEqual(inventory["user_cues"], 6)
        self.assertEqual(report["summary"]["gate"], "unmeasured")
        self.assertEqual(report["summary"]["n_candidates"], 4)
        self.assertEqual(report["summary"]["n_measured"], 0)
        self.assertEqual([row["id"] for row in report["candidates"]], ["samantha", "jarvis", "samantha-v1", "jarvis-v1"])
        self.assertEqual([row["n_clips"] for row in report["candidates"]], [18, 18, 1, 1])
        for row in report["candidates"]:
            for key in ("wer", "ecapa", "emotion_match"):
                self.assertIsNone(row[key]["value"])
                self.assertIsNone(row[key]["pass"])
                self.assertEqual(row[key]["status"], "absent")
            self.assertEqual(row["gate"]["status"], "unmeasured")
            self.assertIsNone(row["gate"]["pass"])
            self.assertFalse(row["gate"]["claimed"])
            self.assertNotIn("user-cue", {clip["role"] for clip in row["clips"]})
        self.assertEqual(len(report["blockers"]), 3)
        self.assertIn("CPU stub was not substituted", report["scorers"]["emotion_match"]["detail"])
        self.assertIn("unmeasured", table)
        self.assertIn("No audio was generated", table)
        self.assertIn(REPLY, table)
        self.assertNotIn("| pass |", table)
        pinned = next(item for item in inventory["files"] if item["file"] == "samantha-neutral-rest.wav")
        digest = hashlib.sha256(
            (REPO / "docs/galion-voice-v2/evals/listening-set/emotion-sweep/samantha-neutral-rest.wav").read_bytes()
        ).hexdigest()
        self.assertEqual(pinned["sha256"], digest)
        self.assertEqual(pinned["sample_rate"], 22050)
        self.assertTrue(pinned["present"])
        self.assertEqual(written, ["report.json", "summary.md"])

    def test_checked_in_report_matches_a_forced_offline_run(self) -> None:
        saved = _offline_env()
        with tempfile.TemporaryDirectory(prefix="emotion-sweep-pin-") as tmp:
            root = Path(tmp)
            _push_offline(root)
            try:
                code, report = execute(out_dir=root / "out", backend="auto")
                fresh_json = (root / "out" / "report.json").read_text(encoding="utf-8")
                fresh_md = (root / "out" / "summary.md").read_text(encoding="utf-8")
            finally:
                _restore_env(saved)
        self.assertEqual(code, 0)
        self.assertEqual(fresh_json, (CHECKED_IN / "report.json").read_text(encoding="utf-8"))
        self.assertEqual(fresh_md, (CHECKED_IN / "summary.md").read_text(encoding="utf-8"))
        self.assertEqual(render_markdown(report), fresh_md)
        self.assertIsNone(json.loads(fresh_json)["candidates"][0]["wer"]["value"])

    def test_stub_backend_is_refused(self) -> None:
        saved = _offline_env()
        with tempfile.TemporaryDirectory(prefix="emotion-sweep-stub-") as tmp:
            root = Path(tmp)
            _push_offline(root)
            try:
                code, report = execute(out_dir=root / "out", backend="stub")
            finally:
                _restore_env(saved)
        self.assertEqual(code, 0)
        detail = report["scorers"]["emotion_match"]["detail"]
        self.assertIn("CPU stub", detail)
        self.assertIsNone(report["candidates"][0]["emotion_match"]["value"])
        self.assertIsNone(report["scorers"]["emotion_match"]["backend"])

    def test_require_scorers_fails_closed(self) -> None:
        saved = _offline_env()
        with tempfile.TemporaryDirectory(prefix="emotion-sweep-req-") as tmp:
            root = Path(tmp)
            _push_offline(root)
            try:
                code, report = execute(out_dir=root / "out", backend="auto", require_scorers=True)
            finally:
                _restore_env(saved)
        self.assertEqual(code, 1)
        self.assertEqual(report["summary"]["gate"], "unmeasured")
        self.assertIsNone(report["candidates"][0]["wer"]["value"])

    def test_source_does_not_download_or_synthesize(self) -> None:
        text = (support.EMOTION_V2 / "harness" / "sweep.py").read_text(encoding="utf-8")
        for banned in ("snapshot_download", "huggingface.co", "StubBackend", "write_wav", "synthesize_tone", "pip install"):
            self.assertNotIn(banned, text)


class ScriptedScorerTest(unittest.TestCase):
    def test_injected_scorers_are_the_only_numbers(self) -> None:
        with tempfile.TemporaryDirectory(prefix="emotion-sweep-script-") as tmp:
            root = Path(tmp)
            clips, csv_path, note = _mini_tree(root)
            wer = root / "wer"
            ecapa = root / "ecapa"
            _write_script(wer)
            _write_script(ecapa)

            def analyse(path: Path):
                posterior = (0.95, 0.05) if "reference" in path.name else (0.90, 0.10)
                return (("neu", "hap"), posterior, (1.0, 0.0))

            saved = {key: os.environ.get(key) for key in ("WER_VALUE", "ECAPA_VALUE")}
            os.environ["WER_VALUE"] = "0.018"
            os.environ["ECAPA_VALUE"] = "0.80"
            try:
                code, report = execute(
                    out_dir=root / "out",
                    clips_dir=clips,
                    csv_path=csv_path,
                    sweep_md=note,
                    wer_bin=wer,
                    ecapa_bin=ecapa,
                    emotion_analyse=analyse,
                    emotion_measurement=True,
                    emotion_backend_name="scripted",
                )
            finally:
                _restore_env(saved)
            table = (root / "out" / "summary.md").read_text(encoding="utf-8")
        self.assertEqual(code, 0, report["summary"])
        samantha = report["candidates"][0]
        self.assertEqual(samantha["id"], "samantha")
        self.assertEqual(samantha["wer"]["value"], 0.018)
        self.assertTrue(samantha["wer"]["pass"])
        self.assertEqual(samantha["ecapa"]["value"], 0.8)
        self.assertTrue(samantha["ecapa"]["pass"])
        self.assertAlmostEqual(samantha["emotion_match"]["value"], 0.965, places=6)
        self.assertEqual(samantha["emotion_match"]["backend"], "scripted")
        self.assertEqual(samantha["gate"]["status"], "measured")
        self.assertTrue(samantha["gate"]["pass"])
        self.assertFalse(samantha["gate"]["claimed"])
        self.assertEqual(samantha["wer"]["scorer_files"], [{"wer": 0.018}])
        self.assertEqual(report["summary"]["gate"], "pass")
        self.assertIn("0.0180 (pass)", table)

    def test_ecapa_baseline_fails_the_measured_gate(self) -> None:
        with tempfile.TemporaryDirectory(prefix="emotion-sweep-fail-") as tmp:
            root = Path(tmp)
            clips, csv_path, note = _mini_tree(root)
            wer = root / "wer"
            ecapa = root / "ecapa"
            _write_script(wer)
            _write_script(ecapa)

            def analyse(path: Path):
                del path
                return (("neu", "hap"), (1.0, 0.0), (1.0, 0.0))

            saved = {key: os.environ.get(key) for key in ("WER_VALUE", "ECAPA_VALUE")}
            os.environ["WER_VALUE"] = "0.0"
            os.environ["ECAPA_VALUE"] = "0.7057"
            try:
                code, report = execute(
                    out_dir=root / "out",
                    clips_dir=clips,
                    csv_path=csv_path,
                    sweep_md=note,
                    wer_bin=wer,
                    ecapa_bin=ecapa,
                    emotion_analyse=analyse,
                    emotion_measurement=True,
                    emotion_backend_name="scripted",
                )
            finally:
                _restore_env(saved)
        self.assertEqual(code, 1)
        self.assertEqual(report["summary"]["gate"], "fail")
        self.assertFalse(report["candidates"][0]["ecapa"]["pass"])
        self.assertEqual(report["candidates"][0]["ecapa"]["value"], 0.7057)
        self.assertFalse(report["rollout_claimed"])

    def test_missing_clip_is_an_error_without_a_score(self) -> None:
        with tempfile.TemporaryDirectory(prefix="emotion-sweep-miss-") as tmp:
            root = Path(tmp)
            clips, csv_path, note = _mini_tree(root)
            (clips / "samantha-neutral-rest.wav").unlink()
            code, report = execute(
                out_dir=root / "out",
                clips_dir=clips,
                csv_path=csv_path,
                sweep_md=note,
                backend="stub",
            )
        self.assertEqual(code, 2)
        self.assertIn("samantha-neutral-rest.wav", report["inventory"]["missing"])
        self.assertEqual(report["summary"]["gate"], "incomplete")
        self.assertIsNone(report["candidates"][0]["wer"]["value"])
        self.assertEqual(report["candidates"][0]["wer"]["status"], "error")

    def test_unsafe_manifest_names_are_refused(self) -> None:
        with tempfile.TemporaryDirectory(prefix="emotion-sweep-name-") as tmp:
            root = Path(tmp)
            clips, csv_path, note = _mini_tree(root)
            csv_path.write_text(
                "persona,user_emotion,mood_history,mood_label,file\n"
                "samantha,neutral,rest,warm,../outside.wav\n",
                encoding="utf-8",
            )
            with self.assertRaises(SweepError):
                execute(out_dir=root / "out", clips_dir=clips, csv_path=csv_path, sweep_md=note)

    def test_lowered_threshold_file_is_refused(self) -> None:
        original = (support.EMOTION_V2 / "thresholds.toml").read_text(encoding="utf-8")
        with tempfile.TemporaryDirectory(prefix="emotion-sweep-gate-") as tmp:
            root = Path(tmp)
            config = root / "thresholds.toml"
            config.write_text(original.replace("wer_max = 0.018", "wer_max = 0.5"), encoding="utf-8")
            with self.assertRaises(SweepError) as raised:
                execute(out_dir=root / "out", config=config)
            self.assertIn("locked", str(raised.exception))
            self.assertIn("0.018", str(raised.exception))


class CliSweepTest(unittest.TestCase):
    def test_cli_reports_unmeasured_and_require_scorers_exits_1(self) -> None:
        saved = _offline_env()
        with tempfile.TemporaryDirectory(prefix="emotion-sweep-cli-") as tmp:
            root = Path(tmp)
            env = os.environ.copy()
            env.pop("GALION_WER", None)
            env.pop("GALION_ECAPA", None)
            env["EMOTION_V2_SUPERB_WEIGHTS"] = str(root / "no-weights")
            env["EMOTION_V2_BACKEND"] = "auto"
            env["HF_HUB_OFFLINE"] = "1"
            env["TRANSFORMERS_OFFLINE"] = "1"
            env["PYTHONPATH"] = str(support.EMOTION_V2)
            out = root / "out"
            completed = subprocess.run(
                [sys.executable, "-m", "harness", "sweep", "--backend", "auto", "--out-dir", str(out)],
                cwd=REPO,
                env=env,
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(completed.returncode, 0, completed.stderr)
            self.assertIn("0/4 candidates measured", completed.stdout)
            self.assertIn("claimed=false", completed.stdout)
            payload = json.loads((out / "report.json").read_text(encoding="utf-8"))
            self.assertIsNone(payload["candidates"][0]["emotion_match"]["value"])
            self.assertFalse((root / "no-weights").exists())
            required = subprocess.run(
                [
                    sys.executable,
                    "-m",
                    "harness",
                    "sweep",
                    "--require-scorers",
                    "--out-dir",
                    str(root / "required"),
                ],
                cwd=REPO,
                env=env,
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(required.returncode, 1, required.stderr)
            self.assertIn("unmeasured", required.stdout)
        _restore_env(saved)


if __name__ == "__main__":
    unittest.main()
