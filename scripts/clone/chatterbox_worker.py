#!/usr/bin/env python3
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""V2-S14 out-of-process clone worker stub.

One long-lived process. Text in, 24 kHz s16le PCM out. Same shape as the warm
Piper worker: one JSON line on stdin, raw PCM on stdout, a marker on stderr.

This stub does not import torch, does not download weights, and does not call
Chatterbox generate. A non-empty reference clip is refused. The rights hold
on film audio is unchanged.

Set GALION_CLONE_STUB_PCM=1 to emit protocol PCM (not a model, not a Flash
result). GALION_CLONE_STUB_FRAMES=N (1..8) emits N heartbeat frames of that
protocol PCM: 1920 samples at 24 kHz, 80 ms, progress on stderr, PCM on
stdout. Without the stub flag, and without the pinned safetensors on disk,
the process exits 2 with galion-clone-absent. With weights present and the
stub flag off, each request is galion-clone-unwired and no samples are
written. This file does not call Chatterbox generate.
"""

from __future__ import annotations

import json
import os
import re
import struct
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))
TAG = re.compile(r"\[[A-Za-z][A-Za-z0-9_ ]{0,32}\]")
REF_KEYS = ("reference", "audio_prompt", "audio_prompt_path", "clip")
# One Galion heartbeat frame: 80 ms at 24 kHz.
FRAME = 1920


def weights_present() -> bool:
    root = os.environ.get("GALION_CLONE_WEIGHTS") or os.path.join(REPO, "weights", "clone")
    model = os.environ.get("GALION_CLONE_MODEL", "nano").strip().lower()
    if model not in ("nano", "turbo"):
        model = "nano"
    names = (
        "shared/s3gen.safetensors",
        "shared/s3gen_meanflow.safetensors",
        "shared/ve.safetensors",
        f"{model}/t3_{model}_v1.safetensors",
    )
    return all(os.path.isfile(os.path.join(root, name)) for name in names)


def refuse(why: str) -> None:
    sys.stderr.write(f"galion-clone-refuse: {why}\n")
    sys.stderr.flush()
    raise SystemExit(3)


def stub_frame_count() -> int:
    raw = os.environ.get("GALION_CLONE_STUB_FRAMES", "").strip()
    if not raw:
        return 0
    try:
        n = int(raw)
    except ValueError:
        return 0
    if n <= 0:
        return 0
    return min(n, 8)


def main() -> None:
    stub = os.environ.get("GALION_CLONE_STUB_PCM") == "1"
    if not stub and not weights_present():
        sys.stderr.write("galion-clone-absent\n")
        sys.stderr.flush()
        raise SystemExit(2)
    engine = "protocol-stub" if stub else "unwired"
    while True:
        line = sys.stdin.buffer.readline()
        if not line:
            return
        try:
            msg = json.loads(line.decode())
        except json.JSONDecodeError:
            refuse("bad json")
        if not isinstance(msg, dict):
            refuse("bad json")
        for key in REF_KEYS:
            value = msg.get(key)
            if value:
                refuse("reference clips are on the rights hold")
        text = str(msg.get("text", ""))
        persona = str(msg.get("persona", "samantha")).strip().lower()
        if persona == "jarvis":
            text = TAG.sub("", text)
        text = " ".join(text.split())
        if engine != "protocol-stub":
            sys.stderr.write("galion-clone-unwired\n")
            sys.stderr.write("galion-clone-engine: unwired\n")
            sys.stderr.write("galion-clone-utt: 0\n")
            sys.stderr.flush()
            continue
        n = max(1, min(len(text), 32767))
        tone = 1000 + (n % 50)
        frames = stub_frame_count()
        if frames:
            one = struct.pack("<h", tone) * FRAME
            parts = []
            for i in range(frames):
                sys.stderr.write(f"galion-clone-progress: {i}\n")
                sys.stderr.flush()
                sys.stdout.buffer.write(one)
                sys.stdout.buffer.flush()
                parts.append(one)
            pcm = b"".join(parts)
        else:
            pcm = struct.pack("<h", n) + struct.pack("<200h", *([tone] * 200))
            sys.stdout.buffer.write(pcm)
            sys.stdout.buffer.flush()
        sys.stderr.write("galion-clone-engine: protocol-stub\n")
        sys.stderr.write(f"galion-clone-persona: {persona}\n")
        sys.stderr.write(f"galion-clone-utt: {len(pcm)}\n")
        sys.stderr.flush()


if __name__ == "__main__":
    main()
