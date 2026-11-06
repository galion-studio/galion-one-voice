"""Opt-in hook tests for cbx-add-voice. No model load, no real renders."""
import json, os, pathlib, subprocess, tempfile, unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
WRAPPER = ROOT / "bin/cbx-add-voice"


class Wrapper(unittest.TestCase):
    def setUp(self):
        self.work = pathlib.Path(tempfile.mkdtemp(prefix="tgt-wrapper-", dir="/tmp"))
        self.log = self.work / "calls.jsonl"
        stub = self.work / "stub"
        stub.write_text(
            "#!/usr/bin/env python3\nimport os,sys,json\n"
            "with open(os.environ['CAPTURE'],'a') as f: f.write(json.dumps(sys.argv)+'\\n')\n"
        )
        stub.chmod(0o755)
        self.env = {
            **os.environ,
            "CAPTURE": str(self.log),
            "HF_HOME": str(self.work / "cache"),
            "CBX": str(stub),
            "CBX_BLEND": str(stub),
            "BLEND_REF": str(stub),
            "EMOTION_SCORER": str(stub),
            "EMOTION_TARGETS": str(stub),
        }
        self.args = ["--name", "own-unit-v2-new-1", "--voice-wav", "original.wav", "--dtype", "fp32"]

    def runw(self, *args):
        return subprocess.run([str(WRAPPER), *args], env=self.env, capture_output=True, text=True)

    def calls(self):
        if not self.log.exists():
            return []
        return [json.loads(x) for x in self.log.read_text().splitlines()]

    def test_off_does_not_invoke_targets(self):
        r = self.runw(*self.args)
        self.assertEqual(r.returncode, 0, r.stderr)
        calls = self.calls()
        self.assertEqual(len(calls), 1)
        self.assertEqual(calls[0][1:], ["--intra-threads", "2", "--inter-threads", "2", "voice", "add", *self.args])

    def test_flag_requires_work_dir(self):
        r = self.runw("--missing-emotion-targets", *self.args)
        self.assertNotEqual(r.returncode, 0)
        self.assertIn("targets-work-dir", r.stderr + r.stdout)

    def test_flag_runs_targets_then_voice_add(self):
        work = self.work / "targets"
        r = self.runw("--missing-emotion-targets", "--targets-work-dir", str(work), *self.args)
        self.assertEqual(r.returncode, 0, r.stderr)
        calls = self.calls()
        self.assertEqual(len(calls), 2)
        self.assertIn("--voice", calls[0])
        self.assertIn("--original", calls[0])
        self.assertIn("--out-dir", calls[0])
        self.assertNotIn("--scorer", calls[0])  # default remains universal; flag omitted
        self.assertEqual(calls[1][1:], ["--intra-threads", "2", "--inter-threads", "2", "voice", "add", *self.args])

    def test_config_opt_in_and_explicit_disable(self):
        cfg = self.work / "config.json"
        cfg.write_text('{"missing_emotion_targets":{"enabled":true}}')
        r = self.runw("--blend-config", str(cfg), "--no-missing-emotion-targets", *self.args)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(len(self.calls()), 1)

    def test_protected_name_fails_before_targets(self):
        r = self.runw(
            "--missing-emotion-targets",
            "--targets-work-dir",
            str(self.work / "t"),
            "--name",
            "own-samantha",
            "--voice-wav",
            "original.wav",
        )
        self.assertNotEqual(r.returncode, 0)
        self.assertFalse(self.log.exists())

    def test_targets_scorer_persona_is_forwarded(self):
        work = self.work / "targets-p"
        r = self.runw(
            "--missing-emotion-targets",
            "--targets-work-dir",
            str(work),
            "--targets-scorer",
            "persona",
            "--targets-persona-file",
            str(self.work / "samantha.toml"),
            *self.args,
        )
        self.assertEqual(r.returncode, 0, r.stderr)
        calls = self.calls()
        self.assertEqual(len(calls), 2)
        self.assertIn("--scorer", calls[0])
        self.assertEqual(calls[0][calls[0].index("--scorer") + 1], "persona")
        self.assertIn("--persona-file", calls[0])
        self.assertEqual(calls[1][1:], ["--intra-threads", "2", "--inter-threads", "2", "voice", "add", *self.args])

    def test_config_scorer_persona(self):
        cfg = self.work / "config.json"
        persona = self.work / "own-unit.toml"
        persona.write_text("[persona]\n")
        cfg.write_text(json.dumps({
            "missing_emotion_targets": {
                "enabled": True,
                "scorer": "persona",
                "persona_file": str(persona),
            }
        }))
        r = self.runw("--blend-config", str(cfg), "--targets-work-dir", str(self.work / "t2"), *self.args)
        self.assertEqual(r.returncode, 0, r.stderr)
        calls = self.calls()
        self.assertEqual(calls[0][calls[0].index("--scorer") + 1], "persona")
        self.assertIn(str(persona), calls[0])

    def test_scorer_persona_not_forwarded_to_cbx_on_default_path(self):
        r = self.runw("--scorer", "persona", *self.args)
        self.assertEqual(r.returncode, 0, r.stderr)
        calls = self.calls()
        self.assertEqual(len(calls), 1)
        self.assertEqual(calls[0][1:], ["--intra-threads", "2", "--inter-threads", "2", "voice", "add", *self.args])


if __name__ == "__main__":
    unittest.main()
