#!/usr/bin/env python3
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""V2-S14 out-of-process clone worker.

One long-lived process. Text in, 24 kHz s16le PCM out. Same shape as the warm
Piper worker: one JSON line on stdin, raw PCM on stdout, markers on stderr.

Symphony B landing. Chatterbox Nano stays the model (Turbo is only the
existing quality option). This process is CPU-only and sized to one vCPU:
thread pools are pinned to 1, and a GPU device is refused. No GGUF.

A reference is accepted only when it is one of the two owner-cleared wavs
(t273), the path is the gitignored repo ref or $GALION_CLIP_STORE/<persona>/reference.wav,
and the sha256 matches scripts/clone/REFS.sha256. Anything else exits 3
with no PCM. MP3 is never a reference.

Jarvis hub texture (clicks, alarms, ambience) is a separate layer. The
reference wav is never stripped or denoised. `texture_layer` false leaves
the layer recorded and does not apply it. `texture_layer` true applies it.
Samantha has no hub layer.

Chatterbox (Resemble AI, MIT, copyright 2025) is the model this process
will call later. It is not vendored, and no Apache-licensed source is
copied into this file. The Galion Voice Clip emit is original.

This file does not load torch and does not call Chatterbox generate.
GALION_CLONE_STUB_PCM=1 emits protocol PCM (not a model, not a Flash result).
Without that, and without the pinned safetensors on disk, the process exits 2
with galion-clone-absent. With weights present and the stub flag off, each
request is galion-clone-unwired and no samples are written. Weights are not
downloaded here.
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
REF_KEYS = ("reference", "audio_prompt", "audio_prompt_path", "clip")
THREAD_KEYS = (
    "OMP_NUM_THREADS",
    "OPENBLAS_NUM_THREADS",
    "MKL_NUM_THREADS",
    "NUMEXPR_NUM_THREADS",
    "VECLIB_MAXIMUM_THREADS",
    "TORCH_NUM_THREADS",
    "GALION_CLONE_THREADS",
)
# Voice-session planning figures (t284). This child is pinned to 1 thread,
# which is the whole ~1 vCPU session budget, not the 0.3 TTS share inside it
# and not the cascade burst of ~4. RAM is a planning band, not a measured RSS.
SESSION_VCPU = 1
BURST_VCPU = 4
RAM_GB = "0.65-1.15"


def refuse(why: str) -> None:
    sys.stderr.write(f"galion-clone-refuse: {why}\n")
    sys.stderr.flush()
    raise SystemExit(3)


SPLIT_PLANS = ("split4", "split3")


def pin_cpu() -> tuple:
    """Force the CPU landing. No GPU. No denoise. One thread under the
    session plan; under a split plan (the Rust launcher pinned this process
    with sched_setaffinity) as many threads as asked, never more than the
    cores in this process's affinity mask."""
    asked = os.environ.get("GALION_CLONE_DEVICE", "cpu").strip().lower()
    if asked not in ("", "cpu"):
        refuse(f"clone landing is CPU-only (device {asked})")
    raw = os.environ.get("GALION_CLONE_THREADS", "").strip()
    plan = os.environ.get("GALION_CLONE_CPU_PLAN", "session").strip().lower()
    applied = 1
    if plan in SPLIT_PLANS and raw.isdigit():
        applied = max(1, min(int(raw), len(os.sched_getaffinity(0))))
    for key in THREAD_KEYS:
        os.environ[key] = str(applied)
    os.environ["GALION_CLONE_DEVICE"] = "cpu"
    os.environ["CUDA_VISIBLE_DEVICES"] = ""
    os.environ["HIP_VISIBLE_DEVICES"] = ""
    os.environ["GALION_CLONE_DENOISE"] = "0"
    note = ""
    if raw not in ("", str(applied)):
        note = f" asked={raw} applied={applied}"
    return note, applied


def model_name() -> str:
    model = os.environ.get("GALION_CLONE_MODEL", "nano").strip().lower()
    if model not in ("nano", "turbo"):
        model = "nano"
    if "gguf" in model:
        refuse("no GGUF on the voice path")
    return model


def weights_present(model: str) -> bool:
    root = os.environ.get("GALION_CLONE_WEIGHTS") or os.path.join(REPO, "weights", "clone")
    names = (
        "shared/s3gen.safetensors",
        "shared/s3gen_meanflow.safetensors",
        "shared/ve.safetensors",
        f"{model}/t3_{model}_v1.safetensors",
    )
    return all(os.path.isfile(os.path.join(root, name)) for name in names)


def persona_name(value: object) -> str:
    name = str(value or "samantha").strip().lower()
    return "jarvis" if name == "jarvis" else "samantha"


def load_pins() -> dict:
    path = os.path.join(HERE, "REFS.sha256")
    out = {}
    with open(path, "r", encoding="utf-8") as handle:
        for line in handle:
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            parts = line.split()
            if len(parts) != 9:
                continue
            persona, sha, duration_s, rate_hz, channels, bits, repo_rel, store_rel, texture = parts
            if not repo_rel.endswith(".wav") or not store_rel.endswith(".wav"):
                continue
            if "mp3" in repo_rel or "mp3" in store_rel:
                continue
            if not texture.startswith("preserve"):
                continue
            out[persona] = {
                "sha": sha,
                "duration_s": duration_s,
                "rate_hz": int(rate_hz),
                "channels": int(channels),
                "bits": int(bits),
                "repo_rel": repo_rel,
                "store_rel": store_rel,
                "texture": texture,
            }
    return out


