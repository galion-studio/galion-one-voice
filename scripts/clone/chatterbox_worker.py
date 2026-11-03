#!/usr/bin/env python3
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Out-of-process Chatterbox Nano worker.

One long-lived process. One JSON line on stdin. Raw 24 kHz s16le on a
private copy of stdout. Progress and framing on stderr.

Upstream Chatterbox prints to stdout (`loaded PerthNet`, `S3 Token -> Mel`).
That corrupts the PCM pipe. This process dups fd 1 for PCM, then points fd 1
and sys.stdout at stderr, before torch or chatterbox is imported.

The utterance marker is written and flushed before any PCM byte. A caller
that waits on stderr first does not deadlock once the stdout pipe fills.
`galion-clone-utt: N` is the byte count that follows.

Generate runs only in this process. It is not on Heartbeat::step. Piper
remains the fallback when this child is absent, unwired, or refused.

A reference is refused unless GALION_CLONE_LOCAL_REFS=1 and the file's
sha256 matches scripts/clone/REFS.sha256 for the requested persona.
LOCKS.toml is not read and is not the switch. Missing weights or a missing
Nano runtime exit 2 with galion-clone-absent and no PCM.

GALION_CLONE_STUB_PCM=1 stays protocol PCM. It does not load a model.
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
ALLOWLIST_PATH = os.path.join(HERE, "REFS.sha256")
TOKENIZER_PINS = os.path.join(HERE, "TOKENIZER.sha256")
WEIGHT_PINS = os.path.join(HERE, "PINS.sha256")
TAG = re.compile(r"\[[A-Za-z][A-Za-z0-9_ ]{0,32}\]")
REF_KEYS = ("reference", "audio_prompt", "audio_prompt_path", "clip")
NANO_SR = 24_000
NANO_RELS = (
    "shared/s3gen.safetensors",
    "shared/s3gen_meanflow.safetensors",
    "shared/ve.safetensors",
    "nano/t3_nano_v1.safetensors",
)
TOKENIZER_NAMES = (
    "vocab.json",
    "merges.txt",
    "added_tokens.json",
    "special_tokens_map.json",
    "tokenizer_config.json",
)
_HELD_STDOUT = None


def weights_root() -> str:
    root = os.environ.get("GALION_CLONE_WEIGHTS", "").strip()
    return root or os.path.join(REPO, "weights", "clone")


def weights_present() -> bool:
    root = weights_root()
    return all(os.path.isfile(os.path.join(root, name)) for name in NANO_RELS)


def normalize_persona(value: object) -> str:
    name = str(value or "samantha").strip().lower()
    return "jarvis" if name == "jarvis" else "samantha"


def refuse(why: str) -> None:
    sys.stderr.write(f"galion-clone-refuse: {why}\n")
    sys.stderr.flush()
    raise SystemExit(3)


def absent(why: str) -> None:
    sys.stderr.write(f"galion-clone-absent: {why}\n")
    sys.stderr.flush()
    raise SystemExit(2)


def one_line(text: object) -> str:
    return " ".join(str(text).split())[:300]


def arm_pcm_channel() -> int:
    """Keep a private PCM fd. Library prints follow stderr.

    The held stdout object is not closed: its buffer still owns fd 1, and
    closing it would close stderr after the dup2.
    """
    global _HELD_STDOUT
    pcm_fd = os.dup(1)
    os.dup2(2, 1)
    try:
        sys.stdout.flush()
    except Exception:
        pass
    _HELD_STDOUT = sys.stdout
    sys.stdout = sys.stderr
    sys.__stdout__ = sys.stderr
    return pcm_fd


def write_all(fd: int, data: bytes) -> None:
    view = memoryview(data)
    while len(view):
        try:
            n = os.write(fd, view)
        except OSError as exc:
            if exc.errno == 4:  # EINTR
                continue
            sys.stderr.write(f"galion-clone-refuse: pcm write failed ({exc.errno})\n")
            sys.stderr.flush()
            raise SystemExit(1) from exc
        if n <= 0:
            sys.stderr.write("galion-clone-refuse: pcm write failed\n")
            sys.stderr.flush()
            raise SystemExit(1)
        view = view[n:]


def frame_utterance(pcm_fd: int, pcm: bytes, engine: str, persona: str) -> None:
    """Marker first, then PCM. Stderr is flushed before the first PCM byte."""
    sys.stderr.write(f"galion-clone-engine: {engine}\n")
    sys.stderr.write(f"galion-clone-persona: {persona}\n")
    sys.stderr.write(f"galion-clone-utt: {len(pcm)}\n")
    sys.stderr.flush()
    if pcm:
        write_all(pcm_fd, pcm)


