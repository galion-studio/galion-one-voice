#!/usr/bin/env python3
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""V2-S14 out-of-process Chatterbox worker.

One long-lived process. Text in, 24 kHz s16le PCM out. Same shape as the warm
Piper worker: one JSON line on stdin, raw PCM on stdout, progress and the
utterance marker on stderr. Generate runs in this process, never inside
Heartbeat::step.

Nano is the primary (`GALION_CLONE_MODEL=nano`). Turbo is the same process
with `GALION_CLONE_MODEL=turbo`.

A reference clip is never accepted on the JSON line (`reference`,
`audio_prompt`, `audio_prompt_path`, `clip` → exit 3, no PCM). The child
opens a local file only when `GALION_CLONE_LOCAL_REFS=1` and
`$GALION_CLIP_STORE/<persona>/reference.wav` matches its `.sha256` sidecar.
That gate does not flip `LOCKS.toml` `reference_clips`. Missing weights,
a sha miss, a missing tokenizer, or a missing `chatterbox` install writes
`galion-clone-unwired` and no samples, so the host speaks with Piper.

`GALION_CLONE_STUB_PCM=1` emits protocol PCM. That is not a model and not
a Flash result. This process does not download weights or clips.

Prosody numbers on the request are the 80 ms `galion-emotion` decision.
They are logged. This process does not add empathy sentences and does not
invent an emotion label.
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
PINS = os.path.join(HERE, "PINS.sha256")
MODEL = None
ENGINE = "unwired"
PINS_OK = None
LOAD_FAILED = False


def weights_root() -> str:
    return os.environ.get("GALION_CLONE_WEIGHTS") or os.path.join(REPO, "weights", "clone")


def model_name() -> str:
    name = os.environ.get("GALION_CLONE_MODEL", "nano").strip().lower()
    return name if name in ("nano", "turbo") else "nano"


def weights_present() -> bool:
    root = weights_root()
    model = model_name()
    names = (
        "shared/s3gen.safetensors",
        "shared/s3gen_meanflow.safetensors",
        "shared/ve.safetensors",
        f"{model}/t3_{model}_v1.safetensors",
    )
    flat = {
        "shared/s3gen.safetensors": "s3gen.safetensors",
        "shared/s3gen_meanflow.safetensors": "s3gen_meanflow.safetensors",
        "shared/ve.safetensors": "ve.safetensors",
        f"{model}/t3_{model}_v1.safetensors": f"t3_{model}_v1.safetensors",
    }
    roots = [root]
    ckpt = os.environ.get("GALION_CLONE_CKPT", "").strip()
    if ckpt:
        roots.append(ckpt)
    def has(rel: str) -> bool:
        base = flat[rel]
        for r in roots:
            if os.path.isfile(os.path.join(r, rel)) or os.path.isfile(os.path.join(r, base)):
                return True
        return False
    return all(has(name) for name in names)


def load_pins() -> dict:
    out = {}
    try:
        with open(PINS, "r", encoding="utf-8") as handle:
            for line in handle:
                line = line.strip()
                if not line or line.startswith("#"):
                    continue
                parts = line.split()
                if len(parts) < 3:
                    continue
                rel, sha, size = parts[0], parts[1].lower(), parts[2]
                if len(sha) != 64:
                    continue
                try:
                    nbytes = int(size)
                except ValueError:
                    continue
                out[rel] = (sha, nbytes)
    except OSError:
        return {}
    return out


def file_matches(path: str, sha: str, size: int) -> bool:
    try:
        if os.path.getsize(path) != size:
            return False
    except OSError:
        return False
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest() == sha


def find_pinned(rel: str, sha: str, size: int) -> str | None:
    base = os.path.basename(rel)
    roots = [weights_root()]
    ckpt = os.environ.get("GALION_CLONE_CKPT", "").strip()
    if ckpt:
        roots.append(ckpt)
    seen = []
    for root in roots:
        for path in (os.path.join(root, rel), os.path.join(root, base)):
            if path not in seen:
                seen.append(path)
    for path in seen:
        if file_matches(path, sha, size):
            return path
    return None


def pins_ok(model: str) -> bool:
    global PINS_OK
    if PINS_OK is not None:
        return PINS_OK
    table = load_pins()
    need = (
        "shared/s3gen.safetensors",
        "shared/s3gen_meanflow.safetensors",
        "shared/ve.safetensors",
        f"{model}/t3_{model}_v1.safetensors",
    )
    PINS_OK = all(rel in table and find_pinned(rel, *table[rel]) for rel in need)
    return PINS_OK


def tokenizer_dir() -> str | None:
    candidates = []
    for key in ("GALION_CLONE_TOKENIZER", "GALION_CLONE_CKPT"):
        value = os.environ.get(key, "").strip()
        if value:
            candidates.append(value)
    candidates.append(os.path.join(weights_root(), "tokenizer"))
    for directory in candidates:
        if os.path.isfile(os.path.join(directory, "tokenizer.json")):
            return directory
    return None