def sha256_file(path: str) -> str:
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def truthy(value: object) -> bool:
    if value is True:
        return True
    if isinstance(value, (int, float)) and value == 1:
        return True
    if isinstance(value, str) and value.strip().lower() in ("1", "true", "yes", "on"):
        return True
    return False


def allowed_realpaths(persona: str, pin: dict) -> set:
    found = {os.path.realpath(os.path.join(REPO, pin["repo_rel"]))}
    store = os.environ.get("GALION_CLIP_STORE", "").strip()
    if store:
        found.add(os.path.realpath(os.path.join(store, pin["store_rel"])))
    return found


def consider_reference(msg: dict, persona: str) -> dict | None:
    for key in ("denoise", "strip", "clean", "normalize"):
        if truthy(msg.get(key)):
            refuse("denoise refused; texture is character")
    texture_req = str(msg.get("texture", "")).strip().lower()
    if texture_req in ("clean", "denoise", "strip", "studio"):
        refuse("denoise refused; texture is character")
    device = str(msg.get("device", "") or "cpu").strip().lower()
    if device not in ("", "cpu"):
        refuse("clone landing is CPU-only")
    for key in ("text", "model", "reference", "audio_prompt_path", "audio_prompt", "clip", "format"):
        if "gguf" in str(msg.get(key, "")).lower():
            refuse("no GGUF on the voice path")
    paths = []
    for key in REF_KEYS:
        value = msg.get(key)
        if not value:
            continue
        if not isinstance(value, str):
            refuse("reference must be a wav path")
        paths.append(value)
    if not paths:
        return None
    if any(item != paths[0] for item in paths):
        refuse("reference fields disagree")
    path = paths[0]
    base = os.path.basename(path).lower()
    if base.endswith(".mp3") or ".mp3" in base or base.endswith((".flac", ".ogg", ".m4a", ".aac")):
        refuse("mp3 is not a clone reference")
    if base not in (f"{persona}.wav", "reference.wav"):
        refuse("reference is not a consented clip (rights hold)")
    pins = load_pins()
    pin = pins.get(persona)
    if pin is None:
        refuse("reference is not a consented clip (rights hold)")
    if not os.path.isfile(path):
        refuse("consented reference absent")
    real = os.path.realpath(path)
    if real not in allowed_realpaths(persona, pin):
        refuse("reference is not a consented clip (rights hold)")
    digest = sha256_file(real)
    if digest != pin["sha"]:
        refuse(f"reference sha256 mismatch for {persona}")
    texture = pin["texture"]
    if texture_req and texture_req != texture:
        refuse("texture does not match the pinned reference")
    if not texture.startswith("preserve"):
        refuse("denoise refused; texture is character")
    return {
        "sha": digest,
        "texture": texture,
        "bytes": os.path.getsize(real),
    }


def texture_layer_state(msg: dict, persona: str) -> dict:
    if truthy(msg.get("strip_texture")) or truthy(msg.get("texture_stripped")):
        refuse("texture layer is not stripped")
    if persona != "jarvis":
        return {"present": 0, "enabled": 0, "stripped": 0}
    raw = msg.get("texture_layer", None)
    if raw is None:
        env = os.environ.get("GALION_JARVIS_TEXTURE", "1").strip().lower()
        enabled = env not in ("0", "false", "off")
    elif isinstance(raw, bool):
        enabled = raw
    else:
        enabled = truthy(raw)
    return {"present": 1, "enabled": 1 if enabled else 0, "stripped": 0}


def log_texture(state: dict, persona: str) -> None:
    sys.stderr.write(
        "galion-clone-texture-layer: persona={persona} present={present} enabled={enabled} stripped={stripped}\n".format(
            persona=persona,
            present=state["present"],
            enabled=state["enabled"],
            stripped=state["stripped"],
        )
    )


def log_ref(ref: dict | None, persona: str) -> None:
    if not ref:
        return
    sys.stderr.write(
        "galion-clone-ref: ok persona={persona} sha256={sha} bytes={nbytes} denoise=false texture={texture}\n".format(
            persona=persona,
            sha=ref["sha"],
            nbytes=ref["bytes"],
            texture=ref["texture"],
        )
    )


def main() -> None:
    asked_note, threads = pin_cpu()
    model = model_name()
    sys.stderr.write(f"galion-clone-cpu: threads={threads} device=cpu denoise=false{asked_note}\n")
    sys.stderr.write(
        "galion-clone-budget: session_vcpu={vcpu} threads={threads} burst_max={burst} ram_gb={ram} device=cpu model={model} gguf=false\n".format(
            threads=threads,
            vcpu=SESSION_VCPU,
            burst=BURST_VCPU,
            ram=RAM_GB,
            model=model,
        )
    )
    sys.stderr.flush()
    stub = os.environ.get("GALION_CLONE_STUB_PCM") == "1"
    if not stub and not weights_present(model):
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
        persona = persona_name(msg.get("persona", "samantha"))
        layer = texture_layer_state(msg, persona)
        ref = consider_reference(msg, persona)
        text = str(msg.get("text", ""))
        if persona == "jarvis":
            text = TAG.sub("", text)
        text = " ".join(text.split())
        if engine != "protocol-stub":
            sys.stderr.write("galion-clone-unwired\n")
            log_ref(ref, persona)
            log_texture(layer, persona)
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
        log_ref(ref, persona)
        log_texture(layer, persona)
        sys.stderr.write("galion-clone-emit: galion-vc\n")
        sys.stderr.write(f"galion-clone-utt: {len(pcm)}\n")
        sys.stderr.flush()


if __name__ == "__main__":
    main()
