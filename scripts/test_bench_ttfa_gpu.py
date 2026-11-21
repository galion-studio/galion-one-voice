#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
"""The GPU bench writes clocks only from a child, and skips CUDA when no GPU is visible."""

from __future__ import annotations

import json
import os
import stat
import subprocess
import tempfile
import unittest
from pathlib import Path

import bench_ttfa_gpu
from device import Hardware

ROOT = Path(__file__).resolve().parents[1]
FAKE = r"""#!/usr/bin/env python3
import json, os, sys
asked = os.environ.get("GALION_TTS_DEVICE", "cpu").strip().lower()
mode = os.environ.get("GALION_FAKE_CUDA", "")
if asked == "cuda" and mode != "ok":
    sys.stderr.write("[galion-piper-rt] GALION_TTS_DEVICE=cuda but CUDA is not available (fake probe); falling back to CPU\n")
    sys.stderr.write("[galion-piper-rt] device=cpu\n")
elif asked == "cuda":
    sys.stderr.write("[galion-piper-rt] device=cuda provider=CUDAExecutionProvider\n")
else:
    sys.stderr.write("[galion-piper-rt] device=cpu\n")
sys.stderr.flush()
for line in sys.stdin:
    if not line.strip():
        continue
    json.loads(line)
    pcm = bytearray()
    for i in range(200):
        pcm += int((i * 100) % 30000).to_bytes(2, "little", signed=True)
    sys.stdout.buffer.write(pcm)
    sys.stdout.buffer.flush()
    sys.stderr.write("[galion-piper-rt] [info] Real-time factor: 0.1 (infer=0.01 sec, audio=0.1 sec)\n")
    sys.stderr.flush()
"""


def pack(directory: Path, runtime_name: str) -> tuple[Path, Path, Path]:
    runtime = directory / runtime_name
    runtime.write_text(FAKE)
    runtime.chmod(runtime.stat().st_mode | stat.S_IEXEC)
    model = directory / "hello.onnx"
    model.write_bytes(b"x")
    config = directory / "hello.onnx.json"
    config.write_text(json.dumps({"audio": {"sample_rate": 22050}}))
    return runtime, model, config