def load_allowlist(path: str = ALLOWLIST_PATH) -> dict[str, tuple[str, str]]:
    allow: dict[str, tuple[str, str]] = {}
    if not os.path.isfile(path):
        return allow
    with open(path, "r", encoding="utf-8") as handle:
        for raw in handle:
            line = raw.split("#", 1)[0].strip()
            if not line:
                continue
            parts = line.split()
            if len(parts) != 3:
                continue
            persona, prefix, suffix = (p.lower() for p in parts)
            if persona not in ("samantha", "jarvis"):
                continue
            blob = prefix + suffix
            if not blob or any(c not in "0123456789abcdef" for c in blob):
                continue
            allow[persona] = (prefix, suffix)
    return allow


def digest_allowed(persona: str, digest: str, allow: dict[str, tuple[str, str]]) -> bool:
    spec = allow.get(persona)
    if spec is None:
        return False
    prefix, suffix = spec
    folded = digest.strip().lower()
    if len(folded) != 64 or any(c not in "0123456789abcdef" for c in folded):
        return False
    return folded.startswith(prefix) and folded.endswith(suffix)


def sha256_file(path: str) -> str:
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def sidecar_digest(path: str) -> str | None:
    side = path + ".sha256"
    if not os.path.isfile(side):
        return None
    text = open(side, "r", encoding="utf-8", errors="replace").read().strip().split()
    if not text:
        refuse("reference sidecar is empty")
    folded = text[0].strip().lower()
    if len(folded) != 64 or any(c not in "0123456789abcdef" for c in folded):
        refuse("reference sidecar is not a sha256")
    return folded


def default_ref(persona: str) -> str | None:
    """Clip store wins when it is set. An empty store does not fall through."""
    store = os.environ.get("GALION_CLIP_STORE", "").strip()
    if store:
        path = os.path.join(store, persona, "reference.wav")
        return path if os.path.isfile(path) else None
    repo_ref = os.path.join(
        REPO, "docs", "galion-voice-v2", "personas", "refs", f"{persona}.wav"
    )
    return repo_ref if os.path.isfile(repo_ref) else None


def nonempty_refs(msg: dict) -> list[tuple[str, object]]:
    found = []
    for key in REF_KEYS:
        value = msg.get(key)
        if value:
            found.append((key, value))
    return found


def resolve_allowlisted(
    persona: str, refs: list[tuple[str, object]], allow: dict[str, tuple[str, str]]
) -> str:
    paths: list[str] = []
    for _key, value in refs:
        if not isinstance(value, str):
            refuse("reference clips are on the rights hold")
        stripped = value.strip()
        if stripped:
            paths.append(stripped)
    unique = list(dict.fromkeys(paths))
    if len(unique) > 1:
        refuse("conflicting reference paths")
    path = unique[0] if unique else default_ref(persona)
    if not path or not os.path.isfile(path):
        refuse(f"local reference missing for {persona}")
    digest = sha256_file(path)
    side = sidecar_digest(path)
    if side is not None and side != digest:
        refuse(f"reference sidecar does not match {persona}")
    if not digest_allowed(persona, digest, allow):
        refuse(f"reference sha256 is not allowlisted for {persona}")
    return path


def prepare_text(text: str, msg: dict) -> str:
    """Honor an explicit tags flag. A missing flag keeps bracket tags.

    The 2026-11-03 smoke left tags in the text it sent to generate.
    The Rust request line sets tags false for Jarvis, and that still strips.
    """
    flag = msg.get("tags", None)
    if flag is not None and not flag:
        text = TAG.sub("", text)
    return " ".join(text.split())


def load_table(path: str) -> list[tuple[str, str, int]]:
    rows: list[tuple[str, str, int]] = []
    if not os.path.isfile(path):
        return rows
    with open(path, "r", encoding="utf-8") as handle:
        for raw in handle:
            line = raw.split("#", 1)[0].strip()
            if not line:
                continue
            parts = line.split()
            if len(parts) < 3:
                continue
            try:
                size = int(parts[2])
            except ValueError:
                continue
            rows.append((parts[0], parts[1].lower(), size))
    return rows


def file_matches(path: str, sha: str, size: int) -> bool:
    if not os.path.isfile(path):
        return False
    if os.path.getsize(path) != size:
        return False
    return sha256_file(path) == sha


