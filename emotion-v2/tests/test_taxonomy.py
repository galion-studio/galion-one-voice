# Galion Voice — Perpetual Open MIT License
# (c) 2026 Maciej Grajczyk / G.A.L.I.O.N.
"""Validate emotion-v2/emotions.toml and the parser's rejection of bad edits."""

from __future__ import annotations

import math
import pathlib
import sys
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))

import taxonomy  # noqa: E402

TOML = ROOT / "emotions.toml"


class EmotionsTomlTest(unittest.TestCase):
    def setUp(self):
        self.tax = taxonomy.load(TOML)

    def test_twelve_entries_with_unique_ids(self):
        self.assertEqual(len(self.tax.emotions), 12)
        ids = [item.id for item in self.tax.emotions]
        self.assertEqual(len(ids), len(set(ids)))
        self.assertEqual(tuple(ids), taxonomy.REQUIRED_IDS)

    def test_coordinates_lie_in_the_unit_interval(self):
        for item in self.tax.emotions:
            self.assertGreaterEqual(item.valence, -1.0, item.id)
            self.assertLessEqual(item.valence, 1.0, item.id)
            self.assertGreaterEqual(item.arousal, -1.0, item.id)
            self.assertLessEqual(item.arousal, 1.0, item.id)
            self.assertTrue(math.isfinite(item.valence), item.id)
            self.assertTrue(math.isfinite(item.arousal), item.id)

    def test_russell_angles_are_the_stored_coordinates(self):
        found = []
        for item in self.tax.emotions:
            if item.coordinate_basis != "russell-1980-angle":
                self.assertIsNone(item.russell_angle_deg, item.id)
                continue
            found.append(item.id)
            radians = math.radians(item.russell_angle_deg)
            self.assertAlmostEqual(item.valence, round(math.cos(radians), 2))
            self.assertAlmostEqual(item.arousal, round(math.sin(radians), 2))
        self.assertEqual(found, ["calm", "happy", "excited", "sad"])

    def test_pitch_centre_is_the_432_unison(self):
        self.assertEqual(self.tax.pitch_centre_hz, 432)
        self.assertEqual(self.tax.pitch_centre_ratio, "1/1")
        for item in self.tax.emotions:
            for ratio in (
                item.prosody.pitch_low_ratio,
                item.prosody.pitch_median_ratio,
                item.prosody.pitch_high_ratio,
            ):
                self.assertIn(ratio, taxonomy.ALLOWED_RATIOS)
                self.assertNotEqual(432 * item.prosody.ratio_value(ratio), 440)

    def test_sectors_and_three_texts(self):
        for sector, members in taxonomy.SECTORS.items():
            got = tuple(item.id for item in self.tax.emotions if item.sector == sector)
            self.assertEqual(got, members)
        texts = []
        for item in self.tax.emotions:
            self.assertEqual(len(item.texts), 3, item.id)
            texts.extend(item.texts)
        self.assertEqual(len(texts), len(set(texts)))

    def rewritten(self, old: str, new: str) -> pathlib.Path:
        text = TOML.read_text(encoding="utf-8")
        self.assertEqual(text.count(old), 1, old)
        work = tempfile.TemporaryDirectory()
        self.addCleanup(work.cleanup)
        path = pathlib.Path(work.name) / "emotions.toml"
        path.write_text(text.replace(old, new, 1), encoding="utf-8")
        return path

    def test_rejects_a_coordinate_outside_the_unit_interval(self):
        path = self.rewritten(
            'id = "happy"\nname = "Happy"\nsector = "bright"\norder = 3\nvalence = 0.99\n',
            'id = "happy"\nname = "Happy"\nsector = "bright"\norder = 3\nvalence = 1.5\n',
        )
        with self.assertRaises(taxonomy.TaxonomyError) as caught:
            taxonomy.load(path)
        self.assertTrue(any("[-1, 1]" in err or "unit-circle" in err for err in caught.exception.errors))

    def test_rejects_a_duplicate_id(self):
        path = self.rewritten(
            'id = "calm"\nname = "Calm"',
            'id = "neutral"\nname = "Neutral"',
        )
        with self.assertRaises(taxonomy.TaxonomyError) as caught:
            taxonomy.load(path)
        self.assertTrue(any("not unique" in err for err in caught.exception.errors))

    def test_rejects_retuning_the_centre_to_440(self):
        path = self.rewritten("pitch_centre_hz = 432\n", "pitch_centre_hz = 440\n")
        with self.assertRaises(taxonomy.TaxonomyError) as caught:
            taxonomy.load(path)
        self.assertTrue(any("432" in err for err in caught.exception.errors))


if __name__ == "__main__":
    unittest.main()
