# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Locked weights, gate, and pure scoring functions. No audio."""

from __future__ import annotations

import math
import unittest

import support  # noqa: F401  # puts the tool on sys.path

from scorelib import (
    CATEGORY_W,
    EMB_W,
    FORMULA,
    GATE,
    ClipScore,
    aggregate,
    as_prob,
    as_unit,
    cosine,
    emotion_match,
    intersection,
    meets_gate,
    pair_score,
    score_sector,
)


class FormulaTest(unittest.TestCase):
    def test_weights_and_gate_are_locked(self) -> None:
        self.assertEqual(FORMULA, "0.70*category_blend+0.30*emb_sim")
        self.assertEqual(CATEGORY_W, 0.70)
        self.assertEqual(EMB_W, 0.30)
        self.assertEqual(GATE, 0.83)
        self.assertTrue(math.isclose(CATEGORY_W + EMB_W, 1.0))

    def test_emotion_match_is_the_weighted_sum(self) -> None:
        self.assertAlmostEqual(emotion_match(0.9, 0.8), 0.70 * 0.9 + 0.30 * 0.8)
        self.assertAlmostEqual(emotion_match(1.0, 1.0), 1.0)
        self.assertAlmostEqual(emotion_match(0.0, 0.0), 0.0)
        # A perfect embedding cannot hide a weak category blend.
        self.assertAlmostEqual(emotion_match(0.5, 1.0), 0.65)
        self.assertLess(emotion_match(0.5, 1.0), GATE)

    def test_gate_is_inclusive_and_refuses_a_new_threshold(self) -> None:
        self.assertTrue(meets_gate(0.83))
        self.assertTrue(meets_gate(1.0))
        self.assertFalse(meets_gate(0.83 - 1e-4))
        self.assertFalse(meets_gate(0.829))
        with self.assertRaises(ValueError) as raised:
            meets_gate(0.9, gate=0.5)
        self.assertIn("locked", str(raised.exception))
        self.assertIn("0.83", str(raised.exception))


class MetricTest(unittest.TestCase):
    def test_intersection_and_cosine(self) -> None:
        self.assertAlmostEqual(intersection([0.5, 0.5], [0.5, 0.5]), 1.0)
        self.assertAlmostEqual(intersection([1.0, 0.0], [0.0, 1.0]), 0.0)
        self.assertAlmostEqual(intersection([0.7, 0.3], [0.4, 0.6]), 0.7)
        self.assertAlmostEqual(cosine(as_unit([1.0, 0.0]), as_unit([1.0, 0.0])), 1.0)
        self.assertAlmostEqual(cosine(as_unit([1.0, 0.0]), as_unit([0.0, 1.0])), 0.0)

    def test_as_prob_softens_logits(self) -> None:
        # Logits (not a distribution) go through softmax.
        probs = as_prob([3.0, 1.0, 0.0])
        self.assertAlmostEqual(sum(probs), 1.0, places=6)
        self.assertGreater(probs[0], probs[1])

    def test_pair_score_uses_locked_weights(self) -> None:
        category, sim, score = pair_score(
            [1.0, 0.0], [1.0, 0.0],
            [1.0, 0.0], [1.0, 0.0],
        )
        self.assertAlmostEqual(category, 1.0)
        self.assertAlmostEqual(sim, 1.0)
        self.assertAlmostEqual(score, 1.0)
        category, sim, score = pair_score(
            [1.0, 0.0], [1.0, 0.0],
            [0.0, 1.0], [0.0, 1.0],
        )
        self.assertAlmostEqual(category, 0.0)
        self.assertAlmostEqual(sim, 0.0)
        self.assertAlmostEqual(score, 0.0)


class AggregateTest(unittest.TestCase):
    def _clip(self, emotion: str, score_pair: tuple[float, float]) -> ClipScore:
        category, sim = score_pair
        return ClipScore(
            path=f"{emotion}.wav",
            emotion=emotion,
            line_id=emotion,
            category_blend=category,
            emb_sim=sim,
            emotion_match=emotion_match(category, sim),
        )

    def test_one_weak_sector_fails_the_voice_even_when_the_mean_is_high(self) -> None:
        strong = (1.0, 1.0)
        weak = (0.4, 0.4)  # 0.70*0.4+0.30*0.4 = 0.4, well under 0.83
        # Eleven perfect sectors plus one weak one: the mean is above 0.83,
        # but the voice still fails.
        strong_names = [
            "neutral", "calm", "happy", "amused", "excited", "tender",
            "sad", "disappointed", "fearful", "surprised", "disgusted",
        ]
        sectors = [
            score_sector(name, [self._clip(name, strong)], f"{name}.wav", f"{name:<64}"[:64])
            for name in strong_names
        ]
        sectors.append(
            score_sector("angry", [self._clip("angry", weak)], "angry.wav", "c" * 64)
        )
        report = aggregate(sectors, persona="samantha", backend="stub")
        self.assertGreater(report.emotion_match, GATE)
        self.assertLess(report.min_emotion_match or 1.0, GATE)
        self.assertTrue(all(row.passed for row in sectors if row.emotion != "angry"))
        self.assertFalse(sectors[-1].passed)
        self.assertFalse(report.passed)
        self.assertEqual(report.n_sectors, 12)
        self.assertEqual(report.n_passed, 11)
        body = report.as_dict()
        self.assertFalse(body["pass"])
        self.assertFalse(body["summary"]["pass"])
        self.assertEqual(body["summary"]["n_passed"], 11)
        self.assertIn("angry", body["sectors"])
        self.assertFalse(body["sectors"]["angry"]["pass"])

    def test_empty_report_does_not_pass(self) -> None:
        report = aggregate([])
        self.assertFalse(report.passed)
        self.assertIsNone(report.min_emotion_match)


if __name__ == "__main__":
    unittest.main()
