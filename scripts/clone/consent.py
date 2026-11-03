#!/usr/bin/env python3
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Consent gate for one Samantha or Jarvis reference clip.

Stdlib only. This does not fetch audio, does not write a wav, does not invent
a counsel name or a sha256, and does not edit LOCKS.toml.

A clip is ready only when every rights-checklist field is filled, the wav is
mono 16-bit PCM of about ten seconds (8–12 s), the sha256 matches, the store
is outside the git tree, the pinned weight sizes are on disk, and
reference_clips is true in the Chatterbox lock. Any miss is a named reason
and clone_live stays false.
"""

from __future__ import annotations

import hashlib
import os
import sys
import wave

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))

REQUIRED = (
    "date",
    "persona",
    "work",
    "performer",
    "grantor",
    "instrument",
    "grantee",
    "scope",
    "public_demo",
    "expiry",
    "operator",
    "withdrawal",
    "counsel",
    "sha256",
    "duration_s",
)

SCOPE = "local-zero-shot-conditioning"
DURATION_MIN = 8.0
DURATION_MAX = 12.0
DURATION_TOLERANCE = 0.25
PLACEHOLDERS = {
    "todo",
    "field",
    "placeholder",
    "unknown",
    "tbd",
    "n/a",
    "na",
    "none",
    "null",
    "fixme",
}
TOKENIZER = (
    "tokenizer/added_tokens.json",
    "tokenizer/merges.txt",
    "tokenizer/special_tokens_map.json",
    "tokenizer/tokenizer_config.json",
    "tokenizer/vocab.json",
)
PERSONAS = ("samantha", "jarvis")


class Gate:
    def __init__(self, ok: bool, reason: str, seconds: float | None = None, public: bool = False, wav: str | None = None):
        self.ok = ok
        self.reason = reason
        self.seconds = seconds
        self.public = public
        self.wav = wav

    @staticmethod
    def absent(reason: str) -> "Gate":
        return Gate(False, reason)


def persona_of(name: str) -> str | None:
    key = (name or "").strip().lower()
    if key in PERSONAS:
        return key
    return None


def locks_path() -> str:
    override = os.environ.get("GALION_LOCKS_FILE", "").strip()
    if override:
        return override
    return os.path.join(REPO, "LOCKS.toml")


def reference_clips_allowed(text: str) -> bool:
    """True only inside the Chatterbox exception table. Missing means false."""
    in_table = False
    for raw in text.splitlines():
        line = raw.strip()
        if line.startswith("[") and line.endswith("]"):
            in_table = line == "[formats.exceptions.v2s14_chatterbox]"
            continue
        if not in_table or not line.startswith("reference_clips"):
            continue
        value = line.split("=", 1)[1].split("#", 1)[0].strip().lower()
        return value == "true"
    return False


def store_root() -> str:
    return os.environ.get("GALION_CLIP_STORE", "").strip()


def weights_root() -> str:
    env = os.environ.get("GALION_CLONE_WEIGHTS", "").strip()
    if env:
        return env
    return os.path.join(REPO, "weights", "clone")


def model_name() -> str:
    name = os.environ.get("GALION_CLONE_MODEL", "nano").strip().lower()
    return name if name in ("nano", "turbo") else "nano"


def inside_tree(path: str, root: str) -> bool:
    path = os.path.realpath(path)
    root = os.path.realpath(root)
    return path == root or path.startswith(root + os.sep)


def parse_consent(text: str) -> dict[str, str]:
    out: dict[str, str] = {}
    for raw in text.splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        if ":" in line:
            key, value = line.split(":", 1)
        elif "=" in line:
            key, value = line.split("=", 1)
        else:
            continue
        key = key.strip().lower().replace(" ", "_")
        out[key] = value.strip()
    return out


def value_rejected(value: str) -> bool:
    if not value or len(value) > 400:
        return True
    if "[" in value or "]" in value:
        return True
    return value.strip().lower() in PLACEHOLDERS


def wav_info(path: str) -> tuple[float, int, int]:
    with wave.open(path, "rb") as handle:
        channels = handle.getnchannels()
        width = handle.getsampwidth()
        rate = handle.getframerate()
        frames = handle.getnframes()
    if rate <= 0:
        raise ValueError("rate")
    return frames / float(rate), channels, width


def sha256_file(path: str) -> str:
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def read_sidecar(path: str) -> str:
    try:
        text = open(path, "r", encoding="utf-8").read().strip().split()
    except OSError:
        return ""
    if not text:
        return ""
    return text[0].strip().lower()


def pins() -> dict[str, int]:
    table: dict[str, int] = {}
    path = os.path.join(HERE, "PINS.sha256")
    with open(path, "r", encoding="utf-8") as handle:
        for raw in handle:
            line = raw.strip()
            if not line or line.startswith("#"):
                continue
            parts = line.split()
            if len(parts) < 3:
                continue
            table[parts[0]] = int(parts[2])
    return table


def weight_files(model: str) -> tuple[str, ...]:
    t3 = f"{model}/t3_{model}_v1.safetensors"
    return (
        "shared/s3gen.safetensors",
        "shared/s3gen_meanflow.safetensors",
        "shared/ve.safetensors",
        t3,
        *TOKENIZER,
    )


def weights_sized(model: str) -> bool:
    """Size match against the pin table. The speech slot still hashes before it speaks."""
    table = pins()
    root = weights_root()
    for rel in weight_files(model):
        size = table.get(rel)
        path = os.path.join(root, rel)
        try:
            got = os.path.getsize(path)
        except OSError:
            return False
        if size is None or got != size:
            return False
    return True


def evaluate(persona: str, *, need_weights: bool = True) -> Gate:
    name = persona_of(persona)
    if name is None:
        return Gate.absent("persona")
    store = store_root()
    if not store:
        return Gate.absent("store")
    if inside_tree(store, REPO):
        return Gate.absent("inside_repo")
    folder = os.path.join(store, name)
    wav = os.path.join(folder, "reference.wav")
    if not os.path.isfile(wav) or inside_tree(wav, REPO):
        return Gate.absent("wav")
    try:
        seconds, channels, width = wav_info(wav)
    except (wave.Error, EOFError, ValueError, OSError):
        return Gate.absent("wav")
    if channels != 1 or width != 2:
        return Gate.absent("mono")
    if seconds < DURATION_MIN or seconds > DURATION_MAX:
        return Gate.absent("duration")
    consent_path = os.path.join(folder, "consent.txt")
    try:
        fields = parse_consent(open(consent_path, "r", encoding="utf-8").read())
    except OSError:
        return Gate.absent("consent")
    for key in REQUIRED:
        if value_rejected(fields.get(key, "")):
            return Gate.absent("consent")
    if fields["persona"].strip().lower() != name:
        return Gate.absent("consent")
    if fields["scope"] != SCOPE:
        return Gate.absent("consent")
    public = fields["public_demo"].strip().lower()
    if public not in ("yes", "no"):
        return Gate.absent("consent")
    if "CET" not in fields["date"] and "Zagreb" not in fields["date"]:
        return Gate.absent("consent")
    try:
        declared = float(fields["duration_s"])
    except ValueError:
        return Gate.absent("consent")
    if abs(declared - seconds) > DURATION_TOLERANCE:
        return Gate.absent("duration")
    digest = fields["sha256"].strip().lower()
    if len(digest) != 64 or any(c not in "0123456789abcdef" for c in digest):
        return Gate.absent("sha")
    sidecar = read_sidecar(wav + ".sha256")
    if sidecar != digest or sha256_file(wav) != digest:
        return Gate.absent("sha")
    try:
        lock_text = open(locks_path(), "r", encoding="utf-8").read()
    except OSError:
        return Gate.absent("lock")
    if not reference_clips_allowed(lock_text):
        return Gate.absent("lock")
    worker = os.path.join(HERE, "chatterbox_worker.py")
    if not os.path.isfile(worker):
        return Gate.absent("worker")
    if need_weights and not weights_sized(model_name()):
        return Gate.absent("weights")
    return Gate(True, "ready", seconds=seconds, public=public == "yes", wav=os.path.realpath(wav))


def measure(persona: str) -> int:
    """Print duration and sha256 of a wav the human already placed. Writes nothing."""
    name = persona_of(persona)
    if name is None:
        sys.stderr.write("measure: persona must be samantha or jarvis\n")
        return 2
    store = store_root()
    if not store:
        sys.stderr.write("measure: GALION_CLIP_STORE is unset. No default path.\n")
        return 2
    if inside_tree(store, REPO):
        sys.stderr.write("measure: the store is inside the git tree. Move it outside.\n")
        return 2
    wav = os.path.join(store, name, "reference.wav")
    if not os.path.isfile(wav):
        sys.stderr.write("measure: reference.wav is not in the store. This script does not fetch one.\n")
        return 2
    try:
        seconds, channels, width = wav_info(wav)
    except (wave.Error, EOFError, ValueError, OSError):
        sys.stderr.write("measure: need a 16-bit PCM wav. This tree does not convert it.\n")
        return 1
    if channels != 1 or width != 2:
        sys.stderr.write("measure: need mono 16-bit PCM. This tree does not remix it.\n")
        return 1
    print(f"duration_s: {seconds:.3f}")
    print(f"sha256: {sha256_file(wav)}")
    if seconds < DURATION_MIN or seconds > DURATION_MAX:
        sys.stderr.write(
            "measure: duration is outside 8–12 seconds. About 10 seconds is the clip. This tree does not trim.\n"
        )
        return 1
    return 0


def main(argv: list[str]) -> int:
    if not argv or argv[0] in ("-h", "--help"):
        sys.stderr.write(
            "usage: consent.py fields | consent.py measure --persona samantha|jarvis | consent.py check --persona samantha|jarvis\n"
        )
        return 2
    cmd = argv[0]
    if cmd == "fields":
        for key in REQUIRED:
            print(key)
        return 0
    if cmd not in ("measure", "check"):
        sys.stderr.write("consent: unknown command\n")
        return 2
    persona = ""
    args = argv[1:]
    while args:
        item = args.pop(0)
        if item == "--persona" and args:
            persona = args.pop(0)
        else:
            sys.stderr.write("consent: unknown argument\n")
            return 2
    if cmd == "measure":
        return measure(persona)
    gate = evaluate(persona)
    name = persona_of(persona) or persona.strip().lower() or "persona"
    if gate.ok and gate.seconds is not None:
        flag = "yes" if gate.public else "no"
        print(f"galion-clip-ready: {name} {gate.seconds:.3f} public={flag}")
        return 0
    print(f"galion-clip-absent: {name} {gate.reason}")
    return 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
