# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
"""Device choice for the Galion mouth. CPU is the default.

``GALION_TTS_DEVICE=cuda`` (or ``gpu``) asks for the ONNX Runtime CUDA
execution provider. ``auto`` stays on CPU. An unknown token stays on CPU and
carries a log line. This module does not time anything and does not invent a
latency.
"""

from __future__ import annotations

import os
import subprocess
from dataclasses import dataclass
from pathlib import Path

ORT_SONAME = "lib/libonnxruntime.so.1.31.1"
GPU_PACKS = (
    "onnxruntime-linux-x64-gpu_cuda13-1.31.1",
    "onnxruntime-linux-x64-gpu_cuda12-1.31.1",
)


@dataclass(frozen=True)
class Plan:
    """What the mouth should ask the Piper child to do."""

    device: str
    log: str | None = None
    skipped: bool = False

    @property
    def try_cuda(self) -> bool:
        return self.device == "cuda"


def parse_request(raw: str | None) -> tuple[str, str | None]:
    token = (raw or "").strip().lower()
    if token in ("", "cpu", "auto"):
        return "cpu", None
    if token in ("cuda", "gpu"):
        return "cuda", None
    shown = (raw or "").strip()
    return "cpu", f"unknown GALION_TTS_DEVICE={shown}; falling back to CPU"


def hidden_by_cuda_visible_devices(value: str | None) -> bool:
    if value is None:
        return False
    token = value.strip()
    return token == "" or token == "-1"


@dataclass(frozen=True)
class Hardware:
    hidden_by_env: bool
    nvidia_smi: bool
    dev_node: bool
    proc_driver: bool

    def visible(self) -> bool:
        if self.hidden_by_env:
            return False
        return self.nvidia_smi or self.dev_node or self.proc_driver

    def reason(self) -> str:
        if self.hidden_by_env:
            return "CUDA_VISIBLE_DEVICES hides every device"
        parts = []
        if not self.nvidia_smi:
            parts.append("nvidia-smi did not list a GPU")
        if not self.dev_node:
            parts.append("/dev/nvidia0 is absent")
        if not self.proc_driver:
            parts.append("/proc/driver/nvidia/version is absent")
        return "; ".join(parts)


def probe_hardware() -> Hardware:
    return Hardware(
        hidden_by_env=hidden_by_cuda_visible_devices(os.environ.get("CUDA_VISIBLE_DEVICES")),
        nvidia_smi=nvidia_smi_lists_gpu(),
        dev_node=Path("/dev/nvidia0").exists(),
        proc_driver=Path("/proc/driver/nvidia/version").is_file(),
    )


def nvidia_smi_lists_gpu() -> bool:
    try:
        proc = subprocess.run(
            ["nvidia-smi", "-L"],
            capture_output=True,
            text=True,
            timeout=3,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired):
        return False
    if proc.returncode != 0:
        return False
    return any(line.startswith("GPU ") for line in proc.stdout.splitlines())


def nvidia_smi_line() -> str | None:
    try:
        proc = subprocess.run(
            ["nvidia-smi", "-L"],
            capture_output=True,
            text=True,
            timeout=3,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired):
        return None
    if proc.returncode != 0:
        return None
    for line in proc.stdout.splitlines():
        if line.startswith("GPU "):
            return line.strip()
    return None


def runtime_supports_cuda(runtime: Path | None) -> bool:
    if runtime is None:
        return False
    return runtime.name == "galion-piper-rt"


def cuda_lib_candidates(repo: Path | None) -> list[Path]:
    roots: list[Path] = []
    if repo is not None:
        roots.append(repo / "weights" / "runtime" / "ort-cuda")
    roots.append(Path("/mnt/g/galion/ort"))
    found: list[Path] = []
    for root in roots:
        for pack in GPU_PACKS:
            found.append(root / pack / ORT_SONAME)
        found.append(root / ORT_SONAME)
    return found


def resolve_cuda_lib(explicit: str | None, candidates: list[Path]) -> Path | None:
    if explicit is not None and explicit.strip():
        path = Path(explicit.strip())
        return path if path.is_file() else None
    for path in candidates:
        if path.is_file():
            return path
    return None


def plan_device(
    raw: str | None,
    *,
    hardware: Hardware,
    runtime: Path | None,
    cuda_lib: Path | None,
) -> Plan:
    """Decide the device the child is asked to use. CPU unless every CUDA gate passes."""
    request, unknown = parse_request(raw)
    if unknown:
        return Plan("cpu", unknown, skipped=True)
    if request != "cuda":
        return Plan("cpu", None, skipped=False)
    if not hardware.visible():
        return Plan(
            "cpu",
            f"GALION_TTS_DEVICE=cuda but CUDA is not available ({hardware.reason()}); falling back to CPU",
            skipped=True,
        )
    if not runtime_supports_cuda(runtime):
        name = runtime.name if runtime is not None else "missing runtime"
        return Plan(
            "cpu",
            f"GALION_TTS_DEVICE=cuda but {name} has no CUDA execution provider; falling back to CPU",
            skipped=True,
        )
    if cuda_lib is None:
        return Plan(
            "cpu",
            "GALION_TTS_DEVICE=cuda but the CUDA onnxruntime library is not on disk; falling back to CPU",
            skipped=True,
        )
    return Plan("cuda", None, skipped=False)


def child_env(device: str, cuda_lib: Path | None) -> dict[str, str]:
    """Environment for one Piper child. The device is explicit, so a parent ``cuda`` does not leak into a CPU child."""
    env = os.environ.copy()
    env["GALION_TTS_DEVICE"] = device
    if device != "cuda":
        env.pop("GALION_ORT_CUDA_LIB", None)
        return env
    if cuda_lib is not None:
        env["GALION_ORT_CUDA_LIB"] = str(cuda_lib)
        parts = [str(cuda_lib.parent)]
        extra = os.environ.get("GALION_CUDA_LIB_DIR", "").strip()
        if extra:
            parts.append(extra)
        old = os.environ.get("LD_LIBRARY_PATH", "").strip()
        if old:
            parts.append(old)
        env["LD_LIBRARY_PATH"] = ":".join(parts)
    return env
