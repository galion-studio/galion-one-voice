# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Per-sector emotion_match on synthetic tones. No weights, no network."""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
import unittest
import urllib.request
from pathlib import Path

import support

from scorelib import GATE, emotion_match
from sector_refs import SHARED_SECTORS, collect_jobs, load_manifest, resolve_ref
from stub import STUB_FREQ_HZ, StubBackend, write_sector_tone

TOOL = support.TOOL
SCRIPT = TOOL / "emotion_match.py"


def _run(args: list[str], env: dict[str, str] | None = None) -> subprocess.CompletedProcess[str]:
    inherited = os.environ.copy()
    inherited.setdefault("HF_HUB_OFFLINE", "1")
    inherited.setdefault("TRANSFORMERS_OFFLINE", "1")
    inherited.setdefault("HF_DATASETS_OFFLINE", "1")
    inherited.setdefault("EMOTION_V2_BACKEND", "stub")
    if env:
        inherited.update(env)
    return subprocess.run(
        [sys.executable, str(SCRIPT), *args],
        cwd=str(support.REPO),
        env=inherited,
        capture_output=True,
        text=True,
        check=False,
    )


class PerSectorScoreTest(unittest.TestCase):
    def test_matching_sines_pass_all_twelve_shared_sectors(self) -> None:
        backend = StubBackend()
        with tempfile.TemporaryDirectory(prefix="em-pass-") as tmp:
            root = Path(tmp)
            refs, renders = support.write_matching_trees(root)
            lines = [
                {"id": f"{emotion}-s01", "emotion": emotion, "path": str(renders[emotion])}
                for emotion in SHARED_SECTORS
            ]
            manifest = load_manifest(support.write_manifest(root / "refs.json", refs, lines))
            from emotion_match import run_per_sector

            # Score through the same functions the CLI uses.
            from scorelib import ClipScore, aggregate, pair_score, score_sector

            sectors = []
            for emotion in SHARED_SECTORS:
                ref = resolve_ref(manifest, emotion)
                ref_prob, ref_emb = backend.analyse(ref.path)
                clips = []
                for job in collect_jobs(manifest):
                    if job.emotion != emotion:
                        continue
                    rp, re = backend.analyse(job.path)
                    category, sim, score = pair_score(rp, re, ref_prob, ref_emb)
                    clips.append(
                        ClipScore(
                            path=str(job.path),
                            emotion=emotion,
                            line_id=job.id,
                            category_blend=category,
                            emb_sim=sim,
                            emotion_match=score,
                        )
                    )
                sectors.append(score_sector(emotion, clips, str(ref.path), ref.sha256))
            report = aggregate(sectors, persona="samantha", backend="stub")
            self.assertTrue(report.passed)
            self.assertEqual(report.n_sectors, 12)
            self.assertEqual(report.n_passed, 12)
            self.assertGreaterEqual(report.min_emotion_match or 0.0, GATE)
            for row in report.sectors:
                self.assertGreaterEqual(row.emotion_match, GATE)
                self.assertTrue(row.passed)
                self.assertEqual(row.n, 1)

    def test_mismatched_sector_fails_that_sector_and_the_gate(self) -> None:
        backend = StubBackend()
        with tempfile.TemporaryDirectory(prefix="em-mis-") as tmp:
            root = Path(tmp)
            refs, renders = support.write_matching_trees(root)
            # Happy render is a disgusted tone, scored against the happy ref.
            write_sector_tone(renders["happy"], "disgusted")
            lines = [
                {"id": f"{emotion}-s01", "emotion": emotion, "path": str(renders[emotion])}
                for emotion in SHARED_SECTORS
            ]
            manifest = load_manifest(support.write_manifest(root / "refs.json", refs, lines))
            from scorelib import ClipScore, aggregate, pair_score, score_sector

            sectors = []
            for emotion in SHARED_SECTORS:
                ref = resolve_ref(manifest, emotion)
                ref_prob, ref_emb = backend.analyse(ref.path)
                clips = []
                for job in collect_jobs(manifest):
                    if job.emotion != emotion:
                        continue
                    rp, re = backend.analyse(job.path)
                    category, sim, score = pair_score(rp, re, ref_prob, ref_emb)
                    clips.append(
                        ClipScore(
                            path=str(job.path),
                            emotion=emotion,
                            line_id=job.id,
                            category_blend=category,
                            emb_sim=sim,
                            emotion_match=score,
                        )
                    )
                sectors.append(score_sector(emotion, clips, str(ref.path), ref.sha256))
            report = aggregate(sectors, persona="samantha", backend="stub")
            by_name = {row.emotion: row for row in report.sectors}
            self.assertFalse(report.passed)
            self.assertFalse(by_name["happy"].passed)
            self.assertLess(by_name["happy"].emotion_match, GATE)
            self.assertTrue(all(by_name[name].passed for name in SHARED_SECTORS if name != "happy"))
            # The mean can still sit near the gate; the min is what fails the voice.
            self.assertGreater(report.emotion_match, by_name["happy"].emotion_match)
            self.assertEqual(report.min_emotion_match, by_name["happy"].emotion_match)

    def test_cli_stub_scores_twelve_sectors_and_writes_json(self) -> None:
        with tempfile.TemporaryDirectory(prefix="em-cli-") as tmp:
            root = Path(tmp)
            refs, _renders = support.write_matching_trees(root)
            manifest = support.write_manifest(root / "refs.json", refs)
            out = root / "out.json"
            completed = _run(
                [
                    "--manifest",
                    str(manifest),
                    "--renders-dir",
                    str(root / "renders"),
                    "--backend",
                    "stub",
                    "--out",
                    str(out),
                ]
            )
            self.assertEqual(completed.returncode, 0, completed.stderr)
            payload = json.loads((out).read_text(encoding="utf-8"))
            self.assertEqual(payload["mode"], "per-sector")
            self.assertEqual(payload["formula"], "0.70*category_blend+0.30*emb_sim")
            self.assertEqual(payload["gate"], 0.83)
            self.assertTrue(payload["pass"])
            self.assertEqual(payload["n_sectors"], 12)
            self.assertEqual(payload["n_passed"], 12)
            self.assertEqual(len(payload["sectors"]), 12)
            self.assertEqual(len(payload["summary"]["sectors"]), 12)
            for emotion in SHARED_SECTORS:
                row = payload["sectors"][emotion]
                self.assertTrue(row["pass"])
                self.assertGreaterEqual(row["emotion_match"], 0.83)
                self.assertEqual(row["ref"]["sha256"], support.sha256_file(refs[emotion]))
                self.assertIn("category_blend", row)
                self.assertIn("emb_sim", row)
            self.assertIn("min_emotion_match", payload["summary"])
            self.assertTrue(payload["summary"]["pass"])
            stdout = json.loads(completed.stdout)
            self.assertEqual(stdout["n_sectors"], 12)

    def test_cli_missing_sector_ref_exits_2_with_a_clear_error(self) -> None:
        with tempfile.TemporaryDirectory(prefix="em-cli-miss-") as tmp:
            root = Path(tmp)
            refs, _ = support.write_matching_trees(root)
            refs.pop("sad")
            (root / "refs" / "sad.wav").unlink()
            manifest = support.write_manifest(root / "refs.json", refs)
            completed = _run(
                [
                    "--manifest",
                    str(manifest),
                    "--renders-dir",
                    str(root / "renders"),
                    "--backend",
                    "stub",
                ]
            )
            self.assertEqual(completed.returncode, 2, completed.stdout + completed.stderr)
            self.assertIn("missing sector reference for sad", completed.stderr)
            self.assertIn("path and sha256", completed.stderr)
            self.assertNotIn("Traceback", completed.stderr)

    def test_cli_sha256_mismatch_exits_2(self) -> None:
        with tempfile.TemporaryDirectory(prefix="em-cli-sha-") as tmp:
            root = Path(tmp)
            refs, _ = support.write_matching_trees(root)
            manifest = support.write_manifest(root / "refs.json", refs)
            body = json.loads(manifest.read_text(encoding="utf-8"))
            body["references"]["calm"]["sha256"] = "ab" * 32
            manifest.write_text(json.dumps(body), encoding="utf-8")
            completed = _run(
                [
                    "--manifest",
                    str(manifest),
                    "--renders-dir",
                    str(root / "renders"),
                    "--backend",
                    "stub",
                ]
            )
            self.assertEqual(completed.returncode, 2, completed.stdout + completed.stderr)
            self.assertIn("sha256 mismatch", completed.stderr)
            self.assertIn("calm", completed.stderr)

    def test_cli_divergent_render_exits_1_and_reports_the_weak_sector(self) -> None:
        with tempfile.TemporaryDirectory(prefix="em-cli-fail-") as tmp:
            root = Path(tmp)
            refs, renders = support.write_matching_trees(root)
            write_sector_tone(renders["fearful"], "happy")
            manifest = support.write_manifest(root / "refs.json", refs)
            out = root / "out.json"
            completed = _run(
                [
                    "--manifest",
                    str(manifest),
                    "--renders-dir",
                    str(root / "renders"),
                    "--backend",
                    "stub",
                    "--out",
                    str(out),
                ]
            )
            self.assertEqual(completed.returncode, 1, completed.stderr)
            payload = json.loads(out.read_text(encoding="utf-8"))
            self.assertFalse(payload["pass"])
            self.assertFalse(payload["sectors"]["fearful"]["pass"])
            self.assertLess(payload["sectors"]["fearful"]["emotion_match"], 0.83)
            self.assertEqual(payload["summary"]["n_passed"], 11)
            self.assertEqual(payload["min_emotion_match"], payload["sectors"]["fearful"]["emotion_match"])

    def test_cli_refuses_to_lower_the_gate(self) -> None:
        completed = _run(["--manifest", "unused.json", "--gate", "0.5", "--backend", "stub"])
        self.assertEqual(completed.returncode, 2)
        self.assertIn("locked at 0.83", completed.stderr)

    def test_self_test_does_not_need_torch(self) -> None:
        completed = _run(["--self-test"])
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertTrue(completed.stdout.startswith("ok "))
        # The self-test prints the locked formula result 0.70*0.9+0.30*0.8 = 0.87
        self.assertIn("0.87", completed.stdout)

    def test_stub_path_does_not_import_torch_or_reach_the_network(self) -> None:
        def refuse(*_args, **_kwargs):
            raise AssertionError("emotion_match tried to use the network")

        original = urllib.request.urlopen
        urllib.request.urlopen = refuse
        try:
            with tempfile.TemporaryDirectory(prefix="em-offline-") as tmp:
                root = Path(tmp)
                refs, _ = support.write_matching_trees(root)
                manifest = support.write_manifest(root / "refs.json", refs)
                blocker = root / "block_imports.py"
                blocker.write_text(
                    "import builtins\n"
                    "_real = builtins.__import__\n"
                    "def _blocked(name, *args, **kwargs):\n"
                    "    root = name.split('.')[0]\n"
                    "    if root in {'torch', 'torchaudio', 'transformers', 'urllib'}:\n"
                    "        raise AssertionError(f'stub path imported {name}')\n"
                    "    return _real(name, *args, **kwargs)\n"
                    "builtins.__import__ = _blocked\n",
                    encoding="utf-8",
                )
                env = os.environ.copy()
                env["PYTHONPATH"] = str(root) + os.pathsep + env.get("PYTHONPATH", "")
                # -c would skip the sitecustomize-style file; prepend via -I no.
                completed = subprocess.run(
                    [
                        sys.executable,
                        "-c",
                        "import runpy, sys; "
                        f"sys.path.insert(0, {str(root)!r}); "
                        "import block_imports; "  # noqa
                        f"sys.argv = ['emotion_match', '--manifest', {str(manifest)!r}, "
                        f"'--renders-dir', {str(root / 'renders')!r}, '--backend', 'stub']; "
                        f"runpy.run_path({str(SCRIPT)!r}, run_name='__main__')",
                    ],
                    cwd=str(support.REPO),
                    env={**env, "HF_HUB_OFFLINE": "1", "TRANSFORMERS_OFFLINE": "1"},
                    capture_output=True,
                    text=True,
                    check=False,
                )
                self.assertEqual(completed.returncode, 0, completed.stderr + completed.stdout)
                self.assertNotIn("AssertionError", completed.stderr)
        finally:
            urllib.request.urlopen = original

    def test_seductive_overlay_is_scored_when_present(self) -> None:
        with tempfile.TemporaryDirectory(prefix="em-sed-cli-") as tmp:
            root = Path(tmp)
            refs, _ = support.write_matching_trees(root, include_seductive=True)
            manifest = support.write_manifest(root / "refs.json", refs)
            completed = _run(
                [
                    "--manifest",
                    str(manifest),
                    "--renders-dir",
                    str(root / "renders"),
                    "--backend",
                    "stub",
                ]
            )
            self.assertEqual(completed.returncode, 0, completed.stderr)
            payload = json.loads(completed.stdout)
            self.assertEqual(payload["n_sectors"], 13)
            self.assertIn("seductive", payload["sectors"])
            self.assertTrue(payload["sectors"]["seductive"]["pass"])
            self.assertGreaterEqual(payload["sectors"]["seductive"]["emotion_match"], 0.83)

    def test_formula_on_stub_outputs_matches_locked_weights(self) -> None:
        backend = StubBackend()
        with tempfile.TemporaryDirectory(prefix="em-formula-") as tmp:
            root = Path(tmp)
            ref = write_sector_tone(root / "amused-ref.wav", "amused")
            same = write_sector_tone(root / "amused-render.wav", "amused")
            other = write_sector_tone(root / "angry-render.wav", "angry")
            ref_p, ref_e = backend.analyse(ref)
            from scorelib import pair_score

            category, sim, score = pair_score(*backend.analyse(same), ref_p, ref_e)
            self.assertAlmostEqual(score, emotion_match(category, sim))
            self.assertGreaterEqual(score, GATE)
            category, sim, score = pair_score(*backend.analyse(other), ref_p, ref_e)
            self.assertAlmostEqual(score, 0.70 * category + 0.30 * sim)
            self.assertLess(score, GATE)
            self.assertEqual(STUB_FREQ_HZ["amused"], 432.0)


if __name__ == "__main__":
    unittest.main()