def tensors_pinned(root: str) -> bool:
    rows = {rel: (sha, size) for rel, sha, size in load_table(WEIGHT_PINS)}
    for rel in NANO_RELS:
        pin = rows.get(rel)
        if pin is None or not file_matches(os.path.join(root, rel), pin[0], pin[1]):
            return False
    return True


def tokenizer_ready(root: str) -> bool:
    rows = load_table(TOKENIZER_PINS)
    if len(rows) < len(TOKENIZER_NAMES):
        return False
    seen = set()
    for rel, sha, size in rows:
        if os.path.basename(rel) not in TOKENIZER_NAMES:
            continue
        if not file_matches(os.path.join(root, rel), sha, size):
            return False
        seen.add(os.path.basename(rel))
    return set(TOKENIZER_NAMES) <= seen


def _link(src: str, dest: str) -> None:
    if os.path.lexists(dest):
        return
    os.symlink(os.path.abspath(src), dest)


def build_ckpt(root: str) -> str:
    explicit = os.environ.get("GALION_CLONE_CKPT", "").strip()
    if explicit and _flat_ckpt_ok(explicit):
        return explicit
    import tempfile

    ckpt = tempfile.mkdtemp(prefix="galion-clone-ckpt-")

    def _cleanup() -> None:
        import shutil

        shutil.rmtree(ckpt, ignore_errors=True)

    import atexit

    atexit.register(_cleanup)
    mapping = {
        "ve.safetensors": os.path.join(root, "shared", "ve.safetensors"),
        "s3gen.safetensors": os.path.join(root, "shared", "s3gen.safetensors"),
        "s3gen_meanflow.safetensors": os.path.join(
            root, "shared", "s3gen_meanflow.safetensors"
        ),
        "t3_nano_v1.safetensors": os.path.join(root, "nano", "t3_nano_v1.safetensors"),
    }
    for name, src in mapping.items():
        if not os.path.isfile(src):
            absent(f"missing {name}")
        _link(src, os.path.join(ckpt, name))
    for name in TOKENIZER_NAMES:
        src = os.path.join(root, "tokenizer", name)
        if not os.path.isfile(src):
            absent(f"missing tokenizer {name}")
        _link(src, os.path.join(ckpt, name))
    return ckpt


def _flat_ckpt_ok(ckpt: str) -> bool:
    """A pre-built directory is used only when its bytes match the pins."""
    weight_rows = {os.path.basename(rel): (sha, size) for rel, sha, size in load_table(WEIGHT_PINS)}
    tok_rows = {
        os.path.basename(rel): (sha, size) for rel, sha, size in load_table(TOKENIZER_PINS)
    }
    needed = ("ve.safetensors", "s3gen_meanflow.safetensors", "t3_nano_v1.safetensors")
    for name in needed:
        pin = weight_rows.get(name)
        if pin is None or not file_matches(os.path.join(ckpt, name), pin[0], pin[1]):
            return False
    for name in TOKENIZER_NAMES:
        pin = tok_rows.get(name)
        if pin is None or not file_matches(os.path.join(ckpt, name), pin[0], pin[1]):
            return False
    return True


def load_nano(ckpt: str):
    os.environ.setdefault("HF_HUB_DISABLE_PROGRESS_BARS", "1")
    os.environ.setdefault("TOKENIZERS_PARALLELISM", "false")
    os.environ.setdefault("TQDM_DISABLE", "1")
    sys.stderr.write("galion-clone-load: chatterbox-nano\n")
    sys.stderr.flush()
    try:
        import inspect

        from chatterbox.tts_turbo import ChatterboxTurboTTS
    except Exception as exc:
        absent(f"chatterbox runtime missing ({type(exc).__name__}: {one_line(exc)})")
    try:
        params = inspect.signature(ChatterboxTurboTTS.from_local).parameters
    except (TypeError, ValueError):
        params = {}
    if "nano" not in params:
        absent(
            "chatterbox runtime has no nano=True "
            "(need resemble-ai/chatterbox 5de7a54; PyPI chatterbox-tts 0.1.7 does not)"
        )
    try:
        import torch

        torch.set_grad_enabled(False)
        model = ChatterboxTurboTTS.from_local(ckpt, "cpu", nano=True)
    except SystemExit:
        raise
    except Exception as exc:
        absent(f"failed to load chatterbox nano ({type(exc).__name__}: {one_line(exc)})")
    sr = int(getattr(model, "sr", 0) or 0)
    if sr != NANO_SR:
        absent(f"chatterbox nano sample rate {sr} is not {NANO_SR}")
    sys.stderr.write("galion-clone-ready: chatterbox-nano\n")
    sys.stderr.flush()
    return model


