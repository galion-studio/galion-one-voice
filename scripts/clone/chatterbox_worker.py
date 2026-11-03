#!/usr/bin/env python3
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""V2-S14 out-of-process clone worker.

One long-lived process. One JSON line on stdin, raw 24 kHz s16le on
stdout, markers on stderr. PCM is framed as chunks so the parent can
clock the first bytes before the utterance marker.

``GALION_CLONE_STUB_PCM=1`` writes protocol PCM in two chunks. It does
not import torch. Without that flag, and without the pinned safetensors,
the process exits 2. With the files present and ``GALION_CLONE_STREAM``
unset, each request is ``galion-clone-unwired`` and no samples are written.

``GALION_CLONE_STREAM=1`` loads Chatterbox and streams. The load is
refused unless ``GALION_CLONE_CKPT`` is a flat checkpoint directory.
``generate()`` is not called. A reference clip still exits 3 unless
``GALION_CLONE_LOCAL_REFS=1`` and the sha256 is on the operator allowlist.
LOCKS.toml is not read.

Stdout is sealed onto stderr before the model import. Upstream meanflow
prints on stdout, and that pipe is the PCM wire.
"""

from __future__ import annotations

import json
import os
import re
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
if HERE not in sys.path:
    sys.path.insert(0, HERE)

import nano_stream

REPO = os.path.dirname(os.path.dirname(HERE))
TAG = re.compile(r"\[[A-Za-z][A-Za-z0-9_ ]{0,32}\]")
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


def seal_pcm_stdout():
    """Keep library prints off the PCM pipe. Returns the real stdout."""
    pcm_fd = os.dup(1)
    os.dup2(2, 1)
    sys.stdout = sys.stderr
    return os.fdopen(pcm_fd, "wb", buffering=0)


def paced_protocol(text: str):
    chunks = nano_stream.protocol_chunks(text)
    for index, chunk in enumerate(chunks):
        if index:
            time.sleep(0.05)
        yield chunk


def main() -> None:
    stub = os.environ.get("GALION_CLONE_STUB_PCM") == "1"
    want_stream = os.environ.get("GALION_CLONE_STREAM") == "1" and not stub
    if not stub and not weights_present():
        sys.stderr.write("galion-clone-absent\n")
        sys.stderr.flush()
        raise SystemExit(2)
    pcm = sys.stdout.buffer
    model = None
    load_note = ""
    if want_stream:
        pcm = seal_pcm_stdout()
        model, load_note = nano_stream.try_load()
    while True:
        line = sys.stdin.buffer.readline()
        if not line:
            return
        try:
            msg = json.loads(line.decode())
        except Exception:
            refuse("bad json")
        if not isinstance(msg, dict):
            refuse("bad json")
        text = str(msg.get("text", ""))
        persona = str(msg.get("persona", "samantha")).strip().lower()
        if persona != "jarvis":
            persona = "samantha"
        ref_path, ref_why = nano_stream.ref_decision(persona, msg)
        if ref_why:
            refuse(ref_why)
        if persona == "jarvis":
            text = TAG.sub("", text)
        text = " ".join(text.split())
        if stub:
            nano_stream.write_chunks(
                pcm,
                paced_protocol(text),
                engine="protocol-stub",
                persona=persona,
                path="stream",
                err=sys.stderr,
            )
            continue
        if model is None:
            why = load_note or "GALION_CLONE_STREAM is not 1; whole-utterance generate is not used"
            nano_stream.write_unwired(sys.stderr, why)
            continue
        if not text:
            nano_stream.write_unwired(sys.stderr, "empty text")
            continue
        try:
            nano_stream.write_chunks(
                pcm,
                nano_stream.iter_pcm(model, text, ref_path),
                engine="model",
                persona=persona,
                path="stream",
                err=sys.stderr,
            )
        except nano_stream.NoStreamHook as exc:
            nano_stream.write_unwired(sys.stderr, str(exc))


if __name__ == "__main__":
    main()
