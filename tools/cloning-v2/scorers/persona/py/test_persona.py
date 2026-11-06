#!/usr/bin/env python3
"""Unit + probe-fixture tests for the persona scorer core.

Reads scorer-probe/results/SUMMARY.json as a read-only fixture.
Does not write scorer-probe/results-persona/ and does not call models.
"""
from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT / "py"))

from detectors import (  # noqa: E402
    CryingDetector, decision_scores, discover_love_seductive, load_detectors,
)
from detectors.legacy_clap_fallback import LegacyClapLoveSeductiveFallback  # noqa: E402
from persona_lib import (  # noqa: E402
    NA, apply_persona_scores, category_score, emotion_match, load_persona,
    resolve_persona_path, strength,
)

PERSONA_DIR = ROOT.parent.parent / "pipeline" / "personas"
SUMMARY = ROOT.parent.parent / "scorer-probe" / "results" / "SUMMARY.json"
PRESENCE = 0.20


class PersonaFiles(unittest.TestCase):
    def setUp(self):
        self.sam = load_persona(PERSONA_DIR / "own-samantha.toml")
        self.jar = load_persona(PERSONA_DIR / "own-jarvis.toml")

    def test_resolve_aliases(self):
        self.assertEqual(resolve_persona_path("own-samantha"), PERSONA_DIR / "own-samantha.toml")
        self.assertEqual(resolve_persona_path("jarvis"), PERSONA_DIR / "own-jarvis.toml")

    def test_samantha_set(self):
        self.assertEqual(self.sam.id, "own-samantha")
        self.assertEqual(self.sam.gate, 0.83)
        self.assertTrue(self.sam.in_set("seductive"))
        self.assertTrue(self.sam.in_set("love"))
        self.assertTrue(self.sam.in_set("happy"))  # alias of joyful
        self.assertTrue(self.sam.in_set("joyful"))
        self.assertTrue(self.sam.in_set("aggressive"))
        self.assertTrue(self.sam.in_set("laughter"))
        self.assertTrue(self.sam.in_set("logic"))
        self.assertTrue(self.sam.in_set("neutral"))
        self.assertFalse(self.sam.in_set("yelling"))
        self.assertFalse(self.sam.in_set("crying"))

    def test_jarvis_seductive_is_na(self):
        self.assertEqual(self.jar.id, "own-jarvis")
        self.assertFalse(self.jar.in_set("seductive"))
        rec = {"e2v": {"sad": 0.0, "neutral": 1.0}, "clap_probs": {"seductive": 0.5, "logic": 0.8, "love": 0.4}}
        self.assertEqual(strength("seductive", rec, persona=self.jar), NA)
        self.assertEqual(strength("love", rec, persona=self.jar), NA)
        self.assertEqual(strength("yelling", rec, persona=self.jar), NA)
        view = apply_persona_scores(rec, persona=self.jar)
        self.assertEqual(view["persona_scores"]["seductive"], NA)
        self.assertEqual(view["persona_scores"]["love"], NA)
        self.assertNotEqual(view["persona_scores"]["logic"], NA)
        self.assertNotEqual(view["persona_scores"]["neutral"], NA)

    def test_out_of_list_is_na_not_zero(self):
        rec = {"efficientat_max": {"yelling": 0.79, "crying": 0.2}, "e2v": {"sad": 0.9, "angry": 0.99}}
        self.assertEqual(strength("yelling", rec, persona=self.sam), NA)
        self.assertEqual(strength("crying", rec, persona=self.sam), NA)
        self.assertIsInstance(strength("aggressive", rec, persona=self.sam), float)

    def test_targets_are_exact(self):
        by = {e.id: e for e in self.sam.emotions}
        self.assertEqual(by["aggressive"].target, 0.9990363121032715)
        self.assertEqual(by["love"].target, 0.8109382317899566)
        self.assertEqual(by["laughter"].target, 0.2743651270866394)
        self.assertEqual(by["logic"].target, 0.49966762061927383)
        self.assertEqual(by["neutral"].target, 1.0)
        self.assertEqual(by["joyful"].target, 0.9999996423721313)
        self.assertEqual(by["seductive"].target, 0.33438899274060635)
        jby = {e.id: e for e in self.jar.emotions}
        self.assertEqual(jby["neutral"].target, 1.0)
        self.assertEqual(jby["logic"].target, 0.8276575418335476)
        self.assertNotIn("seductive", jby)


class CryingDetectorTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.summary = json.loads(SUMMARY.read_text())
        cls.lines = cls.summary["lines"]
        cls.det = CryingDetector()

    def test_decision_equals_e2v_sad(self):
        for ln in self.lines:
            scored = self.det.score(ln)["crying"]
            self.assertEqual(scored.value, ln["e2v"]["sad"])
            self.assertEqual(scored.source, "trained class (emotion2vec+ sad)")
            self.assertTrue(scored.used_for_decision)
            self.assertFalse(scored.extras["efficientat_crying_used_for_decision"])
            self.assertEqual(scored.extras["efficientat_crying_logged"], ln["efficientat_max"]["crying"])
            self.assertNotEqual(scored.value, ln["efficientat_max"]["crying"])

    def test_decision_scores_prefers_e2v_sad_over_eat(self):
        # a record where eat crying is high and e2v sad is ~0 (Jarvis neu01)
        ln = next(x for x in self.lines if x["persona"] == "jarvis" and x["id"] == "neu01")
        # Core detectors only: do not invoke a love_seductive plugin (may load CLAP).
        dets = load_detectors(plugin_dir=Path(tempfile.mkdtemp()), include_legacy_love_seductive_fallback=True)
        scored = decision_scores(ln, dets)["crying"]
        self.assertEqual(scored.value, ln["e2v"]["sad"])
        self.assertLess(scored.value, 0.20)
        self.assertGreaterEqual(ln["efficientat_max"]["crying"], 0.20)

    def _rate(self, xs, pred):
        n = sum(1 for x in xs if pred(x))
        return n, len(xs), n / len(xs)

    def test_probe_detection_and_false_positives(self):
        """Exact counts against SUMMARY.json — do not round."""
        lines = self.lines
        cry = [ln for ln in lines if ln["group"] == "crying"]
        non = [ln for ln in lines if ln["group"] != "crying"]
        nonsad = [ln for ln in non if ln["group"] != "sad"]

        def e2v_hit(ln):
            return self.det.score(ln)["crying"].value >= PRESENCE

        def eat_hit(ln):
            return ln["efficientat_max"]["crying"] >= PRESENCE

        new_tp = self._rate(cry, e2v_hit)
        old_tp = self._rate(cry, eat_hit)
        new_fp = self._rate(non, e2v_hit)
        old_fp = self._rate(non, eat_hit)
        new_fp_strict = self._rate(nonsad, e2v_hit)
        old_fp_strict = self._rate(nonsad, eat_hit)

        self.assertEqual(new_tp, (5, 8, 5 / 8))
        self.assertEqual(old_tp, (0, 8, 0.0))
        self.assertEqual(new_fp, (4, 38, 4 / 38))
        self.assertEqual(old_fp, (6, 38, 6 / 38))
        self.assertEqual(new_fp_strict, (2, 34, 2 / 34))
        self.assertEqual(old_fp_strict, (4, 34, 4 / 34))

        fp_ids = sorted(
            f"{ln['persona']}/{ln['id']}/{ln['group']}"
            for ln in non if e2v_hit(ln)
        )
        self.assertEqual(fp_ids, [
            "jarvis/sur02/surprise",
            "samantha/sad01/sad",
            "samantha/sad02/sad",
            "samantha/sur01/surprise",
        ])
        # cry04 (both voices) and jarvis cry03/cry04 stay below presence — synth misses, not detector invert
        sam_cry = [ln for ln in cry if ln["persona"] == "samantha"]
        jar_cry = [ln for ln in cry if ln["persona"] == "jarvis"]
        self.assertEqual(self._rate(sam_cry, e2v_hit), (3, 4, 0.75))
        self.assertEqual(self._rate(jar_cry, e2v_hit), (2, 4, 0.5))


