#!/usr/bin/env python3
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Galion Labs.
"""Speaking rate changes the PCM, not only the logged prosody number.

Measures a 200 Hz tone at 24 kHz. Does not load a model and does not write a WAV.
"""

from __future__ import annotations

import math
import struct
import sys
import os

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import chatterbox_worker as worker


def tone(n: int, freq: float = 200.0, sr: int = 24000) -> bytes:
    samples = [
        int(round(math.sin(2.0 * math.pi * freq * i / sr) * 16000.0))
        for i in range(n)
    ]
    return struct.pack("<" + "h" * n, *samples)


def period(pcm: bytes) -> float:
    samples = struct.unpack("<" + "h" * (len(pcm) // 2), pcm)
    start = len(samples) // 4
    end = len(samples) * 3 // 4
    marks = [
        i
        for i in range(start + 1, end)
        if samples[i - 1] <= 0 and samples[i] > 0
    ]
    if len(marks) < 4:
        raise SystemExit(f"not enough zero crossings in {len(samples)} samples")
    gaps = sorted(marks[i + 1] - marks[i] for i in range(len(marks) - 1))
    return float(gaps[len(gaps) // 2])


def main() -> None:
    n = 24000
    raw = tone(n)
    same = worker.apply_speaking_rate(raw, 1.0)
    if same != raw:
        raise SystemExit("rate 1 changed the bytes")
    fast = worker.apply_speaking_rate(raw, 1.2)
    slow = worker.apply_speaking_rate(raw, 0.8)
    fast_n = len(fast) // 2
    slow_n = len(slow) // 2
    if not fast_n < n:
        raise SystemExit(f"rate 1.2 did not shorten: {fast_n}")
    if not slow_n > n:
        raise SystemExit(f"rate 0.8 did not lengthen: {slow_n}")
    fast_ratio = fast_n / n
    slow_ratio = slow_n / n
    if abs(fast_ratio - (1.0 / 1.2)) >= 0.08:
        raise SystemExit(f"measured fast ratio {fast_ratio} len {fast_n}")
    if abs(slow_ratio - (1.0 / 0.8)) >= 0.08:
        raise SystemExit(f"measured slow ratio {slow_ratio} len {slow_n}")
    src = period(raw)
    fast_p = period(fast)
    slow_p = period(slow)
    if abs(fast_p - src) >= 6.0 or abs(slow_p - src) >= 6.0:
        raise SystemExit(f"period moved: source {src} fast {fast_p} slow {slow_p}")
    clamped = worker.apply_speaking_rate(raw, 4.0)
    if len(clamped) != len(fast):
        raise SystemExit(f"rate 4 was not clamped to 1.2 ({len(clamped)} vs {len(fast)})")
    if worker.speaking_rate_of({}) != 1.0:
        raise SystemExit("missing prosody must be rate 1")
    if worker.speaking_rate_of({"prosody": {"rate": 0.1}}) != 0.8:
        raise SystemExit("rate was not clamped at 0.8")
    print(
        f"measured n={n} fast={fast_n} slow={slow_n} "
        f"period source={src:.1f} fast={fast_p:.1f} slow={slow_p:.1f}"
    )


if __name__ == "__main__":
    main()
