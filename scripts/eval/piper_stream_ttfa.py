#!/usr/bin/env python3
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Live Piper CPU stream clock.

PASS only when GALION_SPEECH=a21 and the pinned Piper process writes real
PCM. A protocol-stub clock is FAIL. This script does not invent a ttfa_ms.
The number is the time from the request write to the first PCM byte.
"""

from __future__ import annotations

import json
import os
import select
import struct
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
RUNTIME = ROOT / "weights" / "runtime" / "piper" / "piper"
MODEL = ROOT / "weights" / "tts" / "en_US-ljspeech-medium.onnx"
SAMPLES = ROOT / "scripts" / "eval" / "sample-voices"
SAMPLE_NAMES = (
    "pdx-voice.wav",
    "armstrong-en.wav",
    "voicezero-amy_koenig.wav",
    "voicezero-alan_davis_drake.wav",
)
VOICE = "en_US-ljspeech-medium"


def fail(report: dict) -> None:
    sys.stdout.write("FAIL\n")
    sys.stdout.write(json.dumps(report, sort_keys=True) + "\n")
    raise SystemExit(1)


def wav_ok(path: Path) -> bool:
    if not path.is_file() or path.stat().st_size < 44:
        return False
    with path.open("rb") as handle:
        head = handle.read(12)
    return head[:4] == b"RIFF" and head[8:12] == b"WAVE"


def real_piper_pcm(pcm: bytes) -> bool:
    if len(pcm) < 1920 * 2:
        return False
    count = min(len(pcm) // 2, 4000)
    samples = struct.unpack("<" + "h" * count, pcm[: count * 2])
    peak = max(abs(sample) for sample in samples)
    return peak >= 100 and len(set(samples[:800])) >= 8


def read_utterance(proc: subprocess.Popen, text: str, timeout: float) -> dict:
    assert proc.stdin and proc.stdout and proc.stderr
    t0 = time.perf_counter()
    proc.stdin.write((json.dumps({"text": text}) + "\n").encode())
    proc.stdin.flush()
    first = None
    utt = None
    pcm = b""
    err = b""
    deadline = t0 + timeout
    while time.perf_counter() < deadline:
        readable, _, _ = select.select([proc.stdout, proc.stderr], [], [], 0.02)
        if proc.stdout in readable:
            block = os.read(proc.stdout.fileno(), 65536)
            if block:
                if first is None:
                    first = time.perf_counter()
                pcm += block
        if proc.stderr in readable:
            block = os.read(proc.stderr.fileno(), 65536)
            if block:
                err += block
                if utt is None and b"Real-time factor:" in err:
                    utt = time.perf_counter()
        if utt is not None and first is not None:
            break
        if proc.poll() is not None and utt is not None:
            break
    ttfa = None if first is None else round((first - t0) * 1000, 1)
    utt_ms = None if utt is None else round((utt - t0) * 1000, 1)
    return {
        "ttfa_ms": ttfa,
        "utt_ms": utt_ms,
        "pcm_bytes": len(pcm),
        "real_pcm": real_piper_pcm(pcm),
        "stderr_tail": err.decode(errors="replace").strip().splitlines()[-4:],
    }


def main() -> None:
    if os.environ.get("GALION_SPEECH") != "a21":
        fail(
            {
                "gate": "FAIL",
                "hard_pass": False,
                "product_claim": False,
                "piper_measured": False,
                "model_measured": False,
                "stub": False,
                "engine": None,
                "note": "GALION_SPEECH is not a21. A protocol-stub clock is not accepted.",
            }
        )
    present = [name for name in SAMPLE_NAMES if wav_ok(SAMPLES / name)]
    if not RUNTIME.is_file() or not MODEL.is_file():
        fail(
            {
                "gate": "FAIL",
                "hard_pass": False,
                "product_claim": False,
                "piper_measured": False,
                "model_measured": False,
                "stub": False,
                "engine": None,
                "note": "Piper runtime or en_US-ljspeech-medium.onnx is absent. scripts/a21/fetch-a21.sh",
            }
        )
    env = os.environ.copy()
    lib = str(RUNTIME.parent)
    env["LD_LIBRARY_PATH"] = lib + (":" + env["LD_LIBRARY_PATH"] if env.get("LD_LIBRARY_PATH") else "")
    proc = subprocess.Popen(
        [
            str(RUNTIME),
            "--model",
            str(MODEL),
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
    try:
        cold = read_utterance(proc, "Hi.", 30.0)
        warm = read_utterance(proc, "The line is open.", 30.0)
    finally:
        if proc.poll() is None:
            proc.kill()
            proc.wait(timeout=5)
    sample = "scripts/eval/sample-voices/pdx-voice.wav" if "pdx-voice.wav" in present else None
    real = bool(cold.get("real_pcm"))
    ttfa = cold.get("ttfa_ms")
    passed = real and ttfa is not None and ttfa < 1500
    report = {
        "gate": "PASS" if passed else "FAIL",
        "hard_pass": passed,
        "product_claim": passed,
        "piper_measured": passed,
        "model_measured": False,
        "stub": False,
        "engine": "piper" if real else None,
        "voice": VOICE,
        "sample": sample,
        "samples_checked": present,
        "ttfa_ms": ttfa if real else None,
        "utt_ms": cold.get("utt_ms") if real else None,
        "warm_ttfa_ms": warm.get("ttfa_ms") if warm.get("real_pcm") else None,
        "pcm_bytes": cold.get("pcm_bytes"),
        "path": "stream" if real else None,
        "note": (
            "Cold spawn, first PCM byte, Piper en_US-ljspeech-medium on this CPU. "
            "A protocol-stub clock is FAIL and product_claim stays false. "
            "This is not an 80 ms claim."
        ),
    }
    if not passed:
        fail(report)
    sys.stdout.write("PASS\n")
    sys.stdout.write(json.dumps(report, sort_keys=True) + "\n")


if __name__ == "__main__":
    main()
