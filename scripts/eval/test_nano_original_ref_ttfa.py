#!/usr/bin/env python3
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Judge tests for the original-ref Nano TTFA harness.

Fixtures are not a measurement of this host. No torch, no weights, no WAV.
"""

from __future__ import annotations

import hashlib
import importlib.util
import io
import json
import os
import sys
import unittest
from contextlib import redirect_stdout
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
MODULE = HERE / "nano_original_ref_ttfa.py"


def load():
    spec = importlib.util.spec_from_file_location("nano_original_ref_ttfa", MODULE)
    if spec is None or spec.loader is None:
        raise RuntimeError("harness module missing")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


MOD = load()


class JudgeTests(unittest.TestCase):
    def test_contract_holds(self) -> None:
        self.assertIsNone(MOD.contract_error())

    def test_fixtures_do_not_treat_a_stub_as_a_clock(self) -> None:
        for name, cap, gate, _expect in MOD.fixture_cases():
            row = MOD.score_capture(cap)
            self.assertEqual(row["gate"], gate, name)
            if gate != "MEASURED":
                for key in MOD.CLOCK_KEYS:
                    self.assertNotIn(key, row, name)
                self.assertFalse(row["product_claim"], name)
                self.assertFalse(row["claim_80ms"], name)
        stub = MOD.score_capture(MOD.fixture_cases()[0][1])
        self.assertTrue(stub["stub"])
        self.assertNotIn("15", json.dumps(stub))

    def test_80_ms_requires_both_originals_and_a_frame_inside_the_bar(self) -> None:
        fast = MOD.score_capture(MOD.fixture_cases()[4][1])
        jarvis = dict(fast)
        jarvis["persona"] = "jarvis"
        jarvis["ref_sha256"] = MOD.T273["jarvis"]
        jarvis["galion_clone_first"] = (
            "galion-clone-first: 40 persona=jarvis engine=chatterbox-nano"
        )
        both = MOD.combine([fast, jarvis], host="fixture")
        MOD.validate_public(both)
        self.assertEqual(both["gate"], "MEASURED")
        self.assertTrue(both["claim_80ms"])
        self.assertEqual(both["ttfa_ms"], 70)
        self.assertEqual(both["measured_file"]["schema"], MOD.FRAME_FILE)
        self.assertNotIn("onset_p50_ms", both["measured_file"])
        self.assertFalse(both["measured_file"]["stub"])

        slow = dict(jarvis)
        slow["ttfa_ms"] = 400
        slow["claim_80ms"] = False
        slow["first_pcm_ms"] = 120
        slow["galion_clone_first"] = (
            "galion-clone-first: 120 persona=jarvis engine=chatterbox-nano"
        )
        missed = MOD.combine([fast, slow], host="fixture")
        MOD.validate_public(missed)
        self.assertEqual(missed["gate"], "MEASURED")
        self.assertFalse(missed["claim_80ms"])
        self.assertEqual(missed["ttfa_ms"], 400)
        self.assertIsNone(missed["measured_file"])
        self.assertIn("80 ms is not claimed", missed["note"])

    def test_one_persona_cannot_close_the_row(self) -> None:
        fast = MOD.score_capture(MOD.fixture_cases()[4][1])
        report = MOD.combine([fast], host="fixture")
        MOD.validate_public(report)
        self.assertEqual(report["gate"], "BLOCKED")
        self.assertNotIn("ttfa_ms", report)
        self.assertFalse(report["claim_80ms"])

    def test_t273_constants_match_the_commented_ref_rows(self) -> None:
        text = (ROOT / "scripts" / "clone" / "REFS.sha256").read_text(encoding="utf-8")
        for persona, digest in MOD.T273.items():
            self.assertIn(f"# {persona} {digest} ", text)
            self.assertEqual(len(digest), 64)

    def test_synthetic_allowlist_blocks_until_t273_is_the_active_pin(self) -> None:
        worker = MOD.load_worker()
        # Prefix/suffix form: digest_allowed checks start and end. A full
        # digest is the prefix with an empty suffix.
        t623_prefix = {
            "samantha": ("e4bc60c3", "84a4641"),
            "jarvis": ("60ad5457", "58b8f6b6"),
        }
        for persona, digest in MOD.T273.items():
            self.assertFalse(worker.digest_allowed(persona, digest, t623_prefix))
            self.assertTrue(worker.digest_allowed(persona, digest, {persona: (digest, "") for persona in MOD.T273}))

    def test_child_env_strips_stub_and_ref_override(self) -> None:
        env = MOD.child_env(
            {
                "GALION_CLONE_STUB_PCM": "1",
                "GALION_CLONE_STUB_FRAMES": "1",
                "GALION_CLONE_REF_SHA": "samantha:" + MOD.T273["samantha"],
                "GALION_CLONE_REF_ALLOW": "/tmp/allow",
                "GALION_CLONE_FETCH": "1",
                "GALION_CLIP_STORE": "/keep",
            }
        )
        for key in MOD.STRIP_ENV:
            self.assertNotIn(key, env)
        self.assertEqual(env["GALION_CLONE_STREAM"], "1")
        self.assertEqual(env["GALION_CLONE_LOCAL_REFS"], "1")
        self.assertEqual(env["GALION_CLONE_MODEL"], "nano")
        self.assertEqual(env["GALION_CLIP_STORE"], "/keep")

    def test_source_does_not_assign_a_ref_override_or_a_stub(self) -> None:
        text = MODULE.read_text(encoding="utf-8")
        self.assertNotIn('GALION_CLONE_REF_SHA"] =', text)
        self.assertNotIn('GALION_CLONE_STUB_PCM"] =', text)
        self.assertNotIn("piper-stream-ttfa", text)

    def test_shell_does_not_call_piper_or_force_the_allowlist(self) -> None:
        shell = (HERE / "nano-original-ref-ttfa.sh").read_text(encoding="utf-8")
        self.assertNotIn("piper-stream-ttfa", shell)
        self.assertNotIn("GALION_CLONE_REF_SHA", shell)
        self.assertNotIn("GALION_CLONE_STUB_PCM", shell)
        self.assertNotRegex(shell, r">\s*LOCKS\.toml")
        self.assertNotRegex(shell, r">\s*scripts/clone/REFS\.sha256")

    def test_clock_constant_matches_the_lock_and_the_heartbeat(self) -> None:
        self.assertEqual(MOD.locked_frame_ms(), 80)
        text = MOD.HEARTBEAT.read_text(encoding="utf-8")
        self.assertIn("pub const FRAME_MS: u32 = 80;", text)
        self.assertEqual(MOD.FRAME_SAMPLES, 1920)
        self.assertEqual(MOD.FRAME_BYTES, 3840)

    def test_observe_clocks_a_fake_child_without_scoring_it(self) -> None:
        code = (
            "import sys,time\n"
            "sys.stdin.buffer.readline()\n"
            "sys.stderr.write('galion-clone-engine: chatterbox-nano\\n')\n"
            "sys.stderr.write('galion-clone-path: stream\\n')\n"
            "sys.stderr.write('galion-clone-chunk: 3840\\n')\n"
            "sys.stderr.write('galion-clone-first: 0\\n')\n"
            "sys.stderr.flush()\n"
            "sys.stdout.buffer.write(b'\\x00\\x10'*1920)\n"
            "sys.stdout.buffer.flush()\n"
            "time.sleep(0.03)\n"
            "sys.stderr.write('galion-clone-utt: 3840\\n')\n"
            "sys.stderr.flush()\n"
        )
        events = MOD.read_child([sys.executable, "-c", code], os.environ.copy(), b"\n", 5.0)
        cap = MOD.observe(events)
        self.assertEqual(cap["engine"], "chatterbox-nano")
        self.assertEqual(cap["path"], "stream")
        self.assertGreaterEqual(cap["chunks"], 1)
        self.assertIsNotNone(cap["first_pcm_ms"])
        self.assertIsNotNone(cap["first_playable_frame_ms"])
        self.assertTrue(cap["first_marker_before_utt"])
        self.assertGreater(cap["utt_ms"], cap["first_pcm_ms"])
        cap["persona"] = "samantha"
        cap["ref_sha256"] = "0" * 64
        row = MOD.score_capture(cap)
        self.assertEqual(row["gate"], "FAIL")
        self.assertNotIn("ttfa_ms", row)

    def test_spawn_env_is_the_stripped_child_env(self) -> None:
        seen = {}

        def spawn(env, line):
            seen["env"] = env
            seen["line"] = line
            pcm = MOD.varied_pcm()
            stderr = (
                "galion-clone-engine: chatterbox-nano\n"
                "galion-clone-path: stream\n"
                "galion-clone-chunk: %d\n" % len(pcm)
                + "galion-clone-first: 1\n"
                + "galion-clone-utt: %d\n" % len(pcm)
            ).encode()
            return [(10, "err", stderr), (40, "out", pcm), (80, "err", b"")]

        # Hash whatever ref the worker resolves. This host has none, so the
        # capture is not a t273 measurement. The assertion is the env.
        cap = MOD.capture_persona("samantha", spawn)
        self.assertNotIn("GALION_CLONE_STUB_PCM", seen["env"])
        self.assertNotIn("GALION_CLONE_REF_SHA", seen["env"])
        self.assertEqual(seen["env"]["GALION_CLONE_STREAM"], "1")
        payload = json.loads(seen["line"].decode())
        self.assertEqual(payload["persona"], "samantha")
        self.assertEqual(payload["text"], "Hi. I'm here, and I'm listening.")
        self.assertFalse(cap["stub"])

    def test_main_does_not_measure_when_this_tree_is_blocked(self) -> None:
        blockers = MOD.live_blockers()
        if not blockers:
            self.skipTest("this tree can measure; the blocked path is not the one under test")
        calls = []

        def measure():
            calls.append(1)
            raise AssertionError("worker was started")

        locks = (ROOT / "LOCKS.toml").read_bytes()
        refs = (ROOT / "scripts" / "clone" / "REFS.sha256").read_bytes()
        buf = io.StringIO()
        with redirect_stdout(buf):
            with self.assertRaises(SystemExit) as ctx:
                MOD.main(blockers_fn=lambda: blockers, measure_fn=measure)
        self.assertEqual(ctx.exception.code, 2)
        self.assertEqual(calls, [])
        doc = json.loads(buf.getvalue())
        MOD.validate_public(doc)
        self.assertEqual(doc["gate"], "BLOCKED")
        self.assertEqual(doc["measurement"], "not measured")
        self.assertFalse(doc["product_claim"])
        self.assertFalse(doc["claim_80ms"])
        self.assertNotIn("ttfa_ms", doc)
        self.assertFalse(doc["locks_edited"])
        self.assertEqual((ROOT / "LOCKS.toml").read_bytes(), locks)
        self.assertEqual((ROOT / "scripts" / "clone" / "REFS.sha256").read_bytes(), refs)

    def test_ready_measurements_are_classified_and_not_replaced_with_blocked(self) -> None:
        fast = MOD.score_capture(MOD.fixture_cases()[4][1])
        jarvis = dict(fast)
        jarvis["persona"] = "jarvis"
        jarvis["ref_sha256"] = MOD.T273["jarvis"]
        jarvis["galion_clone_first"] = (
            "galion-clone-first: 40 persona=jarvis engine=chatterbox-nano"
        )

        def measure():
            return [fast, jarvis]

        buf = io.StringIO()
        with redirect_stdout(buf):
            with self.assertRaises(SystemExit) as ctx:
                MOD.main(blockers_fn=lambda: [], measure_fn=measure)
        self.assertEqual(ctx.exception.code, 0)
        doc = json.loads(buf.getvalue())
        self.assertEqual(doc["gate"], "MEASURED")
        self.assertTrue(doc["claim_80ms"])
        self.assertEqual(doc["measurement"], "measured")


class BlockerUnitTests(unittest.TestCase):
    def test_hash_helper_does_not_match_a_different_file(self) -> None:
        digest = hashlib.sha256(b"not-a-wav").hexdigest()
        self.assertNotEqual(digest, MOD.T273["samantha"])
        self.assertNotEqual(digest, MOD.T273["jarvis"])


if __name__ == "__main__":
    unittest.main()
