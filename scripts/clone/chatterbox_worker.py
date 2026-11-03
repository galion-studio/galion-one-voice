#!/usr/bin/env python3
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""V2-S14 out-of-process Chatterbox worker. Text in, 24 kHz s16le PCM out.

One long-lived child, same shape as the warm Piper worker. PyTorch stays in
this process. Nothing in a Galion crate links it. The device is CPU. This
process does not set a thread cap (t204u, t205u).

GALION_CLONE_STUB_PCM=1 emits protocol PCM and does not import torch.
Without that flag, missing pinned files exit 2 (galion-clone-absent).
GALION_CLONE_GENERATE=1 loads Chatterbox Nano (or Turbo) from the local
safetensors and calls generate. There is no network fetch in this process.

A speak line still has no reference path. audio_prompt_path on a speak line
exits 3 and writes no PCM. Conditioning is a separate stdin op, and only
after scripts/clone/consent.py accepts the persona. Until that gate is open
the child refuses the op. Perth, if generate runs, stays inside the upstream
call. It is not vendored here. A missing import writes no samples.
"""

from __future__ import annotations

import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import consent

TAG = re.compile(r"\[[A-Za-z][A-Za-z0-9_ ]{0,32}\]")
KEEP_TAGS = ("[laugh]", "[chuckle]")
REF_KEYS = ("reference", "audio_prompt", "audio_prompt_path", "clip")
RATE = 24000


def weights_present() -> bool:
    return consent.weights_sized(consent.model_name())


def refuse(why: str) -> None:
    sys.stderr.write(f"galion-clone-refuse: {why}\n")
    sys.stderr.flush()
    raise SystemExit(3)


def persona_of(msg: dict) -> str:
    name = consent.persona_of(str(msg.get("persona", "samantha")))
    return name or "samantha"


def strip_tags(text: str, keep: bool) -> str:
    if not keep:
        return TAG.sub("", text)

    def repl(match: re.Match[str]) -> str:
        token = match.group(0)
        return token if token in KEEP_TAGS else ""

    return TAG.sub(repl, text)


def unwired(engine: str) -> None:
    sys.stderr.write("galion-clone-unwired\n")
    sys.stderr.write(f"galion-clone-engine: {engine}\n")
    sys.stderr.write("galion-clone-utt: 0\n")
    sys.stderr.flush()


def mark_conditioned(persona: str, seconds: float) -> None:
    sys.stderr.write(f"galion-clone-conditioned: {persona} {seconds:.3f}\n")
    sys.stderr.write("galion-clone-utt: 0\n")
    sys.stderr.flush()


def stage_ckpt(root: str, model: str) -> str:
    import atexit
    import shutil
    import tempfile

    stage = tempfile.mkdtemp(prefix="galion-clone-ckpt-")
    atexit.register(lambda: shutil.rmtree(stage, ignore_errors=True))
    t3 = "t3_nano_v1.safetensors" if model == "nano" else "t3_turbo_v1.safetensors"
    links = {
        "ve.safetensors": os.path.join(root, "shared", "ve.safetensors"),
        "s3gen_meanflow.safetensors": os.path.join(root, "shared", "s3gen_meanflow.safetensors"),
        t3: os.path.join(root, model, t3),
    }
    for name in consent.TOKENIZER:
        links[os.path.basename(name)] = os.path.join(root, name)
    for dest, src in links.items():
        os.symlink(os.path.abspath(src), os.path.join(stage, dest))
    return stage


def load_model(model: str):
    """Import PyTorch only on the generate path. CPU. No thread cap. No download."""
    os.environ["HF_HUB_OFFLINE"] = "1"
    os.environ["TRANSFORMERS_OFFLINE"] = "1"
    import torch
    from chatterbox.tts_turbo import ChatterboxTurboTTS

    torch.set_grad_enabled(False)
    root = consent.weights_root()
    stage = stage_ckpt(root, model)
    tts = ChatterboxTurboTTS.from_local(stage, device="cpu", nano=(model == "nano"))
    tts.conds = None
    return tts


def pcm_bytes(wav, sr: int) -> bytes:
    import numpy as np

    samples = wav.squeeze().detach().cpu().numpy().astype("float64")
    if int(sr) != RATE:
        import librosa

        samples = librosa.resample(samples, orig_sr=int(sr), target_sr=RATE)
    samples = np.clip(samples, -1.0, 1.0)
    return (samples * 32767.0).astype("<i2").tobytes()


def same_file(left: str, right: str) -> bool:
    try:
        return os.path.realpath(left) == os.path.realpath(right)
    except OSError:
        return False


def main() -> None:
    real_out = sys.stdout.buffer
    sys.stdout = sys.stderr
    stub = os.environ.get("GALION_CLONE_STUB_PCM") == "1"
    generate = os.environ.get("GALION_CLONE_GENERATE") == "1" and not stub
    model_id = consent.model_name()
    if not stub and not weights_present():
        sys.stderr.write("galion-clone-absent\n")
        sys.stderr.flush()
        raise SystemExit(2)
    model = None
    if generate:
        try:
            model = load_model(model_id)
        except Exception:
            sys.stderr.write("galion-clone-fault: import\n")
            sys.stderr.flush()
            raise SystemExit(1)
        sys.stderr.write(f"galion-clone-ready: {model_id} cpu {RATE}\n")
        sys.stderr.flush()
    engine = "protocol-stub" if stub else ("model" if model is not None else "unwired")
    conds: dict[str, str] = {}
    last = ""
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
        op = str(msg.get("op", "speak")).strip().lower()
        persona = persona_of(msg)
        if op == "condition":
            # Weight sizes are checked at startup when generate is armed, and
            # again by consent.py check before the parent sends this op.
            gate = consent.evaluate(persona, need_weights=False)
            if not gate.ok or gate.wav is None or gate.seconds is None:
                refuse(gate.reason if gate.reason != "lock" else "reference_clips is false")
            sent = ""
            for key in REF_KEYS:
                value = msg.get(key)
                if value:
                    sent = str(value)
                    break
            if sent and not same_file(sent, gate.wav):
                refuse("path is not the consented reference")
            if model is not None:
                try:
                    model.prepare_conditionals(gate.wav)
                except Exception:
                    refuse("condition failed")
                last = persona
            conds[persona] = gate.wav
            mark_conditioned(persona, gate.seconds)
            continue
        if op == "drop":
            conds.pop(persona, None)
            if last == persona:
                last = ""
                if model is not None:
                    model.conds = None
            sys.stderr.write(f"galion-clone-dropped: {persona}\n")
            sys.stderr.write("galion-clone-utt: 0\n")
            sys.stderr.flush()
            continue
        if op not in ("speak", ""):
            refuse("unknown op")
        for key in REF_KEYS:
            if msg.get(key):
                refuse("reference clips are on the rights hold")
        text = str(msg.get("text", ""))
        keep = bool(msg.get("tags", persona != "jarvis")) and persona != "jarvis"
        text = strip_tags(text, keep)
        text = " ".join(text.split())
        if engine == "model":
            if persona not in conds or model is None:
                unwired("unwired")
                continue
            try:
                if last != persona:
                    model.prepare_conditionals(conds[persona])
                    last = persona
                wav = model.generate(text)
                pcm = pcm_bytes(wav, int(getattr(model, "sr", RATE)))
            except Exception:
                unwired("unwired")
                continue
            if len(pcm) < 2:
                unwired("unwired")
                continue
            real_out.write(pcm)
            real_out.flush()
            sys.stderr.write("galion-clone-engine: model\n")
            sys.stderr.write(f"galion-clone-persona: {persona}\n")
            sys.stderr.write(f"galion-clone-utt: {len(pcm)}\n")
            sys.stderr.flush()
            continue
        if engine != "protocol-stub":
            unwired("unwired")
            continue
        import struct

        n = max(1, min(len(text), 32767))
        tone = 1000 + (n % 50)
        pcm = struct.pack("<h", n) + struct.pack("<200h", *([tone] * 200))
        real_out.write(pcm)
        real_out.flush()
        sys.stderr.write("galion-clone-engine: protocol-stub\n")
        sys.stderr.write(f"galion-clone-persona: {persona}\n")
        sys.stderr.write(f"galion-clone-utt: {len(pcm)}\n")
        sys.stderr.flush()


if __name__ == "__main__":
    main()