def stage_ckpt(model: str) -> str | None:
    """Flat directory `from_local` can open. Symlinks only. No copies of weights."""
    tok = tokenizer_dir()
    if tok is None:
        return None
    explicit = os.environ.get("GALION_CLONE_CKPT", "").strip()
    t3 = f"t3_{model}_v1.safetensors"
    if explicit and os.path.isfile(os.path.join(explicit, "tokenizer.json")) and os.path.isfile(os.path.join(explicit, t3)) and os.path.isfile(os.path.join(explicit, "ve.safetensors")) and os.path.isfile(os.path.join(explicit, "s3gen_meanflow.safetensors")):
        return explicit
    table = load_pins()
    mapping = {
        "ve.safetensors": "shared/ve.safetensors",
        "s3gen_meanflow.safetensors": "shared/s3gen_meanflow.safetensors",
        t3: f"{model}/{t3}",
    }
    stage = os.path.join(weights_root(), f".ckpt-{model}")
    os.makedirs(stage, exist_ok=True)
    for name, rel in mapping.items():
        pin = table.get(rel)
        if pin is None:
            return None
        src = find_pinned(rel, *pin)
        if src is None:
            return None
        dest = os.path.join(stage, name)
        if os.path.lexists(dest):
            try:
                if os.path.samefile(dest, src):
                    continue
            except OSError:
                pass
            os.remove(dest)
        os.symlink(os.path.abspath(src), dest)
    for name in (
        "tokenizer.json",
        "tokenizer_config.json",
        "special_tokens_map.json",
        "vocab.json",
        "merges.txt",
        "added_tokens.json",
    ):
        src = os.path.join(tok, name)
        if not os.path.isfile(src):
            continue
        dest = os.path.join(stage, name)
        if os.path.lexists(dest):
            try:
                if os.path.samefile(dest, src):
                    continue
            except OSError:
                pass
            os.remove(dest)
        os.symlink(os.path.abspath(src), dest)
    if not os.path.isfile(os.path.join(stage, "tokenizer.json")):
        return None
    return stage


def local_refs_on() -> bool:
    return os.environ.get("GALION_CLONE_LOCAL_REFS") == "1"


def allowlisted_clip(persona: str) -> tuple[str, str] | None:
    """(path, sha) when the sidecar matches the wav. Otherwise None. No JSON path."""
    if not local_refs_on():
        return None
    store = os.environ.get("GALION_CLIP_STORE", "").strip()
    if not store or persona not in ("samantha", "jarvis"):
        return None
    wav = os.path.join(store, persona, "reference.wav")
    side = wav + ".sha256"
    try:
        with open(side, "r", encoding="utf-8") as handle:
            expect = handle.read().split()[0].strip().lower()
    except (OSError, IndexError):
        return None
    if len(expect) != 64 or any(c not in "0123456789abcdef" for c in expect):
        return None
    digest = hashlib.sha256()
    try:
        with open(wav, "rb") as handle:
            for chunk in iter(lambda: handle.read(1 << 20), b""):
                digest.update(chunk)
    except OSError:
        return None
    if digest.hexdigest() != expect:
        return None
    return wav, expect


def refuse(why: str) -> None:
    sys.stderr.write(f"galion-clone-refuse: {why}\n")
    sys.stderr.flush()
    raise SystemExit(3)


def unwired(why: str, persona: str) -> None:
    sys.stderr.write(f"galion-clone-unwired: {why}\n")
    sys.stderr.write("galion-clone-engine: unwired\n")
    sys.stderr.write(f"galion-clone-persona: {persona}\n")
    sys.stderr.write("galion-clone-utt: 0\n")
    sys.stderr.flush()


def ensure_model(model: str) -> bool:
    global MODEL, ENGINE, LOAD_FAILED
    if MODEL is not None:
        return True
    if LOAD_FAILED:
        return False
    sys.stderr.write("galion-clone-progress: pins\n")
    sys.stderr.flush()
    if not pins_ok(model):
        return False
    ckpt = stage_ckpt(model)
    if not ckpt:
        return False
    os.environ.setdefault("HF_HUB_OFFLINE", "1")
    os.environ.setdefault("TRANSFORMERS_OFFLINE", "1")
    os.environ.setdefault("HF_HUB_DISABLE_TELEMETRY", "1")
    sys.stderr.write("galion-clone-progress: import\n")
    sys.stderr.flush()
    try:
        from chatterbox.tts_turbo import ChatterboxTurboTTS
    except Exception as exc:
        LOAD_FAILED = True
        sys.stderr.write(f"galion-clone-unwired: import {type(exc).__name__}\n")
        sys.stderr.flush()
        return False
    sys.stderr.write("galion-clone-progress: load\n")
    sys.stderr.flush()
    try:
        MODEL = ChatterboxTurboTTS.from_local(ckpt, device="cpu", nano=(model == "nano"))
    except Exception as exc:
        LOAD_FAILED = True
        sys.stderr.write(f"galion-clone-unwired: load {type(exc).__name__}\n")
        sys.stderr.flush()
        return False
    ENGINE = "chatterbox-nano" if model == "nano" else "chatterbox-turbo"
    return True


