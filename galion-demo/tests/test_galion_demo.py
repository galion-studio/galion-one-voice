# A14 — unit tests for the canned demo policy ports (MIT). Run: python3 -m unittest discover -s tests
import os, sys, tempfile, unittest, json
sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
import galion_demo as g

class Counterbalance(unittest.TestCase):
    def test_selftest(self): self.assertTrue(g.selftest())
    def test_angry_is_deescalated(self):
        c = g.vad(-0.7, 0.8, 0.6, 0.3); t = g.respond(c)
        self.assertEqual(g.label(c), "Angry"); self.assertLess(t["a"], c["a"]); self.assertLess(t["d"], c["d"])
        self.assertIsNone(g.check_counterbalance(c, t)); self.assertIsNotNone(g.check_counterbalance(c, g.mirror(c)))
    def test_joy_is_shared_slightly_below(self):
        c = g.vad(0.8, 0.8, 0.5, 0.9); t = g.respond(c)
        self.assertLess(t["v"], c["v"]); self.assertGreaterEqual(t["v"], 0.7)
    def test_no_consent_is_neutral_warm(self):
        self.assertEqual(g.respond(None), g.vad(*g.NEUTRAL_WARM, c=1.0))

class Gating(unittest.TestCase):
    def test_default_deny_and_prohibited(self):
        self.assertFalse(g.consent_allows(0, "personal")[0])
        for p in g.PROHIBITED: self.assertEqual(g.consent_allows(g.SENSE | g.ADAPT, p), (False, "DenyProhibitedPurpose"))
        self.assertTrue(g.consent_allows(g.SENSE | g.ADAPT, "personal")[0])

class EndToEnd(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.d = tempfile.mkdtemp(); cls.inp = os.path.join(cls.d, "in.wav")
        g.write_wav(cls.inp, [g.gen_input()])
    def _run(self, tag, **kw):
        o = os.path.join(self.d, tag)
        res, kd = g.run(self.inp, o + ".wav", o + ".txt", canned=True, kd_out=o + ".jsonl", **kw)
        return dict((r[0], r[1]) for r in res), kd, open(o + ".wav", "rb").read(), open(o + ".jsonl").read()
    def test_opener_and_hard_off(self):
        dres, dkd, dwav, _ = self._run("default")
        ores, okd, owav, ojl = self._run("opener", bits=g.SENSE | g.ADAPT, purpose="personal", rika_opener=True)
        wres, _, wwav, _ = self._run("work", bits=g.SENSE | g.ADAPT, purpose="workplace", rika_opener=True)
        eres, _, ewav, _ = self._run("edu", bits=g.SENSE | g.ADAPT, purpose="education", rika_opener=True)
        self.assertEqual(dres["E-Rika-opener"], "SKIP"); self.assertEqual(ores["E-Rika-opener"], "PASS")
        self.assertEqual(ores["F-counterbalance"], "PASS")
        self.assertEqual(wwav, dwav); self.assertEqual(ewav, dwav); self.assertNotEqual(owav, dwav)
        self.assertEqual(wres["E-Rika-opener"], "SKIP"); self.assertEqual(eres["E-Rika-opener"], "SKIP")
        rows = [json.loads(l) for l in ojl.splitlines()]
        self.assertEqual(len(rows), len(okd))
        self.assertTrue(all(r["teacher_output"] is None for r in rows))  # no teacher run, nothing invented
        self.assertEqual({r["spec"] for r in rows}, {"E-in-A-out", "D-language", "emotion-ser"})
    def test_deterministic(self):
        a = self._run("a", bits=g.SENSE | g.ADAPT, rika_opener=True); b = self._run("b", bits=g.SENSE | g.ADAPT, rika_opener=True)
        self.assertEqual(a[2], b[2]); self.assertEqual(a[3], b[3])

if __name__ == "__main__": unittest.main()
