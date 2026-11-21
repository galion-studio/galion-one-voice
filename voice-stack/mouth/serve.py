# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
"""Galion mouth. HTTP TTS on 127.0.0.1:8002. CPU unless CUDA is actually available.

The process binds numeric 127.0.0.1 only. It does not call an LLM, Ollama, or
a paid API. It speaks the text it is given through the Piper child. A missing
pack is a 503 and no audio.
"""

from __future__ import annotations

import json
import os
import sys
import time
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path

from device import Hardware, Plan, child_env, plan_device, probe_hardware, resolve_cuda_lib, cuda_lib_candidates
from piper_pipe import PiperPipe, child_device, spawn

REPO = Path(__file__).resolve().parents[2]
DEFAULT_BIND = "127.0.0.1:8002"
MAX_BODY = 64 * 1024
MAX_TEXT = 500


def parse_bind(raw: str) -> tuple[str, int]:
    text = (raw or "").strip()
    if text.startswith("[") or "://" in text or text.startswith("localhost"):
        raise ValueError(f"refusing bind {text}; galion-mouth listens on 127.0.0.1 only")
    host, sep, port_s = text.rpartition(":")
    if sep != ":" or host != "127.0.0.1":
        raise ValueError(f"refusing bind {text}; galion-mouth listens on 127.0.0.1 only")
    try:
        port = int(port_s)
    except ValueError as exc:
        raise ValueError(f"refusing bind {text}; galion-mouth listens on 127.0.0.1 only") from exc
    if port < 0 or port > 65535:
        raise ValueError(f"refusing bind {text}; galion-mouth listens on 127.0.0.1 only")
    return host, port


def find_runtime(repo: Path) -> Path | None:
    if os.environ.get("GALION_PIPER_LEGACY", "").strip().lower() in ("1", "true", "yes"):
        names = ("piper",)
    else:
        names = ("galion-piper-rt", "piper")
    explicit = os.environ.get("GALION_A21_RUNTIME", "").strip()
    if explicit:
        path = Path(explicit)
        return path if path.is_file() and os.access(path, os.X_OK) else None
    candidates: list[Path] = []
    for root in (repo, Path.cwd()):
        for name in names:
            candidates.append(root / "weights" / "runtime" / "piper-rt" / name)
            candidates.append(root / "weights" / "runtime" / "piper" / name)
            candidates.append(root / "voice-stack" / "linux-package" / "wsl" / "var" / "models" / "tts" / "piper" / name)
    for path in candidates:
        if path.is_file() and os.access(path, os.X_OK) and path.name in names:
            return path
    return None


def find_model(repo: Path) -> tuple[Path | None, Path | None]:
    explicit = ""
    for key in ("GALION_PIPER_MODEL", "GALION_TTS_WEIGHTS", "GALION_SPEECH_WEIGHTS", "GALION_A21_MODEL"):
        explicit = os.environ.get(key, "").strip()
        if explicit:
            break
    if explicit:
        model = Path(explicit)
        config = Path(os.environ.get("GALION_PIPER_CONFIG", "").strip() or f"{model}.json")
        if model.is_file() and config.is_file():
            return model, config
        return None, None
    name = "en_US-ljspeech-medium.onnx"
    candidates = [
        repo / "weights" / "tts" / name,
        repo / "voice-stack" / "linux-package" / "wsl" / "var" / "models" / "tts" / name,
        repo / "voice-stack" / "tts-piper-stream" / "var" / "voices" / name,
    ]
    for model in candidates:
        config = Path(f"{model}.json")
        if model.is_file() and config.is_file():
            return model, config
    return None, None


def sample_rate(config: Path | None) -> int:
    if config is None or not config.is_file():
        return 22050
    try:
        data = json.loads(config.read_text())
    except (OSError, json.JSONDecodeError):
        return 22050
    audio = data.get("audio") if isinstance(data, dict) else None
    if isinstance(audio, dict) and isinstance(audio.get("sample_rate"), int):
        return int(audio["sample_rate"])
    if isinstance(data, dict) and isinstance(data.get("sample_rate"), int):
        return int(data["sample_rate"])
    return 22050


def pcm_to_wav(pcm: bytes, rate: int) -> bytes:
    import io
    import wave

    buf = io.BytesIO()
    with wave.open(buf, "wb") as handle:
        handle.setnchannels(1)
        handle.setsampwidth(2)
        handle.setframerate(rate)
        handle.writeframes(pcm)
    return buf.getvalue()


