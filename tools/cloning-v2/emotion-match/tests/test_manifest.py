# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Manifest loading, sha256 pins, and closed failures. Sine fixtures only."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

import support

from sector_refs import (
    SCHEMA,
    SHARED_SECTORS,
    SectorRefError,
    collect_jobs,
    load_manifest,
    resolve_ref,
)
from stub import write_sector_tone, write_wav, synthesize_tone


class ManifestTest(unittest.TestCase):
    def test_load_and_resolve_pinned_refs(self) -> None:
        with tempfile.TemporaryDirectory(prefix="em-manifest-") as tmp:
            root = Path(tmp)
            refs, _renders = support.write_matching_trees(root)
            manifest_path = support.write_manifest(root / "refs.json", refs)
            loaded = load_manifest(manifest_path)
            self.assertEqual(loaded.schema, SCHEMA)
            self.assertEqual(loaded.persona, "samantha")
            self.assertEqual(loaded.gate, 0.83)
            self.assertEqual(set(loaded.references), set(SHARED_SECTORS))
            for emotion, path in refs.items():
                resolved = resolve_ref(loaded, emotion)
                self.assertEqual(resolved.path, path.resolve())
                self.assertEqual(resolved.sha256, support.sha256_file(path))

    def test_paths_are_relative_to_the_manifest_or_root(self) -> None:
        with tempfile.TemporaryDirectory(prefix="em-rel-") as tmp:
            root = Path(tmp)
            ref = write_sector_tone(root / "refs" / "happy.wav", "happy")
            body = {
                "schema": SCHEMA,
                "references": {
                    "happy": {
                        "path": "refs/happy.wav",
                        "sha256": support.sha256_file(ref),
                    }
                },
            }
            path = root / "manifests" / "refs.json"
            path.parent.mkdir()
            path.write_text(json.dumps(body), encoding="utf-8")
            # Relative to --root, not to the manifest's directory.
            loaded = load_manifest(path, root=root)
            self.assertEqual(resolve_ref(loaded, "Happy").path, ref.resolve())

    def test_refuses_a_lowered_or_raised_gate(self) -> None:
        with tempfile.TemporaryDirectory(prefix="em-gate-") as tmp:
            root = Path(tmp)
            ref = write_sector_tone(root / "neutral.wav", "neutral")
            for value in (0.5, 0.82, 0.9, 1.0):
                path = support.write_manifest(
                    root / f"g{value}.json",
                    {"neutral": ref},
                    extra={"gate": value},
                )
                with self.assertRaises(SectorRefError) as raised:
                    load_manifest(path)
                message = str(raised.exception)
                self.assertIn("locked", message)
                self.assertIn("0.83", message)

    def test_refuses_changed_weights_or_formula(self) -> None:
        with tempfile.TemporaryDirectory(prefix="em-w-") as tmp:
            root = Path(tmp)
            ref = write_sector_tone(root / "neutral.wav", "neutral")
            formula = support.write_manifest(
                root / "formula.json",
                {"neutral": ref},
                extra={"formula": "0.50*category_blend+0.50*emb_sim"},
            )
            with self.assertRaises(SectorRefError) as raised:
                load_manifest(formula)
            self.assertIn("0.70*category_blend+0.30*emb_sim", str(raised.exception))
            weights = support.write_manifest(
                root / "weights.json",
                {"neutral": ref},
                extra={"weights": {"category_blend": 0.5, "emb_sim": 0.5}},
            )
            with self.assertRaises(SectorRefError) as raised:
                load_manifest(weights)
            self.assertIn("locked", str(raised.exception))

    def test_sha256_mismatch_fails_clearly(self) -> None:
        with tempfile.TemporaryDirectory(prefix="em-sha-") as tmp:
            root = Path(tmp)
            ref = write_sector_tone(root / "sad.wav", "sad")
            other = write_wav(root / "other.wav", synthesize_tone(210.0))
            path = support.write_manifest(root / "refs.json", {"sad": ref})
            # Tamper the pin so it no longer matches the file on disk.
            body = json.loads(path.read_text(encoding="utf-8"))
            body["references"]["sad"]["sha256"] = support.sha256_file(other)
            path.write_text(json.dumps(body), encoding="utf-8")
            loaded = load_manifest(path)
            with self.assertRaises(SectorRefError) as raised:
                resolve_ref(loaded, "sad")
            message = str(raised.exception)
            self.assertIn("sha256 mismatch", message)
            self.assertIn("sad", message)
            self.assertIn(support.sha256_file(other), message)
            self.assertIn(support.sha256_file(ref), message)

    def test_missing_reference_file_fails_clearly(self) -> None:
        with tempfile.TemporaryDirectory(prefix="em-miss-file-") as tmp:
            root = Path(tmp)
            ref = write_sector_tone(root / "angry.wav", "angry")
            digest = support.sha256_file(ref)
            path = support.write_manifest(root / "refs.json", {"angry": ref})
            ref.unlink()
            loaded = load_manifest(path)
            with self.assertRaises(SectorRefError) as raised:
                resolve_ref(loaded, "angry")
            message = str(raised.exception)
            self.assertIn("not found", message)
            self.assertIn("angry", message)
            self.assertIn("angry.wav", message)
            self.assertEqual(len(digest), 64)

    def test_missing_sector_ref_for_a_line_fails_clearly(self) -> None:
        with tempfile.TemporaryDirectory(prefix="em-miss-sec-") as tmp:
            root = Path(tmp)
            happy = write_sector_tone(root / "happy.wav", "happy")
            render = write_sector_tone(root / "renders" / "sad" / "s01.wav", "sad")
            path = support.write_manifest(
                root / "refs.json",
                {"happy": happy},
                lines=[{"id": "sad-s01", "emotion": "sad", "path": str(render)}],
            )
            loaded = load_manifest(path)
            jobs = collect_jobs(loaded)
            self.assertEqual(jobs[0].emotion, "sad")
            with self.assertRaises(SectorRefError) as raised:
                resolve_ref(loaded, jobs[0].emotion, jobs[0].id)
            message = str(raised.exception)
            self.assertIn("missing sector reference for sad", message)
            self.assertIn("sad-s01", message)
            self.assertIn("path and sha256", message)

    def test_unknown_emotion_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory(prefix="em-unk-") as tmp:
            root = Path(tmp)
            ref = write_sector_tone(root / "neutral.wav", "neutral")
            path = support.write_manifest(root / "refs.json", {"neutral": ref})
            body = json.loads(path.read_text(encoding="utf-8"))
            body["references"]["grumpy"] = body["references"]["neutral"]
            path.write_text(json.dumps(body), encoding="utf-8")
            with self.assertRaises(SectorRefError) as raised:
                load_manifest(path)
            self.assertIn("unknown emotion sector", str(raised.exception))
            self.assertIn("grumpy", str(raised.exception))

    def test_seductive_is_optional_until_a_line_uses_it(self) -> None:
        with tempfile.TemporaryDirectory(prefix="em-sed-") as tmp:
            root = Path(tmp)
            refs, _ = support.write_matching_trees(root)
            path = support.write_manifest(root / "refs.json", refs)
            loaded = load_manifest(path)
            self.assertNotIn("seductive", loaded.references)
            with self.assertRaises(SectorRefError) as raised:
                resolve_ref(loaded, "seductive", "seductive-s01")
            self.assertIn("missing sector reference for seductive", str(raised.exception))
            overlay = write_sector_tone(root / "refs" / "seductive.wav", "seductive")
            refs["seductive"] = overlay
            path = support.write_manifest(root / "with-overlay.json", refs)
            loaded = load_manifest(path)
            resolved = resolve_ref(loaded, "seductive")
            self.assertEqual(resolved.path, overlay.resolve())

    def test_bad_sha256_format_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory(prefix="em-fmt-") as tmp:
            root = Path(tmp)
            ref = write_sector_tone(root / "calm.wav", "calm")
            path = support.write_manifest(root / "refs.json", {"calm": ref})
            body = json.loads(path.read_text(encoding="utf-8"))
            body["references"]["calm"]["sha256"] = "not-a-digest"
            path.write_text(json.dumps(body), encoding="utf-8")
            with self.assertRaises(SectorRefError) as raised:
                load_manifest(path)
            self.assertIn("64-character hex", str(raised.exception))


if __name__ == "__main__":
    unittest.main()
