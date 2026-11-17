#!/usr/bin/env python3
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Galion Labs.
"""The worker reads exaggeration, cfg, temperature, pitch, energy and pauses.

No model is loaded. No WAV is written. A missing field keeps the old literal,
and that literal still produces the old PCM bytes.
"""

from __future__ import annotations

import json
import os
import struct
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import chatterbox_worker as worker


def tone(n: int, freq: float = 200.0, sr: int = 24000) -> bytes:
    import math

    samples = [
        int(round(math.sin(2.0 * math.pi * freq * i / sr) * 8000.0))
        for i in range(n)
    ]
    return struct.pack("<" + "h" * n, *samples)


def rms(pcm: bytes) -> float:
    samples = struct.unpack("<" + "h" * (len(pcm) // 2), pcm)
    if not samples:
        return 0.0
    acc = sum(s * s for s in samples) / len(samples)
    return acc ** 0.5


def main() -> None:
    identity = worker.controls_of({"text": "Hi."})
    if identity.exaggeration != 0.0 or identity.cfg_weight != 0.0 or identity.temperature != 0.8:
        raise SystemExit(f"defaults drifted: {identity.__dict__}")
    if identity.pitch_st != 0.0:
        raise SystemExit("pitch default")
    if not identity.prosody_identity():
        raise SystemExit("missing prosody must be identity")

    raw = tone(24000)
    same = worker.apply_prosody_pcm(raw, identity)
    if same != raw:
        raise SystemExit("identity prosody changed the bytes")

    loud = worker.controls_of({"prosody": {"energy": 1.2}})
    louder = worker.apply_prosody_pcm(raw, loud)
    if not rms(louder) > rms(raw) * 1.1:
        raise SystemExit(f"energy did not raise RMS ({rms(raw)} -> {rms(louder)})")

    paused = worker.controls_of({"prosody": {"pre_pause_ms": 80}})
    held = worker.apply_prosody_pcm(raw, paused)
    gap = 80 * 24000 // 1000
    if held[: gap * 2] != b"\x00" * (gap * 2):
        raise SystemExit("pre_pause_ms did not prepend silence")
    if held[gap * 2 :] != raw:
        raise SystemExit("pause ate the tone")

    pitched = worker.controls_of({"prosody": {"pitch_st": 2.0}})
    higher = worker.apply_prosody_pcm(raw, pitched)
    if not len(higher) < len(raw):
        raise SystemExit(f"pitch did not shorten the buffer ({len(higher)} vs {len(raw)})")

    asked = worker.controls_of(
        {
            "exaggeration": 0.4,
            "cfg": 0.2,
            "temperature": 0.7,
            "prosody": {"pitch_st": 9.0, "energy": 4.0, "pre_pause_ms": 900, "rate": 0.1},
        }
    )
    if asked.exaggeration != 0.4 or abs(asked.cfg_weight - 0.2) > 1e-9 or abs(asked.temperature - 0.7) > 1e-9:
        raise SystemExit(f"model knobs not read: {asked.__dict__}")
    if asked.pitch_st != 4.0 or asked.energy != 1.2 or asked.pre_pause_ms != 400 or asked.rate != 0.8:
        raise SystemExit(f"clamps drifted: {asked.__dict__}")
    if worker.stub_tone("Hi.", identity) == worker.stub_tone("Hi.", asked):
        raise SystemExit("stub tone ignored exaggeration, cfg and temperature")

    if not worker.exaggeration_matches("0.0", 0.0):
        raise SystemExit("zero cache key must stay the string 0.0")
    if worker.exaggeration_matches("0", 0.0):
        raise SystemExit("a bare 0 must not match the zero cache key")
    if not worker.exaggeration_matches("0.40", 0.4):
        raise SystemExit("non-zero exaggeration should match numerically")

    env = os.environ.copy()
    env["GALION_CLONE_STUB_PCM"] = "1"
    env["GALION_CLONE_DEVICE"] = "cpu"
    env.pop("GALION_CLONE_STUB_FRAMES", None)
    payload = {
        "text": "Hi.",
        "persona": "samantha",
        "model": "nano",
        "exaggeration": 0.5,
        "cfg_weight": 0.25,
        "temperature": 1.0,
        "prosody": {"pitch_st": 0.0, "energy": 1.0, "rate": 1.0, "pre_pause_ms": 0},
    }
    proc = subprocess.run(
        [sys.executable, os.path.join(HERE, "chatterbox_worker.py")],
        input=(json.dumps(payload) + "\n").encode(),
        capture_output=True,
        env=env,
        timeout=15,
        check=False,
    )
    if proc.returncode != 0:
        raise SystemExit(proc.stderr.decode())
    err = proc.stderr.decode()
    if "galion-clone-controls: exaggeration=0.5000 cfg_weight=0.2500 temperature=1.0000" not in err:
        raise SystemExit(f"controls were not logged\n{err}")
    n = len("Hi.")
    old = 1000 + (n % 50)
    tail = proc.stdout[2:]
    if len(tail) < 2:
        raise SystemExit(f"stub PCM too short: {len(proc.stdout)}")
    got = struct.unpack("<h", tail[:2])[0]
    if got == old:
        raise SystemExit("stub PCM ignored exaggeration, cfg and temperature")
    print(
        f"controls exaggeration={asked.exaggeration} cfg={asked.cfg_weight} "
        f"temperature={asked.temperature} pause_samples={gap} stub_sample={got}"
    )


if __name__ == "__main__":
    main()