class PluginHook(unittest.TestCase):
    def test_legacy_fallback_when_no_plugin(self):
        dets = load_detectors(plugin_dir=Path(tempfile.mkdtemp()), include_legacy_love_seductive_fallback=True)
        names = [d.name for d in dets]
        self.assertIn("crying-e2v-sad", names)
        self.assertTrue(any(isinstance(d, LegacyClapLoveSeductiveFallback) for d in dets))
        rec = {"clap_probs": {"love": 0.31, "seductive": 0.22}}
        scored = decision_scores(rec, dets)
        self.assertAlmostEqual(scored["love"].value, 0.31)
        self.assertIn("legacy-clap-fallback", scored["love"].source)

    def test_plugin_replaces_fallback(self):
        td = Path(tempfile.mkdtemp())
        (td / "love_seductive.py").write_text(
            "from detectors.base import DetectorScore\n"
            "class LoveSeductiveDetector:\n"
            "    name = 'love-seductive-plugin'\n"
            "    categories = ('love', 'seductive')\n"
            "    def score(self, rec):\n"
            "        return {\n"
            "            'love': DetectorScore('love', 0.77, 'plugin-test'),\n"
            "            'seductive': DetectorScore('seductive', 0.66, 'plugin-test'),\n"
            "        }\n"
        )
        plugins = discover_love_seductive(td)
        self.assertEqual(len(plugins), 1)
        dets = load_detectors(plugin_dir=td)
        names = [d.name for d in dets]
        self.assertIn("love-seductive-plugin", names)
        self.assertFalse(any(isinstance(d, LegacyClapLoveSeductiveFallback) for d in dets))
        scored = decision_scores({}, dets)
        self.assertEqual(scored["love"].value, 0.77)
        self.assertEqual(scored["love"].source, "plugin-test")


class GatePersonaAware(unittest.TestCase):
    def test_out_of_set_moment_is_dropped(self):
        sam = load_persona(PERSONA_DIR / "own-samantha.toml")
        # yelling is high but out of set; neutral is in set
        win_yell = {
            "id": "w_yell",
            "e2v": {"neutral": 0.1, "angry": 0.1, "sad": 0.0, "happy": 0.0,
                    "disgusted": 0.0, "fearful": 0.0, "surprised": 0.0,
                    "other": 0.0, "unknown": 0.0},
            "efficientat_max": {"yelling": 0.9, "laughter": 0.1, "crying": 0.1},
        }
        win_neu = {
            "id": "w_neu",
            "e2v": {"neutral": 1.0, "angry": 0.0, "sad": 0.0, "happy": 0.0,
                    "disgusted": 0.0, "fearful": 0.0, "surprised": 0.0,
                    "other": 0.0, "unknown": 0.0},
            "efficientat_max": {"yelling": 0.1, "laughter": 0.1, "crying": 0.1},
        }
        line = {
            "id": "t01",
            "e2v": {"neutral": 1.0, "angry": 0.0, "sad": 0.0, "happy": 0.0,
                    "disgusted": 0.0, "fearful": 0.0, "surprised": 0.0,
                    "other": 0.0, "unknown": 0.0,
                    "embedding": [1.0, 0.0, 0.0]},
            "efficientat_max": {"yelling": 0.05, "laughter": 0.1, "crying": 0.1},
        }
        win_neu = dict(win_neu)
        win_neu["e2v"] = dict(win_neu["e2v"])
        win_neu["e2v"]["embedding"] = [1.0, 0.0, 0.0]
        moments = [
            {"category": "yelling", "strength": 0.9, "window": win_yell},
            {"category": "neutral", "strength": 1.0, "window": win_neu},
        ]
        out = emotion_match(line_recs=[line], moments=moments, persona=sam)
        cats = [r["category"] for r in out["leaderboard"]]
        self.assertNotIn("yelling", cats)
        self.assertIn("neutral", cats)
        self.assertIn("yelling", out["na_categories"])
        self.assertGreaterEqual(out["emotion_match"], 0.83)
        self.assertTrue(out["pass"])

    def test_category_score_weights(self):
        # 0.55*max + 0.45*mean
        self.assertAlmostEqual(category_score([1.0, 0.0]), 0.55 * 1.0 + 0.45 * 0.5)


if __name__ == "__main__":
    unittest.main()
