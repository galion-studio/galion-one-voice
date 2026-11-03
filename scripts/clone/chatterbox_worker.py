#!/usr/bin/env python3
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""V2-S14 out-of-process clone worker stub.

One long-lived process. Text in, 24 kHz s16le PCM out. Same shape as the warm
Piper worker: one JSON line on stdin, raw PCM on stdout, a marker on stderr.

This stub does not import torch, does not download weights, and does not call
Chatterbox generate. A `reference` path is accepted only when owner clearance
t273 is recorded and the file is a WAV in `$GALION_CLIP_STORE` outside the
work tree (`docs/galion-voice-v2/symphony/CLIP-STORE.md`). No counsel name
is invented here. Other clip keys are still refused. The WAV is not copied
to stdout.

Set GALION_CLONE_STUB_PCM=1 to emit protocol PCM (not a model, not a Flash
result). Without that, and without the pinned safetensors on disk, the
process exits 2 with galion-clone-absent. With weights present and the stub
flag off, each request is galion-clone-unwired and no samples are written.
"""

from __future__ import annotations

import hashlib
import json
import os
import re
import struct
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))
TAG = re.compile(r"\[[A-Za-z][A-Za-z0-9_ ]{0,32}\]")
CLOSED_KEYS = ("audio_prompt", "audio_prompt_path", "clip")
BAD_AUDIO = (
    ".mp3",
    ".flac",
    ".ogg",
    ".opus",
    ".m4a",
    ".aac",
    ".mp4",
    ".mkv",
    ".mov",
    ".webm",
    ".wave",
)


def exact_persona(name: str) -> str | None:
    n = name.strip().lower()
    if n in ("samantha", "jarvis"):
        return n
    return None


def toml_send_reference(persona: str) -> bool:
    path = os.path.join(REPO, "docs", "galion-voice-v2", "personas", "voice", f"{persona}.toml")
    try:
        text = open(path, encoding="utf-8").read()
    except OSError:
        return False
    in_worker = False
    for line in text.splitlines():
        s = line.strip()
        if s.startswith("[") and s.endswith("]"):
            in_worker = s == "[worker]"
            continue
        if in_worker and s.startswith("send_reference"):
            return s.split("=", 1)[-1].strip().lower() == "true"
    return False


def clearance_text() -> str | None:
    override = os.environ.get("GALION_CLIP_CLEARANCE", "").strip()
    path = override or os.path.join(REPO, "docs", "galion-voice-v2", "symphony", "clearance-t273.rec")
    try:
        return open(path, encoding="utf-8").read()
    except OSError:
        return None


def record_allows(text: str, persona: str) -> bool:
    got: dict[str, str] = {}
    for line in text.splitlines():
        s = line.strip()
        if not s or s.startswith("#") or "=" not in s:
            continue
        k, v = s.split("=", 1)
        got[k.strip()] = v.strip()
    if "counsel" not in got:
        return False
    personas = [p.strip() for p in got.get("personas", "").split(",") if p.strip()]
    return (
        got.get("decision") == "t273"
        and got.get("owner") == "Maciej Grajczyk"
        and bool(got.get("recorded"))
        and got.get("scope") == "local-zero-shot-conditioning"
        and got.get("public_demo") == "no"
        and got.get("private_cleared") == "yes"
        and got.get("wav_only") == "yes"
        and got.get("lock_edited") == "no"
        and got.get("send_reference") == "yes"
        and persona in personas
    )


def ledger_private_cleared(persona: str) -> bool:
    needle = {
        "samantha": "Samantha · PRIVATE CLEARED",
        "jarvis": "Jarvis · PRIVATE CLEARED",
    }.get(persona)
    if needle is None:
        return False
    path = os.path.join(REPO, "LEDGER.md")
    try:
        lines = open(path, encoding="utf-8").read().splitlines()
    except OSError:
        return False
    return any(needle in line and "t273" in line and "Maciej Grajczyk" in line for line in lines)


def clearance_allows(persona: str) -> bool:
    if exact_persona(persona) is None or not toml_send_reference(persona):
        return False
    text = clearance_text()
    return text is not None and record_allows(text, persona) and ledger_private_cleared(persona)


def jarvis_texture(value: object) -> str | None:
    """`hub` or `off` for Jarvis. None when the key is absent. A bad value is refused by the caller."""
    if value is None or value == "":
        return None
    if value in ("hub", "off"):
        return str(value)
    return ""


def sha256_file(path: str) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def reference_refusal(persona: str, value: object) -> str | None:
    """None means the reference may pass. A string is the refusal reason."""
    if not isinstance(value, str) or not value.strip():
        if value:
            return "reference clips are on the rights hold"
        return None
    raw = value.strip()
    low = raw.lower()
    if low.endswith(BAD_AUDIO) or ".mp3" in low:
        return "wav only"
    if not low.endswith(".wav"):
        return "wav only"
    if not clearance_allows(persona):
        return "reference clips are on the rights hold"
    store = os.environ.get("GALION_CLIP_STORE", "").strip()
    if not store or not os.path.isabs(store):
        return "clip store"
    store_r = os.path.realpath(store)
    repo_r = os.path.realpath(REPO)
    if store_r == repo_r or store_r.startswith(repo_r + os.sep):
        return "clip store"
    persona_dir = os.path.realpath(os.path.join(store_r, persona))
    wav = os.path.realpath(os.path.join(persona_dir, "reference.wav"))
    if not wav.startswith(persona_dir + os.sep):
        return "clip store"
    if wav == repo_r or wav.startswith(repo_r + os.sep):
        return "clip store"
    if not os.path.isfile(raw):
        return "clip store"
    if os.path.realpath(raw) != wav:
        return "clip store"
    try:
        with open(wav, "rb") as f:
            head = f.read(12)
    except OSError:
        return "clip store"
    if len(head) < 12 or head[:4] != b"RIFF" or head[8:12] != b"WAVE":
        return "wav only"
    side = wav + ".sha256"
    try:
        token = open(side, encoding="utf-8").read().split()
    except OSError:
        return "sha256"
    if not token or len(token[0]) != 64 or any(c not in "0123456789abcdefABCDEF" for c in token[0]):
        return "sha256"
    try:
        got = sha256_file(wav)
    except OSError:
        return "sha256"
    if token[0].lower() != got:
        return "sha256"
    return None


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
        for key in CLOSED_KEYS:
            if msg.get(key):
                refuse("reference clips are on the rights hold")
        text = str(msg.get("text", ""))
        persona = str(msg.get("persona", "samantha")).strip().lower()
        if persona not in ("samantha", "jarvis"):
            persona = "samantha"
        ref_ok = False
        if "reference" in msg and msg.get("reference"):
            why = reference_refusal(persona, msg.get("reference"))
            if why:
                refuse(why)
            ref_ok = True
            sys.stderr.write("galion-clone-reference: wav\n")
        layer = jarvis_texture(msg.get("texture"))
        if layer == "":
            refuse("texture")
        if layer is not None and persona != "jarvis":
            refuse("texture")
        if ref_ok and persona == "jarvis":
            if layer is None:
                env = os.environ.get("GALION_JARVIS_TEXTURE", "").strip()
                layer = "off" if env == "0" or env.lower() == "off" else "hub"
            sys.stderr.write(f"galion-clone-texture: {layer}\n")
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
        pcm = struct.pack("<h", n) + struct.pack("<200h", *([tone] * 200))
        sys.stdout.buffer.write(pcm)
        sys.stdout.buffer.flush()
        sys.stderr.write("galion-clone-engine: protocol-stub\n")
        sys.stderr.write(f"galion-clone-persona: {persona}\n")
        sys.stderr.write(f"galion-clone-utt: {len(pcm)}\n")
        sys.stderr.flush()


if __name__ == "__main__":
    main()
