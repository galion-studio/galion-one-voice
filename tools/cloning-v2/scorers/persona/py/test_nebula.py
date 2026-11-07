# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Synthetic distributions for the offline nebula metric.

No audio, no model, no generated dataset. The 0.70/0.30 emotion_match
formula and the 0.83 gate are checked, not changed.
"""

from __future__ import annotations

import math
import sys
import tempfile
import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REPO = Path(__file__).resolve().parents[5]
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT / "py"))

from nebula import (  # noqa: E402
    EMOTION_V2,
    field_for_clips,
    js_divergence,
    score_nebula,
    score_nebula_set,
    wasserstein_1d,
    wasserstein_1d_samples,
)
from persona_lib import (  # noqa: E402
    BLEND,
    EMB,
    GATE_DEFAULT,
    emotion_match,
    load_persona,
)

PERSONA_DIR = ROOT.parent.parent / "pipeline" / "personas"
EMOTIONS_TOML = REPO / "emotion-v2" / "emotions.toml"


def _weights(samantha: bool = False, **mass: float) -> dict[str, float]:
    labels = EMOTION_V2 + (("seductive",) if samantha else ())
    weights = {label: 0.0 for label in labels}
    for label, value in mass.items():
        if label not in weights:
            raise AssertionError(label)
        weights[label] = value
    return weights


def _clip(samantha: bool = False, vad=(0.0, 0.0, 0.0), samples=None, **mass: float) -> dict:
    payload = {"weights": _weights(samantha, **mass)}
    if samples is not None:
        payload["vad_samples"] = [
            {"valence": row[0], "arousal": row[1], "dominance": row[2]} for row in samples
        ]
    else:
        payload["vad"] = {"valence": vad[0], "arousal": vad[1], "dominance": vad[2]}
    return payload


class DistributionGeometry(unittest.TestCase):
    def test_inventory_order_matches_emotions_toml(self):
        data = tomllib.loads(EMOTIONS_TOML.read_text(encoding="utf-8"))
        ids = [row["id"] for row in sorted(data["emotions"], key=lambda row: row["order"])]
        self.assertEqual(ids, list(EMOTION_V2))
        self.assertEqual(len(ids), 12)
        self.assertNotIn("seductive", ids)

    def test_identical_distributions_are_zero(self):
        ref = _clip(neutral=0.5, calm=0.5, vad=(0.2, -0.1, 0.4))
        rendered = _clip(neutral=0.5, calm=0.5, vad=(0.2, -0.1, 0.4))
        report = score_nebula(ref, rendered, samantha=False)
        self.assertEqual(report["status"], "scored")
        self.assertEqual(report["js_divergence"], 0.0)
        self.assertEqual(report["wasserstein_1d"], 0.0)
        self.assertEqual(report["vad_distance"], 0.0)
        self.assertEqual(report["seductive"], "omitted")
        self.assertNotIn("seductive", report["labels"])
        self.assertFalse(report["affects_emotion_match"])
        self.assertFalse(report["affects_gate"])
        self.assertFalse(report["replaces_emotion_match"])

    def test_one_hot_shift_and_disjoint_js(self):
        ref = _clip(neutral=1.0)
        rendered = _clip(calm=1.0)
        report = score_nebula(ref, rendered, samantha=False)
        self.assertAlmostEqual(report["wasserstein_1d"], 1.0)
        self.assertAlmostEqual(report["js_divergence"], 1.0)
        self.assertAlmostEqual(js_divergence([1.0, 0.0], [0.0, 1.0]), 1.0)

    def test_known_split_on_the_first_two_bins(self):
        # Independent calculation, not a call back into score_nebula.
        # P = 0.75 neutral + 0.25 calm, Q = 0.25 neutral + 0.75 calm.
        # M = 0.5 / 0.5. KL(P||M) = 0.75*log2(1.5) + 0.25*log2(0.5).
        expected_js = 0.75 * math.log2(1.5) + 0.25 * math.log2(0.5)
        p = [0.75, 0.25] + [0.0] * 10
        q = [0.25, 0.75] + [0.0] * 10
        self.assertAlmostEqual(js_divergence(p, q), expected_js)
        self.assertAlmostEqual(wasserstein_1d(p, q), 0.5)
        report = score_nebula(
            _clip(neutral=0.75, calm=0.25),
            _clip(neutral=0.25, calm=0.75),
            samantha=False,
        )
        self.assertAlmostEqual(report["js_divergence"], expected_js)
        self.assertAlmostEqual(report["wasserstein_1d"], 0.5)
        self.assertAlmostEqual(report["per_emotion_abs"]["neutral"], 0.5)
        self.assertAlmostEqual(report["per_emotion_abs"]["calm"], 0.5)
        self.assertEqual(report["per_emotion_abs"]["angry"], 0.0)

    def test_mass_travels_the_full_inventory(self):
        report = score_nebula(_clip(neutral=1.0), _clip(disgusted=1.0), samantha=False)
        self.assertAlmostEqual(report["wasserstein_1d"], 11.0)
        self.assertAlmostEqual(report["js_divergence"], 1.0)

    def test_weights_are_normalised_and_order_independent(self):
        messy = _clip(calm=2.0, neutral=2.0)
        messy["weights"] = {key: messy["weights"][key] for key in reversed(messy["weights"])}
        report = score_nebula(messy, _clip(neutral=0.5, calm=0.5), samantha=False)
        self.assertEqual(report["js_divergence"], 0.0)
        self.assertAlmostEqual(report["reference_distribution"]["neutral"], 0.5)
        self.assertAlmostEqual(report["reference_distribution"]["calm"], 0.5)

    def test_vad_euclidean_is_3_4_5(self):
        report = score_nebula(
            _clip(neutral=1.0, vad=(0.0, 0.0, 0.0)),
            _clip(neutral=1.0, vad=(3.0, 4.0, 0.0)),
            samantha=False,
        )
        self.assertAlmostEqual(report["vad_distance"], 5.0)
        self.assertAlmostEqual(report["vad_abs"]["valence"], 3.0)
        self.assertAlmostEqual(report["vad_abs"]["arousal"], 4.0)
        self.assertEqual(report["vad_abs"]["dominance"], 0.0)
        self.assertAlmostEqual(report["vad_wasserstein_1d"]["valence"], 3.0)
        self.assertEqual(report["js_divergence"], 0.0)

    def test_vad_samples_pool_ratings_and_clips_separately(self):
        # Clip mean valence is 1 on both sides, so the Euclidean valence gap is 0.
        # Pooled ratings [0, 2] against [1]: half the mass moves 1 from 0, half
        # moves 1 from 2. Earth mover's distance is 1.
        ref = _clip(neutral=1.0, samples=[(0.0, 0.0, 0.0), (2.0, 0.0, 0.0)])
        rendered = _clip(neutral=1.0, vad=(1.0, 0.0, 0.0))
        report = score_nebula(ref, rendered, samantha=False)
        self.assertAlmostEqual(report["vad_distance"], 0.0)
        self.assertAlmostEqual(report["vad_wasserstein_1d"]["valence"], 1.0)
        self.assertAlmostEqual(wasserstein_1d_samples([0.0, 2.0], [1.0]), 1.0)

    def test_set_mean_is_equal_weight_per_clip(self):
        references = [_clip(neutral=1.0, vad=(-1.0, 0.0, 0.0)), _clip(happy=1.0, vad=(1.0, 0.0, 0.0))]
        rendered = [_clip(neutral=1.0, vad=(0.0, 0.0, 0.0))]
        report = score_nebula_set(references, rendered, samantha=False)
        self.assertAlmostEqual(report["reference_distribution"]["neutral"], 0.5)
        self.assertAlmostEqual(report["reference_distribution"]["happy"], 0.5)
        self.assertAlmostEqual(report["reference_vad"]["valence"], 0.0)
        self.assertEqual(report["n_reference"], 2)
        self.assertEqual(report["n_rendered"], 1)
        # Happy is index 2. Half the mass moves two bins: W1 = 1.
        self.assertAlmostEqual(report["wasserstein_1d"], 1.0)

    def test_samantha_includes_seductive_as_its_own_bin(self):
        report = score_nebula(
            _clip(True, neutral=1.0, seductive=0.0),
            _clip(True, seductive=1.0),
            samantha=True,
        )
        self.assertEqual(report["seductive"], "included")
        self.assertEqual(report["labels"][-1], "seductive")
        self.assertEqual(len(report["labels"]), 13)
        self.assertAlmostEqual(report["wasserstein_1d"], 12.0)

    def test_zero_seductive_is_kept_for_samantha(self):
        report = score_nebula(
            _clip(True, neutral=1.0, seductive=0.0),
            _clip(True, neutral=1.0, seductive=0.0),
            samantha=True,
        )
        self.assertEqual(report["reference_distribution"]["seductive"], 0.0)
        self.assertEqual(report["js_divergence"], 0.0)

    def test_jarvis_rejects_seductive_instead_of_storing_zero(self):
        payload = _clip(neutral=1.0)
        payload["weights"]["seductive"] = 0.0
        with self.assertRaises(ValueError):
            score_nebula(payload, _clip(neutral=1.0), samantha=False)

    def test_samantha_requires_the_seductive_key(self):
        with self.assertRaises(ValueError):
            score_nebula(_clip(neutral=1.0), _clip(neutral=1.0), samantha=True)

    def test_negative_empty_and_nonfinite_mass_are_rejected(self):
        negative = _clip(neutral=1.0)
        negative["weights"]["calm"] = -0.1
        with self.assertRaises(ValueError):
            score_nebula(negative, _clip(neutral=1.0), samantha=False)
        empty = _clip()
        with self.assertRaises(ValueError):
            score_nebula(empty, _clip(neutral=1.0), samantha=False)
        bad = _clip(neutral=1.0)
        bad["weights"]["neutral"] = math.nan
        with self.assertRaises(ValueError):
            score_nebula(bad, _clip(neutral=1.0), samantha=False)

    def test_vad_point_must_match_sample_mean(self):
        payload = _clip(neutral=1.0, samples=[(0.0, 0.0, 0.0), (2.0, 0.0, 0.0)])
        payload["vad"] = {"valence": 0.0, "arousal": 0.0, "dominance": 0.0}
        with self.assertRaises(ValueError):
            score_nebula(payload, _clip(neutral=1.0), samantha=False)

    def test_distances_are_symmetric(self):
        left = _clip(neutral=0.2, angry=0.8, vad=(-0.4, 0.7, 0.1))
        right = _clip(happy=0.3, tender=0.7, vad=(0.5, -0.2, 0.2))
        a = score_nebula(left, right, samantha=False)
        b = score_nebula(right, left, samantha=False)
        self.assertAlmostEqual(a["js_divergence"], b["js_divergence"])
        self.assertAlmostEqual(a["wasserstein_1d"], b["wasserstein_1d"])
        self.assertAlmostEqual(a["vad_distance"], b["vad_distance"])


class ReportField(unittest.TestCase):
    def test_absent_when_no_distribution_was_supplied(self):
        report = field_for_clips(
            samantha=False,
            reference_recs=[{"id": "w"}],
            rendered_recs=[{"id": "t01"}, {"id": "t02"}],
        )
        self.assertEqual(report["status"], "absent")
        self.assertIsNone(report["js_divergence"])
        self.assertIsNone(report["wasserstein_1d"])
        self.assertIsNone(report["vad_distance"])

    def test_partial_payloads_are_not_silently_averaged(self):
        report = field_for_clips(
            samantha=False,
            reference_recs=[{"nebula": _clip(neutral=1.0)}],
            rendered_recs=[{"nebula": _clip(neutral=1.0)}, {"id": "bare"}],
        )
        self.assertEqual(report["status"], "error")
        self.assertIsNone(report["js_divergence"])

    def test_clip_field_matches_direct_score(self):
        reference = _clip(True, neutral=1.0, seductive=0.0, vad=(0.1, 0.2, 0.3))
        rendered = _clip(True, angry=1.0, seductive=0.0, vad=(-0.2, 0.4, 0.0))
        via_field = field_for_clips(
            samantha=True,
            reference=[reference],
            rendered_recs=[{"nebula": rendered}],
        )
        direct = score_nebula(reference, rendered, samantha=True)
        self.assertEqual(via_field["status"], "scored")
        self.assertEqual(via_field["labels"][-1], "seductive")
        self.assertAlmostEqual(via_field["js_divergence"], direct["js_divergence"])
        self.assertAlmostEqual(via_field["wasserstein_1d"], direct["wasserstein_1d"])
        self.assertAlmostEqual(via_field["vad_distance"], direct["vad_distance"])
        self.assertGreater(via_field["wasserstein_1d"], 0.0)
        self.assertTrue(math.isfinite(via_field["vad_distance"]))


class EmotionMatchUntouched(unittest.TestCase):
    def setUp(self):
        self.persona = load_persona(PERSONA_DIR / "own-samantha.toml")
        from detectors import load_detectors

        self.detectors = load_detectors(
            plugin_dir=Path(tempfile.mkdtemp()),
            include_legacy_love_seductive_fallback=True,
        )
        self.window = {
            "id": "w_neu",
            "e2v": {
                "neutral": 1.0, "angry": 0.0, "sad": 0.0, "happy": 0.0,
                "disgusted": 0.0, "fearful": 0.0, "surprised": 0.0,
                "other": 0.0, "unknown": 0.0,
                "embedding": [1.0, 0.0, 0.0],
            },
        }
        self.line = {
            "id": "t01",
            "e2v": {
                "neutral": 1.0, "angry": 0.0, "sad": 0.0, "happy": 0.0,
                "disgusted": 0.0, "fearful": 0.0, "surprised": 0.0,
                "other": 0.0, "unknown": 0.0,
                "embedding": [1.0, 0.0, 0.0],
            },
        }
        self.moments = [{"category": "neutral", "strength": 1.0, "window": self.window}]

    def _match(self, line, reference=None):
        return emotion_match(
            line_recs=[line],
            moments=self.moments,
            persona=self.persona,
            detectors=self.detectors,
            nebula_reference=reference,
        )

    def test_coefficients_and_gate_constants(self):
        self.assertEqual(BLEND, 0.70)
        self.assertEqual(EMB, 0.30)
        self.assertEqual(GATE_DEFAULT, 0.83)
        self.assertEqual(self.persona.gate, 0.83)

    def test_nebula_does_not_move_emotion_match_or_the_gate(self):
        plain = self._match(self.line)
        decorated_line = dict(self.line)
        decorated_line["nebula"] = _clip(True, disgusted=1.0, seductive=0.0, vad=(-1.0, 1.0, -1.0))
        reference = _clip(True, neutral=1.0, seductive=0.0, vad=(1.0, -1.0, 1.0))
        scored = self._match(decorated_line, reference)
        self.assertEqual(scored["emotion_match"], plain["emotion_match"])
        self.assertEqual(scored["category_blend"], plain["category_blend"])
        self.assertEqual(scored["emb_sim"], plain["emb_sim"])
        self.assertEqual(scored["pass"], plain["pass"])
        self.assertTrue(scored["pass"])
        self.assertGreaterEqual(scored["emotion_match"], 0.83)
        self.assertAlmostEqual(
            scored["emotion_match"],
            0.70 * scored["category_blend"] + 0.30 * scored["emb_sim"],
        )
        self.assertEqual(plain["nebula"]["status"], "absent")
        self.assertEqual(scored["nebula"]["status"], "scored")
        self.assertGreater(scored["nebula"]["js_divergence"], 0.0)
        self.assertGreater(scored["nebula"]["wasserstein_1d"], 0.0)
        self.assertGreater(scored["nebula"]["vad_distance"], 0.0)
        keys = list(scored)
        self.assertEqual(keys[keys.index("emotion_match") + 1], "nebula")
        formula = (ROOT / "py" / "persona_lib.py").read_text(encoding="utf-8")
        self.assertIn("em = BLEND * cat_blend + EMB * emb_sim", formula)
        self.assertIn("emotion_match = 0.70 * category_blend + 0.30 * emb_sim", formula)
        universal = (
            REPO / "tools" / "cloning-v2" / "scorers" / "universal" / "py" / "universal_match.py"
        ).read_text(encoding="utf-8")
        self.assertEqual(universal.count("0.70 * cat_c + 0.30 * emb_c"), 1)
        self.assertEqual(universal.count("0.70 * cat_b + 0.30 * emb_s"), 1)


if __name__ == "__main__":
    unittest.main()
