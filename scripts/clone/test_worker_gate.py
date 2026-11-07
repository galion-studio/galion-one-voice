#!/usr/bin/env python3
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Gate tests for the Chatterbox Nano worker. No weights, no torch, no audio."""

from __future__ import annotations

import importlib.util
import json
import os
import struct
import subprocess
import sys
import tempfile
import threading
import time
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
WORKER = HERE / "chatterbox_worker.py"


def load_worker():
    spec = importlib.util.spec_from_file_location("galion_chatterbox_worker", WORKER)
    if spec is None or spec.loader is None:
        raise RuntimeError("worker module missing")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


MOD = load_worker()


def child_env(**extra: str) -> dict[str, str]:
    env = os.environ.copy()
    for key in (
        "GALION_CLONE_STUB_PCM",
        "GALION_CLONE_LOCAL_REFS",
        "GALION_CLONE_WEIGHTS",
        "GALION_CLONE_CKPT",
        "GALION_CLIP_STORE",
        "GALION_CLONE_MODEL",
    ):
        env.pop(key, None)
    env.update(extra)
    return env


def run_worker(payload: dict, env: dict[str, str]) -> subprocess.CompletedProcess[bytes]:
    return subprocess.run(
        [sys.executable, str(WORKER)],
        input=(json.dumps(payload) + "\n").encode(),
        capture_output=True,
        env=env,
        timeout=15,
        check=False,
    )


class AllowlistTests(unittest.TestCase):
    def test_documented_prefixes_are_the_file(self) -> None:
        allow = MOD.load_allowlist(str(HERE / "REFS.sha256"))
        # The allowlist reads the active REFS row's full digest as the prefix
        # with an empty suffix. samantha-exp's active row is the proposed
        # Amy Koenig 17.84-29.25 s cut. The previous CC0 row stays commented.
        self.assertTrue(allow["samantha"][0].startswith("16364282"))
        self.assertTrue((allow["samantha"][0] + allow["samantha"][1]).endswith("921005"))
        self.assertTrue(allow["jarvis"][0].startswith("60ad5457"))
        self.assertTrue((allow["jarvis"][0] + allow["jarvis"][1]).endswith("58b8f6b6"))
        self.assertEqual(set(allow), {"samantha", "jarvis"})

    def test_persona_binding(self) -> None:
        # Synthetic digests against a prefix/suffix allowlist (the 3-column form
        # the worker still accepts).
        allow = {"samantha": ("bdcb0c29", "c7a8693"), "jarvis": ("c2200b59", "4b532c60")}
        samantha = "bdcb0c29" + ("ab" * 24) + "a" + "c7a8693"
        jarvis = "c2200b59" + ("ab" * 24) + "4b532c60"
        self.assertEqual(len(samantha), 64)
        self.assertEqual(len(jarvis), 64)
        self.assertTrue(MOD.digest_allowed("samantha", samantha.upper(), allow))
        self.assertTrue(MOD.digest_allowed("jarvis", jarvis, allow))
        self.assertFalse(MOD.digest_allowed("samantha", jarvis, allow))
        self.assertFalse(MOD.digest_allowed("jarvis", samantha, allow))
        self.assertFalse(MOD.digest_allowed("samantha", "ab" * 32, allow))
        self.assertFalse(MOD.digest_allowed("samantha", samantha[:-1], allow))

    def test_worker_does_not_download_or_call_from_pretrained(self) -> None:
        src = WORKER.read_text(encoding="utf-8")
        self.assertNotIn("from_pretrained", src)
        self.assertNotIn("snapshot_download", src)
        self.assertNotIn("huggingface.co", src)
        self.assertIn("nano=True", src)
        self.assertIn("audio_prompt_path", src)


class FramingTests(unittest.TestCase):
    def test_marker_is_flushed_before_a_large_pcm_write(self) -> None:
        err_r, err_w = os.pipe()
        pcm_r, pcm_w = os.pipe()
        os.set_blocking(err_r, False)
        payload = b"\x00\x01" * 100_000
        old_err = sys.stderr

        def run() -> None:
            sys.stderr = os.fdopen(err_w, "w", buffering=1, closefd=True)
            try:
                MOD.frame_utterance(pcm_w, payload, "chatterbox-nano", "samantha")
            finally:
                sys.stderr.flush()
                sys.stderr.close()
                os.close(pcm_w)
                sys.stderr = old_err

        thread = threading.Thread(target=run)
        thread.start()
        err = b""
        deadline = time.time() + 5
        while b"galion-clone-utt:" not in err and time.time() < deadline:
            try:
                chunk = os.read(err_r, 4096)
            except BlockingIOError:
                time.sleep(0.01)
                continue
            if not chunk:
                break
            err += chunk
        self.assertIn(b"galion-clone-utt: %d" % len(payload), err)
        self.assertIn(b"galion-clone-engine: chatterbox-nano", err)
        os.set_blocking(pcm_r, True)
        got = b""
        while len(got) < len(payload):
            chunk = os.read(pcm_r, 65536)
            if not chunk:
                break
            got += chunk
        thread.join(timeout=5)
        os.close(err_r)
        os.close(pcm_r)
        self.assertFalse(thread.is_alive())
        self.assertEqual(got, payload)
        self.assertNotIn(b"galion", got)


class ProcessTests(unittest.TestCase):
    def test_stub_pcm_stays_on_stdout_and_logs_on_stderr(self) -> None:
        text = "Hi."
        n = len(text)
        tone = 1000 + (n % 50)
        expect = struct.pack("<h", n) + struct.pack("<200h", *([tone] * 200))
        proc = run_worker(
            {"text": text, "persona": "samantha", "model": "nano", "tags": True},
            child_env(GALION_CLONE_STUB_PCM="1"),
        )
        self.assertEqual(proc.returncode, 0, proc.stderr.decode())
        self.assertEqual(proc.stdout, expect)
        self.assertNotIn(b"galion", proc.stdout)
        self.assertNotIn(b"Perth", proc.stdout)
        err = proc.stderr.decode()
        self.assertIn(f"galion-clone-utt: {len(expect)}", err)
        self.assertIn("galion-clone-engine: protocol-stub", err)
        self.assertIn("galion-clone-persona: samantha", err)

    def test_stub_still_refuses_a_reference(self) -> None:
        proc = run_worker(
            {"text": "hi", "reference": "clip.wav"},
            child_env(GALION_CLONE_STUB_PCM="1", GALION_CLONE_LOCAL_REFS="1"),
        )
        self.assertEqual(proc.returncode, 3, proc.stderr.decode())
        self.assertEqual(proc.stdout, b"")
        self.assertIn("rights hold", proc.stderr.decode())

    def test_jarvis_stub_strips_bracket_tags(self) -> None:
        text = "hello [laugh] there"
        stripped = "hello there"
        n = len(stripped)
        tone = 1000 + (n % 50)
        expect = struct.pack("<h", n) + struct.pack("<200h", *([tone] * 200))
        proc = run_worker(
            {"text": text, "persona": "jarvis", "tags": False},
            child_env(GALION_CLONE_STUB_PCM="1"),
        )
        self.assertEqual(proc.returncode, 0, proc.stderr.decode())
        self.assertEqual(proc.stdout, expect)

    def test_missing_weights_exit_absent_even_with_the_ref_gate(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            env = child_env(
                GALION_CLONE_WEIGHTS=tmp,
                GALION_CLONE_LOCAL_REFS="1",
            )
            proc = run_worker(
                {"text": "Hi.", "persona": "samantha", "audio_prompt_path": "/tmp/nope.wav"},
                env,
            )
        self.assertEqual(proc.returncode, 2, proc.stderr.decode())
        self.assertEqual(proc.stdout, b"")
        self.assertIn("galion-clone-absent", proc.stderr.decode())
        self.assertNotIn("rights hold", proc.stderr.decode())

    def test_weights_without_the_env_stay_unwired_and_still_refuse(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            for rel in MOD.NANO_RELS:
                path = root / rel
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(b"")
            unwired = run_worker(
                {"text": "Hi.", "persona": "samantha", "model": "nano"},
                child_env(GALION_CLONE_WEIGHTS=str(root)),
            )
            refused = run_worker(
                {"text": "Hi.", "audio_prompt_path": str(root / "x.wav")},
                child_env(GALION_CLONE_WEIGHTS=str(root)),
            )
        self.assertEqual(unwired.returncode, 0, unwired.stderr.decode())
        self.assertEqual(unwired.stdout, b"")
        self.assertIn("galion-clone-unwired", unwired.stderr.decode())
        self.assertIn("galion-clone-utt: 0", unwired.stderr.decode())
        self.assertEqual(refused.returncode, 3)
        self.assertEqual(refused.stdout, b"")
        self.assertIn("rights hold", refused.stderr.decode())

    def test_local_refs_reject_a_sha_that_is_not_allowlisted(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            for rel in MOD.NANO_RELS:
                path = root / rel
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(b"")
            store = root / "store"
            (store / "samantha").mkdir(parents=True)
            wav = store / "samantha" / "reference.wav"
            wav.write_bytes(b"RIFF----WAVEnot-the-allowlisted-clip")
            env = child_env(
                GALION_CLONE_WEIGHTS=str(root),
                GALION_CLONE_LOCAL_REFS="1",
                GALION_CLIP_STORE=str(store),
            )
            proc = run_worker(
                {"text": "Hi.", "persona": "samantha", "model": "nano"},
                env,
            )
        err = proc.stderr.decode()
        self.assertEqual(proc.returncode, 3, err)
        self.assertEqual(proc.stdout, b"")
        self.assertIn("not allowlisted", err)
        self.assertNotIn("Traceback", err)
        self.assertNotIn("torch", err.lower())

    def test_sidecar_mismatch_refuses(self) -> None:
        allow = MOD.load_allowlist()
        with tempfile.TemporaryDirectory() as tmp:
            wav = Path(tmp) / "reference.wav"
            wav.write_bytes(b"hello-clip")
            side = Path(str(wav) + ".sha256")
            side.write_text("ab" * 32 + "\n", encoding="utf-8")
            old = sys.stderr
            buf = __import__("io").StringIO()
            sys.stderr = buf
            try:
                with self.assertRaises(SystemExit) as raised:
                    MOD.resolve_allowlisted(
                        "samantha", [("audio_prompt_path", str(wav))], allow
                    )
            finally:
                sys.stderr = old
            self.assertEqual(raised.exception.code, 3)
            self.assertIn("sidecar", buf.getvalue())

    def test_clip_store_does_not_fall_through_when_set(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            store = root / "store"
            (store / "samantha").mkdir(parents=True)
            wav = store / "samantha" / "reference.wav"
            wav.write_bytes(b"clip")
            old = os.environ.get("GALION_CLIP_STORE")
            try:
                os.environ["GALION_CLIP_STORE"] = str(store)
                self.assertEqual(MOD.default_ref("samantha"), str(wav))
                os.environ["GALION_CLIP_STORE"] = str(root / "empty")
                (root / "empty").mkdir()
                self.assertIsNone(MOD.default_ref("samantha"))
            finally:
                if old is None:
                    os.environ.pop("GALION_CLIP_STORE", None)
                else:
                    os.environ["GALION_CLIP_STORE"] = old

    def test_tags_flag_strips_only_when_false(self) -> None:
        self.assertEqual(
            MOD.prepare_text("Power [alarm] on.", {"tags": False}),
            "Power on.",
        )
        self.assertEqual(
            MOD.prepare_text("Stay [chuckle] here.", {"tags": True}),
            "Stay [chuckle] here.",
        )
        self.assertIn("[chuckle]", MOD.prepare_text("Stay [chuckle] here.", {}))


if __name__ == "__main__":
    unittest.main()
