# G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network
# Developed and distributed by Galion Labs
# Perpetual Open MIT License
# Copyright © 2026 Galion Labs. All rights reserved.
"""Compute device for scorer plugins.

The production default is ``cpu``: no GPU probe, no torch import, and every
CPU thread is offered to the scorers. ``auto`` is opt-in (for example on the
Galion Studio laptop): it picks CUDA when a CUDA build of torch is installed
and sees a GPU, and otherwise falls back to CPU with all threads.
``cuda`` asks for the GPU and fails if there is none.

This module only decides. The scorer plugin receives the decision in the
``build_scorers(config)`` dict as ``device``, ``num_threads``, and
``device_reason`` and is responsible for placing its model.
"""

from __future__ import annotations

import importlib
import importlib.util
import os
from collections.abc import Callable
from dataclasses import dataclass
from typing import Any

from emotion_v2_dataset.errors import QualityError

DEVICE_CHOICES = ("cpu", "auto", "cuda")
PRODUCTION_DEVICE = "cpu"
THREAD_ENV = ("OMP_NUM_THREADS", "MKL_NUM_THREADS", "OPENBLAS_NUM_THREADS")


@dataclass(frozen=True)
class ComputeChoice:
    device: str
    num_threads: int
    reason: str

    def as_config(self) -> dict[str, Any]:
        return {"device": self.device, "num_threads": self.num_threads, "device_reason": self.reason}


def cpu_threads() -> int:
    count = None
    if hasattr(os, "process_cpu_count"):
        count = os.process_cpu_count()  # Python 3.13+: respects affinity
    if not count:
        count = os.cpu_count()
    return max(1, int(count or 1))


def cuda_available(importer: Callable[[str], Any] | None = None) -> tuple[bool, str]:
    if importer is None:
        if importlib.util.find_spec("torch") is None:
            return False, "torch not installed"
        importer = importlib.import_module
    try:
        backend = importer("torch")
    except ImportError:
        return False, "torch not installed"
    cuda = getattr(backend, "cuda", None)
    probe = getattr(cuda, "is_available", None)
    if not callable(probe):
        return False, "torch has no cuda module"
    try:
        if not probe():
            return False, "torch sees no CUDA device"
    except Exception as exc:  # a broken driver must not crash the filter
        return False, f"CUDA probe failed: {type(exc).__name__}"
    name_fn = getattr(cuda, "get_device_name", None)
    try:
        name = str(name_fn(0)) if callable(name_fn) else "cuda:0"
    except Exception:
        name = "cuda:0"
    return True, f"CUDA device {name}"


def resolve_device(
    requested: str = PRODUCTION_DEVICE,
    *,
    importer: Callable[[str], Any] | None = None,
    threads: int | None = None,
) -> ComputeChoice:
    if requested not in DEVICE_CHOICES:
        raise QualityError(f"device must be one of {', '.join(DEVICE_CHOICES)}")
    count = threads if threads is not None else cpu_threads()
    if requested == "cpu":
        return ComputeChoice("cpu", count, "cpu requested (production default)")
    ok, why = cuda_available(importer)
    if ok:
        return ComputeChoice("cuda", count, why)
    if requested == "cuda":
        raise QualityError(f"cuda requested but unavailable: {why}")
    return ComputeChoice("cpu", count, f"auto fell back to cpu: {why}")


def apply_thread_env(choice: ComputeChoice) -> None:
    """Offer every thread to BLAS/OpenMP scorers unless the caller already set it."""
    for name in THREAD_ENV:
        os.environ.setdefault(name, str(choice.num_threads))
