# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
"""One Piper child on the json-input / raw PCM pipe. Timestamps come from the clock around that child."""

from __future__ import annotations

import json
import os
import subprocess
import threading
import time
from dataclasses import dataclass
from pathlib import Path


@dataclass
class Utterance:
    pcm: bytes
    first_byte_at: float | None
    write_at: float
    stderr: str


class PiperPipe:
    def __init__(self, proc: subprocess.Popen[bytes]):
        self.proc = proc
        self.stderr_parts: list[str] = []
        self._rtf = threading.Event()
        self._lock = threading.Lock()
        self._thread = threading.Thread(target=self._read_err, daemon=True)
        self._thread.start()

    def _read_err(self) -> None:
        stderr = self.proc.stderr
        if stderr is None:
            return
        while True:
            try:
                raw = stderr.readline()
            except (OSError, ValueError):
                return
            if not raw:
                return
            line = raw.decode(errors="replace")
            with self._lock:
                self.stderr_parts.append(line)
            if "Real-time factor" in line:
                self._rtf.set()

    def stderr_text(self) -> str:
        with self._lock:
            return "".join(self.stderr_parts)

    def alive(self) -> bool:
        return self.proc.poll() is None

    def close(self) -> None:
        if self.proc.poll() is None:
            try:
                if self.proc.stdin:
                    self.proc.stdin.close()
            except OSError:
                pass
            try:
                self.proc.wait(timeout=2)
            except subprocess.TimeoutExpired:
                self.proc.kill()
                self.proc.wait(timeout=2)
        for stream in (self.proc.stdin, self.proc.stdout, self.proc.stderr):
            if stream is None:
                continue
            try:
                stream.close()
            except OSError:
                pass
        self._thread.join(timeout=2)

    def speak(self, text: str, timeout_s: float = 120.0) -> Utterance:
        stdin = self.proc.stdin
        stdout = self.proc.stdout
        if stdin is None or stdout is None:
            raise RuntimeError("piper pipes are closed")
        self._rtf.clear()
        write_at = time.perf_counter()
        stdin.write((json.dumps({"text": text}) + "\n").encode())
        stdin.flush()
        fd = stdout.fileno()
        os.set_blocking(fd, False)
        chunks: list[bytes] = []
        first: float | None = None
        deadline = time.perf_counter() + timeout_s
        while time.perf_counter() < deadline:
            try:
                blob = os.read(fd, 65536)
            except BlockingIOError:
                blob = None
            if blob:
                if first is None:
                    first = time.perf_counter()
                chunks.append(blob)
                continue
            if blob == b"":
                break
            if self._rtf.is_set() or self.proc.poll() is not None:
                time.sleep(0.005)
                try:
                    while True:
                        more = os.read(fd, 65536)
                        if not more:
                            break
                        if first is None:
                            first = time.perf_counter()
                        chunks.append(more)
                except BlockingIOError:
                    pass
                break
            time.sleep(0.0005)
        return Utterance(b"".join(chunks), first, write_at, self.stderr_text())


def spawn(runtime: Path, model: Path, config: Path, env: dict[str, str]) -> PiperPipe:
    proc = subprocess.Popen(
        [
            str(runtime),
            "--model",
            str(model),
            "--config",
            str(config),
            "--output_raw",
            "--json-input",
            "--noise_scale",
            "0",
            "--noise_w",
            "0",
            "--length_scale",
            "1.0",
            "--sentence_silence",
            "0",
        ],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=env,
    )
    return PiperPipe(proc)


def child_device(stderr: str) -> tuple[str | None, str | None]:
    """Return ``(device, fallback_line)``. A fallback line forces CPU."""
    device: str | None = None
    fallback: str | None = None
    for line in stderr.splitlines():
        if line.startswith("[galion-piper-rt] device="):
            token = line.split("=", 1)[1].strip().split()[0]
            if token in ("cpu", "cuda"):
                device = token
        if "falling back to CPU" in line:
            fallback = line.strip()
            device = "cpu"
    return device, fallback