def wav_to_s16le(wav) -> bytes:
    import torch

    audio = wav.detach().to(dtype=torch.float32, device="cpu").reshape(-1).contiguous()
    if audio.numel() == 0:
        refuse("chatterbox nano returned no samples")
    audio = audio.clamp(-1.0, 1.0).mul(32767.0).round()
    raw = audio.to(torch.int16).numpy()
    if sys.byteorder != "little":
        raw = raw.byteswap()
    return raw.tobytes()


def render(model, text: str, path: str, cache: dict) -> bytes:
    sys.stderr.write("galion-clone-gen: start\n")
    sys.stderr.flush()
    # Reuse conditionals when the allowlisted path is unchanged. The first
    # call passes the path through with no denoise, trim, or high-pass.
    kwargs = {}
    if cache.get("path") != path:
        kwargs["audio_prompt_path"] = path
    try:
        wav = model.generate(text, **kwargs)
    except Exception as exc:
        refuse(f"chatterbox nano generate failed ({type(exc).__name__}: {one_line(exc)})")
    cache["path"] = path
    pcm = wav_to_s16le(wav)
    sys.stderr.write(f"galion-clone-gen: bytes {len(pcm)}\n")
    sys.stderr.flush()
    return pcm


def read_request() -> dict | None:
    line = sys.stdin.buffer.readline()
    if not line:
        return None
    try:
        msg = json.loads(line.decode())
    except json.JSONDecodeError:
        refuse("bad json")
    if not isinstance(msg, dict):
        refuse("bad json")
    return msg


def run_stub(pcm_fd: int) -> None:
    while True:
        msg = read_request()
        if msg is None:
            return
        for key in REF_KEYS:
            if msg.get(key):
                refuse("reference clips are on the rights hold")
        text = str(msg.get("text", ""))
        persona = normalize_persona(msg.get("persona", "samantha"))
        if persona == "jarvis":
            text = TAG.sub("", text)
        text = " ".join(text.split())
        n = max(1, min(len(text), 32767))
        tone = 1000 + (n % 50)
        pcm = struct.pack("<h", n) + struct.pack("<200h", *([tone] * 200))
        frame_utterance(pcm_fd, pcm, "protocol-stub", persona)


def run_unwired(pcm_fd: int) -> None:
    while True:
        msg = read_request()
        if msg is None:
            return
        if nonempty_refs(msg):
            refuse("reference clips are on the rights hold")
        persona = normalize_persona(msg.get("persona", "samantha"))
        sys.stderr.write("galion-clone-unwired\n")
        frame_utterance(pcm_fd, b"", "unwired", persona)


def run_nano(pcm_fd: int, allow: dict[str, tuple[str, str]]) -> None:
    root = weights_root()
    model = None
    cache: dict = {}
    while True:
        msg = read_request()
        if msg is None:
            return
        persona = normalize_persona(msg.get("persona", "samantha"))
        model_name = str(
            msg.get("model") or os.environ.get("GALION_CLONE_MODEL") or "nano"
        ).strip().lower()
        if model_name != "nano":
            sys.stderr.write("galion-clone-unwired\n")
            sys.stderr.write("galion-clone-note: generate path is chatterbox nano\n")
            frame_utterance(pcm_fd, b"", "unwired", persona)
            continue
        path = resolve_allowlisted(persona, nonempty_refs(msg), allow)
        text = prepare_text(str(msg.get("text", "")), msg)
        if not text:
            refuse("empty text")
        if model is None:
            if not tensors_pinned(root):
                absent("pinned safetensors mismatch")
            if not tokenizer_ready(root):
                absent("tokenizer files missing or not pinned")
            model = load_nano(build_ckpt(root))
        pcm = render(model, text, path, cache)
        frame_utterance(pcm_fd, pcm, "chatterbox-nano", persona)


def main() -> None:
    pcm_fd = arm_pcm_channel()
    if os.environ.get("GALION_CLONE_STUB_PCM") == "1":
        run_stub(pcm_fd)
        return
    if not weights_present():
        absent("pinned safetensors missing")
    if os.environ.get("GALION_CLONE_LOCAL_REFS") != "1":
        run_unwired(pcm_fd)
        return
    run_nano(pcm_fd, load_allowlist())


if __name__ == "__main__":
    main()
