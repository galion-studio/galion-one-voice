#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
"""Mouth device policy, loopback bind, and a fake Piper child. No GPU claim."""

from __future__ import annotations

import json
import os
import stat
import tempfile
import threading
import unittest
import urllib.request
import wave
from pathlib import Path

import device
import serve

FAKE = """#!/usr/bin/env python3
import json, os, sys
asked = os.environ.get("GALION_TTS_DEVICE", "cpu").strip().lower()
mode = os.environ.get("GALION_FAKE_CUDA", "")
if asked == "cuda" and mode != "ok":
    sys.stderr.write("[galion-piper-rt] GALION_TTS_DEVICE=cuda but CUDA is not available (fake probe); falling back to CPU\\n")
    sys.stderr.write("[galion-piper-rt] device=cpu\\n")
elif asked == "cuda":
    sys.stderr.write("[galion-piper-rt] device=cuda provider=CUDAExecutionProvider\\n")
else:
    sys.stderr.write("[galion-piper-rt] device=cpu\\n")
sys.stderr.flush()
for line in sys.stdin:
    if not line.strip():
        continue
    text = json.loads(line).get("text", "")
    pcm = bytearray()
    for i in range(200):
        pcm += int((i * 100) % 30000).to_bytes(2, "little", signed=True)
    sys.stdout.buffer.write(pcm)
    sys.stdout.buffer.flush()
    sys.stderr.write("[galion-piper-rt] [info] Real-time factor: 0.1 (infer=0.01 sec, audio=0.1 sec)\\n")
    sys.stderr.flush()
"""


def write_fake(directory: Path, name: str) -> Path:
    path = directory / name
    path.write_text(FAKE)
    path.chmod(path.stat().st_mode | stat.S_IEXEC)
    model = directory / "hello.onnx"
    model.write_bytes(b"not-an-onnx")
    (directory / "hello.onnx.json").write_text(json.dumps({"audio": {"sample_rate": 22050}}))
    return path


class DevicePolicy(unittest.TestCase):
    def test_cpu_is_default(self) -> None:
        for raw in (None, "", "cpu", "auto", " AUTO "):
            request, note = device.parse_request(raw)
            self.assertEqual(request, "cpu")
            self.assertIsNone(note)

    def test_unknown_falls_back(self) -> None:
        request, note = device.parse_request("vulkan")
        self.assertEqual(request, "cpu")
        self.assertIn("falling back to CPU", note or "")

    def test_cuda_needs_hardware_runtime_and_library(self) -> None:
        hidden = device.Hardware(True, True, True, True)
        visible = device.Hardware(False, True, True, True)
        runtime = Path("galion-piper-rt")
        lib = Path("libonnxruntime.so")
        self.assertFalse(device.plan_device("cuda", hardware=hidden, runtime=runtime, cuda_lib=lib).try_cuda)
        legacy = device.plan_device("cuda", hardware=visible, runtime=Path("piper"), cuda_lib=lib)
        self.assertEqual(legacy.device, "cpu")
        self.assertIn("falling back to CPU", legacy.log or "")
        missing = device.plan_device("cuda", hardware=visible, runtime=runtime, cuda_lib=None)
        self.assertIn("not on disk", missing.log or "")
        ready = device.plan_device("cuda", hardware=visible, runtime=runtime, cuda_lib=lib)
        self.assertTrue(ready.try_cuda)
        self.assertIsNone(ready.log)

    def test_bind_is_loopback_only(self) -> None:
        self.assertEqual(serve.parse_bind("127.0.0.1:8002"), ("127.0.0.1", 8002))
        self.assertEqual(serve.parse_bind("127.0.0.1:0"), ("127.0.0.1", 0))
        for raw in ("0.0.0.0:8002", "localhost:8002", "[::1]:8002", "192.0.2.1:8002", ""):
            with self.assertRaises(ValueError):
                serve.parse_bind(raw)


class Server(unittest.TestCase):
    def test_health_and_wav_with_fake_piper(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            runtime = write_fake(root, "piper")
            model = root / "hello.onnx"
            config = root / "hello.onnx.json"
            saved = {key: os.environ.get(key) for key in ("GALION_A21_RUNTIME", "GALION_PIPER_MODEL", "GALION_PIPER_CONFIG", "GALION_TTS_DEVICE", "GALION_TTS_WEIGHTS", "GALION_SPEECH_WEIGHTS", "GALION_A21_MODEL", "GALION_PIPER_LEGACY")}
            os.environ["GALION_A21_RUNTIME"] = str(runtime)
            os.environ["GALION_PIPER_MODEL"] = str(model)
            os.environ["GALION_PIPER_CONFIG"] = str(config)
            os.environ["GALION_TTS_DEVICE"] = "cuda"
            os.environ.pop("GALION_PIPER_LEGACY", None)
            try:
                hidden = device.Hardware(False, False, False, False)
                mouth = serve.build_mouth(root, "127.0.0.1:0", hardware=hidden)
                self.assertEqual(mouth.plan.device, "cpu")
                self.assertIn("falling back to CPU", mouth.plan.log or "")
                server = serve.serve_forever(mouth)
                thread = threading.Thread(target=server.serve_forever, daemon=True)
                thread.start()
                port = server.server_address[1]
                self.assertEqual(server.server_address[0], "127.0.0.1")
                with urllib.request.urlopen(f"http://127.0.0.1:{port}/health") as response:
                    health = json.loads(response.read().decode())
                self.assertTrue(health["ok"])
                self.assertEqual(health["device"], "cpu")
                self.assertFalse(health["product_claim"])
                self.assertNotIn("cold_spawn_to_first_pcm_ms", health)
                body = json.dumps({"text": "Hello."}).encode()
                request = urllib.request.Request(
                    f"http://127.0.0.1:{port}/synthesize",
                    data=body,
                    headers={"Content-Type": "application/json"},
                )
                with urllib.request.urlopen(request) as response:
                    wav = response.read()
                    self.assertEqual(response.headers.get("Content-Type"), "audio/wav")
                self.assertTrue(wav.startswith(b"RIFF"))
                import io
                with wave.open(io.BytesIO(wav), "rb") as handle:
                    self.assertEqual(handle.getnframes(), 200)
                    self.assertEqual(handle.getframerate(), 22050)
                    frames = handle.readframes(handle.getnframes())
                self.assertTrue(any(frames))
            finally:
                try:
                    server.shutdown()
                    server.server_close()
                    mouth.close()
                except Exception:
                    pass
                for key, value in saved.items():
                    if value is None:
                        os.environ.pop(key, None)
                    else:
                        os.environ[key] = value


if __name__ == "__main__":
    unittest.main()