class Bench(unittest.TestCase):
    def test_absent_pack_has_no_milliseconds(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp) / "out"
            hidden = Hardware(False, False, False, False)
            report = bench_ttfa_gpu.run_bench(out, runtime=Path(tmp) / "missing", model=Path(tmp) / "missing.onnx", config=Path(tmp) / "missing.onnx.json", hardware=hidden)
            bench_ttfa_gpu.assert_honest(report)
            self.assertFalse(report["cpu"]["measured"])
            self.assertTrue(report["cuda"]["skipped"])
            self.assertFalse(report["cuda"]["measured"])
            self.assertEqual(bench_ttfa_gpu.timing_keys(report["cuda"]), [])
            self.assertEqual(bench_ttfa_gpu.timing_keys(report["cpu"]), [])
            self.assertFalse(report["product_claim"])

    def test_cpu_clock_comes_from_the_child_and_cuda_is_skipped(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            runtime, model, config = pack(root, "piper")
            out = root / "out"
            hidden = Hardware(False, False, False, False)
            report = bench_ttfa_gpu.run_bench(out, runtime=runtime, model=model, config=config, hardware=hidden, warm_reps=2)
            bench_ttfa_gpu.assert_honest(report)
            self.assertTrue(report["cpu"]["measured"])
            self.assertGreater(report["cpu"]["cold_pcm_bytes"], 0)
            self.assertEqual(len(report["cpu"]["warm_submit_to_first_pcm_ms"]), 2)
            self.assertGreaterEqual(min(report["cpu"]["warm_submit_to_first_pcm_ms"]), 0.0)
            cold = out / "cpu-cold.wav"
            self.assertTrue(cold.is_file())
            self.assertGreater(cold.stat().st_size, 44)
            self.assertFalse(report["cuda"]["measured"])
            self.assertTrue(report["cuda"]["skipped"])
            self.assertFalse((out / "cuda-cold.wav").exists())
            self.assertNotIn("cold_spawn_to_first_pcm_ms", report["cuda"])

    def test_cuda_fallback_is_not_recorded_as_cuda(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            runtime, model, config = pack(root, "galion-piper-rt")
            lib = root / "libonnxruntime.so.1.31.1"
            lib.write_bytes(b"not-a-library")
            out = root / "out"
            visible = Hardware(False, True, True, True)
            saved = os.environ.get("GALION_ORT_CUDA_LIB")
            saved_fake = os.environ.get("GALION_FAKE_CUDA")
            os.environ["GALION_ORT_CUDA_LIB"] = str(lib)
            os.environ.pop("GALION_FAKE_CUDA", None)
            try:
                report = bench_ttfa_gpu.run_bench(out, runtime=runtime, model=model, config=config, hardware=visible, warm_reps=1, repo=root)
            finally:
                if saved is None:
                    os.environ.pop("GALION_ORT_CUDA_LIB", None)
                else:
                    os.environ["GALION_ORT_CUDA_LIB"] = saved
                if saved_fake is None:
                    os.environ.pop("GALION_FAKE_CUDA", None)
                else:
                    os.environ["GALION_FAKE_CUDA"] = saved_fake
            bench_ttfa_gpu.assert_honest(report)
            self.assertFalse(report["cuda"]["measured"])
            self.assertIn("falling back to CPU", report["cuda"]["reason"])
            self.assertNotIn("cold_spawn_to_first_pcm_ms", report["cuda"])
            self.assertFalse((out / "cuda-warm.wav").exists())

    def test_cli_on_this_machine_does_not_claim_gpu(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp) / "logs"
            env = os.environ.copy()
            env["GALION_VOICE_TEST_DIR"] = str(out)
            env["GALION_A21_RUNTIME"] = str(Path(tmp) / "no-piper")
            env["GALION_PIPER_MODEL"] = str(Path(tmp) / "no-model.onnx")
            env.pop("GALION_TTS_WEIGHTS", None)
            env.pop("GALION_SPEECH_WEIGHTS", None)
            env.pop("GALION_A21_MODEL", None)
            proc = subprocess.run(
                ["bash", str(ROOT / "scripts" / "bench-ttfa-gpu.sh")],
                cwd=ROOT,
                env=env,
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(proc.returncode, 2, proc.stderr)
            report = json.loads((out / "ttfa-gpu.json").read_text())
            bench_ttfa_gpu.assert_honest(report)
            self.assertTrue(report["cuda"]["skipped"])
            self.assertFalse(report["cuda"]["measured"])
            if not report["cuda_visible"]:
                self.assertNotIn("cold_spawn_to_first_pcm_ms", report["cuda"])

    def test_pinned_voice_mismatch_is_unmeasured(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            runtime, _, _ = pack(root, "piper")
            model = root / "en_US-ljspeech-medium.onnx"
            model.write_bytes(b"not-the-pinned-voice")
            config = root / "en_US-ljspeech-medium.onnx.json"
            config.write_text("{}\n")
            out = root / "out"
            report = bench_ttfa_gpu.run_bench(out, runtime=runtime, model=model, config=config, hardware=Hardware(False, False, False, False), warm_reps=1)
            bench_ttfa_gpu.assert_honest(report)
            self.assertFalse(report["cpu"]["measured"])
            self.assertIn("sha256 mismatch", report["cpu"]["reason"])
            self.assertTrue(report["cuda"]["skipped"])
            self.assertEqual(bench_ttfa_gpu.timing_keys(report["cpu"]), [])
            self.assertEqual(bench_ttfa_gpu.timing_keys(report["cuda"]), [])
            self.assertFalse(any(out.glob("*.wav")))

    def test_unmeasured_cuda_milliseconds_are_rejected(self) -> None:
        report = {
            "product_claim": False,
            "cuda_visible": False,
            "cpu": {"measured": False, "skipped": False, "device": None},
            "cuda": {"measured": False, "skipped": True, "cold_spawn_to_first_pcm_ms": 1.0},
        }
        with self.assertRaises(SystemExit):
            bench_ttfa_gpu.assert_honest(report)

    def test_docs_do_not_publish_a_millisecond(self) -> None:
        import re
        text = (ROOT / "voice-stack" / "mouth" / "README.md").read_text()
        self.assertIsNone(re.search(r"\d+(?:\.\d+)?\s*ms", text))
        self.assertNotIn("ttfa_ms", text)


if __name__ == "__main__":
    unittest.main()