def generate_pcm(text: str, clip: str) -> bytes | None:
    """Run generate with fd 1 pointed at stderr so library prints cannot enter the PCM pipe."""
    sys.stdout.flush()
    saved = os.dup(1)
    try:
        os.dup2(2, 1)
        sys.stderr.write("galion-clone-progress: generate\n")
        sys.stderr.flush()
        wav = MODEL.generate(text, audio_prompt_path=clip)
    except Exception as exc:
        sys.stderr.write(f"galion-clone-unwired: generate {type(exc).__name__}\n")
        sys.stderr.flush()
        return None
    finally:
        sys.stdout.flush()
        os.dup2(saved, 1)
        os.close(saved)
    try:
        import numpy as np
    except Exception as exc:
        sys.stderr.write(f"galion-clone-unwired: numpy {type(exc).__name__}\n")
        sys.stderr.flush()
        return None
    array = wav
    if hasattr(array, "detach"):
        array = array.detach().float().cpu().numpy()
    samples = np.asarray(array, dtype=np.float32).reshape(-1)
    sr = int(getattr(MODEL, "sr", 24000) or 24000)
    if sr != 24000 and samples.size:
        n = int(round(samples.size * 24000 / sr))
        src = np.linspace(0.0, 1.0, samples.size, endpoint=False)
        dst = np.linspace(0.0, 1.0, max(n, 1), endpoint=False)
        samples = np.interp(dst, src, samples).astype(np.float32)
    samples = np.clip(samples, -1.0, 1.0)
    pcm = (samples * 32767.0).astype("<i2")
    return pcm.tobytes()


def log_prosody(msg: dict) -> None:
    raw = msg.get("prosody")
    if not isinstance(raw, dict):
        return
    def num(key: str, default: float) -> float:
        try:
            value = float(raw.get(key, default))
        except (TypeError, ValueError):
            return default
        if value != value or value in (float("inf"), float("-inf")):
            return default
        return value
    try:
        pause = int(raw.get("pre_pause_ms", 0))
    except (TypeError, ValueError):
        pause = 0
    sys.stderr.write(
        "galion-clone-prosody: pitch_st=%.4f range=%.4f rate=%.4f energy=%.4f warmth=%.4f pre_pause_ms=%d\n"
        % (
            num("pitch_st", 0.0),
            num("range", 1.0),
            num("rate", 1.0),
            num("energy", 1.0),
            num("warmth", 0.5),
            pause,
        )
    )


def emit_stub(text: str, persona: str) -> None:
    n = max(1, min(len(text), 32767))
    tone = 1000 + (n % 50)
    pcm = struct.pack("<h", n) + struct.pack("<200h", *([tone] * 200))
    sys.stdout.buffer.write(pcm)
    sys.stdout.buffer.flush()
    sys.stderr.write("galion-clone-engine: protocol-stub\n")
    sys.stderr.write(f"galion-clone-persona: {persona}\n")
    sys.stderr.write(f"galion-clone-utt: {len(pcm)}\n")
    sys.stderr.flush()


def main() -> None:
    stub = os.environ.get("GALION_CLONE_STUB_PCM") == "1"
    if not stub and not weights_present():
        sys.stderr.write("galion-clone-absent\n")
        sys.stderr.flush()
        raise SystemExit(2)
    model = model_name()
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
        if persona != "jarvis":
            persona = "samantha"
        if persona == "jarvis":
            text = TAG.sub("", text)
        text = " ".join(text.split())
        log_prosody(msg)
        if stub:
            emit_stub(text, persona)
            continue
        clip = allowlisted_clip(persona)
        if clip is None:
            unwired("ref", persona)
            continue
        _path, sha = clip
        sys.stderr.write(f"galion-clone-ref: {persona} {sha[:12]}\n")
        sys.stderr.flush()
        if not ensure_model(model):
            why = "pins" if not pins_ok(model) else "tokenizer" if tokenizer_dir() is None else "model"
            unwired(why, persona)
            continue
        pcm = generate_pcm(text, clip[0])
        if not pcm:
            unwired("generate", persona)
            continue
        sys.stdout.buffer.write(pcm)
        sys.stdout.buffer.flush()
        sys.stderr.write(f"galion-clone-engine: {ENGINE}\n")
        sys.stderr.write(f"galion-clone-persona: {persona}\n")
        sys.stderr.write(f"galion-clone-utt: {len(pcm)}\n")
        sys.stderr.flush()


if __name__ == "__main__":
    main()