class Mouth:
    def __init__(
        self,
        bind: tuple[str, int],
        runtime: Path | None,
        model: Path | None,
        config: Path | None,
        plan: Plan,
        cuda_lib: Path | None,
        hardware: Hardware,
    ):
        self.bind = bind
        self.runtime = runtime
        self.model = model
        self.config = config
        self.plan = plan
        self.cuda_lib = cuda_lib
        self.hardware = hardware
        self.child: PiperPipe | None = None
        self.child_fallback: str | None = plan.log
        self.ready = False
        if plan.log:
            print(f"galion-mouth: {plan.log}", file=sys.stderr, flush=True)
        if runtime is not None and model is not None and config is not None:
            self.child = self._open(plan.device)
            self.ready = self.child is not None and self.child.alive()

    def _open(self, device: str) -> PiperPipe | None:
        if self.runtime is None or self.model is None or self.config is None:
            return None
        pipe = spawn(self.runtime, self.model, self.config, child_env(device, self.cuda_lib if device == "cuda" else None))
        if self.runtime.name != "galion-piper-rt":
            return pipe
        deadline = time.perf_counter() + 120
        while time.perf_counter() < deadline:
            device_now, fallback = child_device(pipe.stderr_text())
            if fallback and device == "cuda":
                print(f"galion-mouth: {fallback}", file=sys.stderr, flush=True)
                self.child_fallback = fallback
                self.plan = Plan("cpu", fallback, skipped=True)
                pipe.close()
                return self._open("cpu")
            if device_now is not None or not pipe.alive():
                if not pipe.alive() and device == "cuda":
                    err = pipe.stderr_text().strip() or "child exited before device="
                    note = f"GALION_TTS_DEVICE=cuda but the child exited ({err}); falling back to CPU"
                    print(f"galion-mouth: {note}", file=sys.stderr, flush=True)
                    self.child_fallback = note
                    self.plan = Plan("cpu", note, skipped=True)
                    return self._open("cpu")
                return pipe
            time.sleep(0.01)
        return pipe

    def speak(self, text: str) -> bytes:
        if self.child is None or not self.child.alive():
            if self.runtime is None or self.model is None:
                raise RuntimeError("piper runtime or voice pack is not on disk")
            self.child = self._open(self.plan.device)
        if self.child is None:
            raise RuntimeError("piper child did not start")
        uttered = self.child.speak(text)
        if not uttered.pcm or uttered.first_byte_at is None:
            raise RuntimeError("piper produced no PCM")
        return uttered.pcm

    def health(self) -> dict[str, object]:
        device_now = self.plan.device
        fallback = self.child_fallback
        if self.child is not None:
            seen, child_fallback = child_device(self.child.stderr_text())
            if child_fallback:
                fallback = child_fallback
                device_now = "cpu"
            elif seen:
                device_now = seen
        return {
            "ok": self.ready,
            "bind": f"{self.bind[0]}:{self.bind[1]}",
            "engine": "piper",
            "voice": self.model.name if self.model else None,
            "requested_device": os.environ.get("GALION_TTS_DEVICE", ""),
            "device": device_now,
            "fallback": fallback,
            "cuda_visible": self.hardware.visible(),
            "product_claim": False,
        }

    def close(self) -> None:
        if self.child is not None:
            self.child.close()
            self.child = None


def make_handler(mouth: Mouth):
    class Handler(BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.1"

        def log_message(self, fmt: str, *args) -> None:
            return

        def _json(self, code: int, body: dict) -> None:
            raw = json.dumps(body).encode()
            self.send_response(code)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(raw)))
            self.end_headers()
            self.wfile.write(raw)

        def do_GET(self) -> None:  # noqa: N802
            path = self.path.split("?", 1)[0]
            if path != "/health":
                self._json(404, {"ok": False, "error": "not found"})
                return
            code = 200 if mouth.ready else 503
            self._json(code, mouth.health())

        def do_POST(self) -> None:  # noqa: N802
            path = self.path.split("?", 1)[0]
            if path != "/synthesize":
                self._json(404, {"ok": False, "error": "not found"})
                return
            length = int(self.headers.get("Content-Length", "0") or "0")
            if length < 0 or length > MAX_BODY:
                self._json(400, {"ok": False, "error": "body too large"})
                return
            raw = self.rfile.read(length) if length else b""
            try:
                payload = json.loads(raw.decode() or "{}")
                text = str(payload.get("text", "")).strip()
            except (UnicodeError, json.JSONDecodeError, AttributeError):
                self._json(400, {"ok": False, "error": "expected JSON {\"text\": ...}"})
                return
            if not text or len(text) > MAX_TEXT:
                self._json(400, {"ok": False, "error": "text must be 1 to 500 characters"})
                return
            if not mouth.ready:
                self._json(503, {"ok": False, "error": "piper runtime or voice pack is not on disk", "product_claim": False})
                return
            try:
                pcm = mouth.speak(text)
                wav = pcm_to_wav(pcm, sample_rate(mouth.config))
            except Exception as exc:  # noqa: BLE001 — the client gets the child error, not a stack
                self._json(500, {"ok": False, "error": str(exc), "product_claim": False})
                return
            self.send_response(200)
            self.send_header("Content-Type", "audio/wav")
            self.send_header("Content-Length", str(len(wav)))
            self.end_headers()
            self.wfile.write(wav)

    return Handler


def build_mouth(repo: Path, bind_raw: str, hardware: Hardware | None = None) -> Mouth:
    host, port = parse_bind(bind_raw)
    runtime = find_runtime(repo)
    model, config = find_model(repo)
    hw = hardware if hardware is not None else probe_hardware()
    cuda_lib = resolve_cuda_lib(os.environ.get("GALION_ORT_CUDA_LIB"), cuda_lib_candidates(repo))
    chosen = plan_device(os.environ.get("GALION_TTS_DEVICE"), hardware=hw, runtime=runtime, cuda_lib=cuda_lib)
    return Mouth((host, port), runtime, model, config, chosen, cuda_lib, hw)


def serve_forever(mouth: Mouth) -> HTTPServer:
    server = HTTPServer(mouth.bind, make_handler(mouth))
    host, port = server.server_address[:2]
    mouth.bind = (str(host), int(port))
    return server


def main() -> int:
    raw_bind = os.environ.get("GALION_MOUTH_BIND", DEFAULT_BIND)
    try:
        mouth = build_mouth(REPO, raw_bind)
    except ValueError as exc:
        print(f"galion-mouth: {exc}", file=sys.stderr)
        return 2
    server = serve_forever(mouth)
    bound = server.server_address
    print(
        f"galion-mouth: bind=127.0.0.1:{bound[1]} device={mouth.plan.device} ready={str(mouth.ready).lower()}",
        file=sys.stderr,
        flush=True,
    )
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        return 0
    finally:
        mouth.close()
        server.server_close()
    return 0


if __name__ == "__main__":
    sys.exit(main())
