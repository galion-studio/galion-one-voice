#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
"""Cold and warm time-to-first-audio for the Piper mouth, CPU and CUDA.

Every millisecond is a perf_counter around a real child process. When the
voice pack, the runtime, or CUDA is absent, that section has measured=false
and no millisecond field. CUDA is skipped when no GPU is visible, the runtime
is not galion-piper-rt, or the CUDA onnxruntime library is not on disk.
A child that logs a CPU fallback is not recorded as a CUDA result.

Writes JSON and, only for a measured section, WAV files under
GALION_VOICE_TEST_DIR (default /mnt/g/galion/logs/voice-test).
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import statistics
import sys
import time
import wave
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "voice-stack" / "mouth"))

from device import (  # noqa: E402
    Hardware,
    child_env,
    cuda_lib_candidates,
    nvidia_smi_line,
    plan_device,
    probe_hardware,
    resolve_cuda_lib,
    runtime_supports_cuda,
)
from piper_pipe import child_device, spawn  # noqa: E402
from serve import find_model, find_runtime, sample_rate  # noqa: E402

TEXT = "Hello."
PINNED = {
    "en_US-ljspeech-medium.onnx": "6f52a751e2349abe7a76735eb09dc1875298c77ea2342ffd2fef79ff81b87f22",
    "en_US-ljspeech-medium.onnx.json": "141d612cc0a95ed7efc1ca936b845c2364967f2e9217c5dbfcf69fc4d6c65860",
}
TIMING_SUFFIX = "_ms"


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def pin_error(path: Path) -> str | None:
    expected = PINNED.get(path.name)
    if expected is None:
        return None
    got = sha256_file(path)
    if got != expected:
        return f"sha256 mismatch for {path.name}"
    return None


def absent(reason: str, *, skipped: bool) -> dict:
    body = {"measured": False, "skipped": skipped, "reason": reason, "device": None}
    return body


def write_wav(path: Path, pcm: bytes, rate: int) -> None:
    with wave.open(str(path), "wb") as handle:
        handle.setnchannels(1)
        handle.setsampwidth(2)
        handle.setframerate(rate)
        handle.writeframes(pcm)


def measure_device(
    *,
    device: str,
    runtime: Path,
    model: Path,
    config: Path,
    cuda_lib: Path | None,
    out_dir: Path,
    warm_reps: int,
    stem: str,
) -> dict:
    """Time one fresh child. Returns no millisecond keys unless PCM actually arrived on the asked device."""
    env = child_env(device, cuda_lib if device == "cuda" else None)
    rate = sample_rate(config)
    started = time.perf_counter()
    pipe = spawn(runtime, model, config, env)
    try:
        cold = pipe.speak(TEXT)
        cold_ms = None if cold.first_byte_at is None else (cold.first_byte_at - started) * 1000.0
        warms: list[float] = []
        warm_pcm = b""
        for _ in range(warm_reps):
            uttered = pipe.speak(TEXT)
            if uttered.first_byte_at is None or not uttered.pcm:
                warms = []
                warm_pcm = b""
                break
            warms.append((uttered.first_byte_at - uttered.write_at) * 1000.0)
            warm_pcm = uttered.pcm
        stderr = pipe.stderr_text()
    finally:
        pipe.close()
    seen, fallback = child_device(stderr)
    if device == "cuda" and (fallback or seen != "cuda"):
        why = fallback or f"child device line was {seen!r}, not cuda"
        return absent(why, skipped=False)
    if cold_ms is None or not cold.pcm or len(warms) != warm_reps or not warm_pcm:
        return absent("child produced no PCM", skipped=False)
    cold_wav = out_dir / f"{stem}-cold.wav"
    warm_wav = out_dir / f"{stem}-warm.wav"
    write_wav(cold_wav, cold.pcm, rate)
    write_wav(warm_wav, warm_pcm, rate)
    return {
        "measured": True,
        "skipped": False,
        "device": device,
        "runtime": str(runtime),
        "model": str(model),
        "model_sha256": sha256_file(model),
        "sample_rate": rate,
        "text": TEXT,
        "cold_spawn_to_first_pcm_ms": cold_ms,
        "warm_submit_to_first_pcm_ms": warms,
        "warm_median_ms": statistics.median(warms),
        "cold_pcm_bytes": len(cold.pcm),
        "warm_pcm_bytes": len(warm_pcm),
        "cold_wav": cold_wav.name,
        "warm_wav": warm_wav.name,
        "child_device": seen,
        "clock": "cold: Popen until first stdout byte; warm: stdin write until first stdout byte",
    }


def run_bench(
    out_dir: Path,
    *,
    runtime: Path | None = None,
    model: Path | None = None,
    config: Path | None = None,
    hardware: Hardware | None = None,
    warm_reps: int = 5,
    repo: Path | None = None,
) -> dict:
    repo = repo or ROOT
    out_dir.mkdir(parents=True, exist_ok=True)
    hw = hardware if hardware is not None else probe_hardware()
    if runtime is None or model is None or config is None:
        found_runtime = find_runtime(repo) if runtime is None else runtime
        found_model, found_config = find_model(repo)
        runtime = runtime or found_runtime
        model = model or found_model
        config = config or found_config
    cuda_lib = resolve_cuda_lib(os.environ.get("GALION_ORT_CUDA_LIB"), cuda_lib_candidates(repo))
    report: dict = {
        "schema": "galion-mouth-ttfa-gpu/v1",
        "product_claim": False,
        "text": TEXT,
        "warm_reps": warm_reps,
        "out_dir": str(out_dir),
        "cuda_visible": hw.visible(),
        "nvidia_smi": nvidia_smi_line() if hw.nvidia_smi else None,
        "runtime_supports_cuda": runtime_supports_cuda(runtime),
    }
    if runtime is None or model is None or config is None or not runtime.is_file() or not model.is_file() or not config.is_file():
        report["cpu"] = absent("Piper runtime or voice pack is not on disk", skipped=False)
        report["cuda"] = absent("Piper runtime or voice pack is not on disk", skipped=True)
        return report
    bad = pin_error(model) or pin_error(config)
    if bad:
        report["cpu"] = absent(bad, skipped=False)
        report["cuda"] = absent(bad, skipped=True)
        return report
    report["cpu"] = measure_device(
        device="cpu",
        runtime=runtime,
        model=model,
        config=config,
        cuda_lib=None,
        out_dir=out_dir,
        warm_reps=warm_reps,
        stem="cpu",
    )
    cuda_plan = plan_device("cuda", hardware=hw, runtime=runtime, cuda_lib=cuda_lib)
    if not cuda_plan.try_cuda:
        report["cuda"] = absent(cuda_plan.log or "CUDA was not attempted", skipped=True)
        return report
    report["cuda"] = measure_device(
        device="cuda",
        runtime=runtime,
        model=model,
        config=config,
        cuda_lib=cuda_lib,
        out_dir=out_dir,
        warm_reps=warm_reps,
        stem="cuda",
    )
    if report["cuda"].get("measured") is not True:
        report["cuda"]["skipped"] = report["cuda"].get("skipped", False)
        for name in ("cuda-cold.wav", "cuda-warm.wav"):
            path = out_dir / name
            if path.exists():
                path.unlink()
    return report


def timing_keys(section: dict) -> list[str]:
    return [key for key in section if key.endswith(TIMING_SUFFIX)]


def assert_honest(report: dict) -> None:
    """Raise if an unmeasured section carries a millisecond or a CUDA claim without audio."""
    if report.get("product_claim") is not False:
        raise SystemExit("product_claim must stay false")
    for name in ("cpu", "cuda"):
        section = report[name]
        if section.get("measured") is True:
            for key in ("cold_spawn_to_first_pcm_ms", "warm_submit_to_first_pcm_ms", "warm_median_ms"):
                if key not in section:
                    raise SystemExit(f"{name} measured without {key}")
            if not section.get("cold_pcm_bytes") or not section.get("warm_pcm_bytes"):
                raise SystemExit(f"{name} measured without PCM")
            continue
        found = timing_keys(section)
        if found:
            raise SystemExit(f"{name} is unmeasured but has {found}")
        if section.get("device") == "cuda":
            raise SystemExit("unmeasured section claims device=cuda")
    cuda = report["cuda"]
    if cuda.get("measured") is True and cuda.get("device") != "cuda":
        raise SystemExit("cuda section measured on a device other than cuda")
    if not report.get("cuda_visible") and cuda.get("measured") is True:
        raise SystemExit("CUDA result recorded when cuda_visible is false")


def main() -> int:
    parser = argparse.ArgumentParser(description="Measure Piper TTFA on CPU and CUDA. Does not invent numbers.")
    parser.add_argument("--out-dir", default=os.environ.get("GALION_VOICE_TEST_DIR", "/mnt/g/galion/logs/voice-test"))
    parser.add_argument("--runtime", default="")
    parser.add_argument("--model", default="")
    parser.add_argument("--config", default="")
    parser.add_argument("--warm-reps", type=int, default=5)
    args = parser.parse_args()
    out_dir = Path(args.out_dir)
    try:
        out_dir.mkdir(parents=True, exist_ok=True)
    except OSError as exc:
        print(f"bench-ttfa-gpu: cannot create {out_dir}: {exc}", file=sys.stderr)
        return 1
    runtime = Path(args.runtime) if args.runtime else None
    model = Path(args.model) if args.model else None
    config = Path(args.config) if args.config else (Path(f"{model}.json") if model else None)
    report = run_bench(out_dir, runtime=runtime, model=model, config=config, warm_reps=args.warm_reps)
    assert_honest(report)
    dest = out_dir / "ttfa-gpu.json"
    dest.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(json.dumps({"wrote": str(dest), "cpu_measured": report["cpu"]["measured"], "cuda_measured": report["cuda"]["measured"], "cuda_skipped": report["cuda"]["skipped"]}))
    if report["cpu"]["measured"] is not True:
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
