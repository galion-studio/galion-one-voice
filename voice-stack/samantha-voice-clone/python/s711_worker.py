#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
"""Samantha s711 voice worker.

One JSON line on stdin. Raw 24 kHz mono s16le on a private copy of stdout.
Markers on stderr. The process imports ``scripts/clone/chatterbox_worker.py``
for framing, sha256, and PCM packing. It does not download weights, does not
write a wav, and does not chain a sentence onto the previous sentence.

Recipe, fixed:
  mode expr, exaggeration 0.5, cfg 0.5, seed 711+index,
  every sentence conditioned on the original reference,
  24 kHz 16-bit mono, raw (no mastering, slowdown, or effect pass).

The reference path is the one in the request. Its sha256 must start with
10ac44dc. Consent is local zero-shot conditioning only.

``GALION_SAMANTHA_STUB=1`` emits protocol PCM and does not import torch.
A real utterance needs ``GALION_SAMANTHA_WEIGHTS`` pointing at a local
Chatterbox expressive checkpoint. This file never calls a download loader.
CUDA is used only when torch reports it and the device request allows it.
CPU is the path when CUDA is absent. Neither path is timed as 80 ms here.
"""

from __future__ import annotations

import hashlib
import os
import struct
import sys
import time

MODE = "expr"
EXAGGERATION = 0.5
CFG = 0.5
SEED_BASE = 711
REF_PREFIX = "10ac44dc"
RATE = 24_000
FRAME = 1_920  # 80 ms at 24 kHz
CONSENT = "local-zero-shot-conditioning"
_REPO = None


def one_line(text: object) -> str:
    return " ".join(str(text).split())[:300]


def refuse(why: str) -> None:
    sys.stderr.write(f"galion-s711-refuse: {one_line(why)}\n")
    sys.stderr.flush()
    raise SystemExit(3)


def absent(why: str) -> None:
    sys.stderr.write(f"galion-s711-absent: {one_line(why)}\n")
    sys.stderr.flush()
    raise SystemExit(2)


def repo_root() -> str:
    override = os.environ.get("GALION_REPO", "").strip()
    if override:
        return os.path.abspath(override)
    here = os.path.dirname(os.path.abspath(__file__))
    return os.path.abspath(os.path.join(here, "..", "..", ".."))


def repo():
    """The repo Chatterbox worker module. Framing and sha256 live there."""
    global _REPO
    if _REPO is not None:
        return _REPO
    clone = os.path.join(repo_root(), "scripts", "clone")
    if clone not in sys.path:
        sys.path.insert(0, clone)
    import chatterbox_worker as cw

    _REPO = cw
    return cw


def prefix_ok(digest: str) -> bool:
    folded = digest.strip().lower()
    if len(folded) != 64:
        return False
    if any(c not in "0123456789abcdef" for c in folded):
        return False
    return folded.startswith(REF_PREFIX)


def seed_for(index: int) -> int:
    if index < 0:
        refuse("sentence index is negative")
    return SEED_BASE + index


def recipe_ok(msg: dict) -> tuple[int, int]:
    """Accept only the s711 line. Mood is ignored for the knobs."""
    if not isinstance(msg, dict):
        refuse("request must be a json object")
    if str(msg.get("mode", "")).strip().lower() != MODE:
        refuse("s711 mode is expr")
    try:
        exaggeration = float(msg.get("exaggeration"))
        cfg = float(msg.get("cfg_weight", msg.get("cfg")))
        seed = int(msg["seed"])
        index = int(msg["index"])
    except (KeyError, TypeError, ValueError):
        refuse("s711 recipe fields missing")
    if abs(exaggeration - EXAGGERATION) > 1e-6:
        refuse("s711 exaggeration is 0.5")
    if abs(cfg - CFG) > 1e-6:
        refuse("s711 cfg is 0.5")
    if msg.get("raw") is not True:
        refuse("s711 output is raw")
    if msg.get("effects") is True or msg.get("mastering") is True or msg.get("slowdown") is True:
        refuse("s711 output is raw")
    if msg.get("denoise") is True:
        refuse("denoise refused")
    if str(msg.get("consent", "")) != CONSENT:
        refuse("consent scope is local zero-shot conditioning only")
    for key in ("train", "finetune", "fine_tune", "upload", "remote"):
        if msg.get(key):
            refuse(f"{key} is outside the local zero-shot consent")
    if msg.get("chain") is True or msg.get("previous_audio") or msg.get("prompt_from_previous"):
        refuse("s711 conditions every sentence on the original reference")
    if seed != seed_for(index):
        refuse("seed is not 711+index")
    return seed, index


def sha256_file(path: str) -> str:
    try:
        return repo().sha256_file(path)
    except SystemExit:
        raise
    except Exception:
        digest = hashlib.sha256()
        with open(path, "rb") as handle:
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                digest.update(chunk)
        return digest.hexdigest()


def reference_path(msg: dict) -> str:
    paths = []
    for key in ("audio_prompt_path", "reference"):
        value = msg.get(key)
        if value:
            if not isinstance(value, str):
                refuse("reference must be a wav path")
            paths.append(value)
    if not paths:
        refuse("reference path missing")
    if any(item != paths[0] for item in paths):
        refuse("reference fields disagree")
    path = paths[0]
    base = os.path.basename(path).lower()
    if not base.endswith(".wav"):
        refuse("reference must be a wav path")
    if base.endswith((".mp3", ".flac", ".onnx", ".pcm", ".s16le")):
        refuse("reference must be a wav path")
    return path


def check_ref(msg: dict) -> str:
    path = reference_path(msg)
    if not os.path.isfile(path):
        refuse("reference file missing")
    digest = sha256_file(path)
    if not prefix_ok(digest):
        refuse(f"reference sha256 prefix is not {REF_PREFIX}")
    claimed = str(msg.get("ref_sha256") or "").strip().lower()
    if claimed and claimed != digest:
        refuse("ref_sha256 does not match the file")
    return digest


def stub_pcm(text: str, seed: int) -> bytes:
    """Protocol PCM. Not a voice and not a model sample."""
    tone = (int(seed) + len(text)) % 32767
    if tone <= 0:
        tone = 1
    return struct.pack("<h", tone) * FRAME


def resample_s16(pcm: bytes, src_rate: int, dst_rate: int = RATE) -> bytes:
    """Match 24 kHz. This is a rate change, not a slowdown of a performance."""
    if src_rate == dst_rate:
        return pcm
    if src_rate <= 0 or dst_rate <= 0 or len(pcm) < 2 or len(pcm) % 2:
        refuse("bad pcm for resample")
    samples = list(struct.unpack("<" + "h" * (len(pcm) // 2), pcm))
    out_n = max(1, int(round(len(samples) * dst_rate / src_rate)))
    last = len(samples) - 1
    out = []
    for i in range(out_n):
        pos = i * (src_rate / dst_rate)
        i0 = int(pos)
        if i0 >= last:
            out.append(samples[last])
            continue
        frac = pos - i0
        mixed = samples[i0] + (samples[i0 + 1] - samples[i0]) * frac
        mixed = max(-32767, min(32767, int(round(mixed))))
        out.append(mixed)
    return struct.pack("<" + "h" * len(out), *out)


def emit(pcm_fd: int, pcm: bytes, seed: int, index: int, digest: str, dt_us: int, device: str) -> None:
    if len(pcm) % 2:
        refuse("pcm is not 16-bit")
    err = sys.stderr
    err.write(
        "galion-s711-recipe: mode=expr exaggeration=0.5000 cfg=0.5000 "
        "seed=%d index=%d chain=false ref_prefix=%s\n" % (seed, index, digest[:8])
    )
    err.write(f"galion-s711-device: {device}\n")
    err.write("galion-s711-format: rate=24000 channels=1 bits=16 encoding=s16le raw=true\n")
    err.write(f"galion-s711-ttfa-us: {dt_us}\n")
    err.write(f"galion-s711-utt: {len(pcm)}\n")
    err.flush()
    if pcm:
        repo().write_all(pcm_fd, pcm)


def read_request() -> dict | None:
    return repo().read_request()


def run_stub(pcm_fd: int) -> None:
    while True:
        msg = read_request()
        if msg is None:
            return
        started = time.perf_counter()
        seed, index = recipe_ok(msg)
        digest = check_ref(msg)
        text = " ".join(str(msg.get("text", "")).split())
        if not text:
            refuse("empty text")
        pcm = stub_pcm(text, seed)
        dt_us = int((time.perf_counter() - started) * 1_000_000)
        emit(pcm_fd, pcm, seed, index, digest, dt_us, "stub")


def apply_thread_cap() -> str:
    raw = os.environ.get("GALION_SAMANTHA_THREADS", "").strip()
    if not raw.isdigit() or int(raw) < 1:
        return "cpu-default"
    for key in (
        "OMP_NUM_THREADS",
        "OPENBLAS_NUM_THREADS",
        "MKL_NUM_THREADS",
        "NUMEXPR_NUM_THREADS",
        "VECLIB_MAXIMUM_THREADS",
        "TORCH_NUM_THREADS",
    ):
        os.environ[key] = raw
    return f"cpu-threads={raw}"


def resolve_device(asked: str) -> str:
    asked = (asked or "auto").strip().lower()
    if asked in ("", "auto"):
        try:
            import torch
        except Exception:
            return "cpu"
        return "cuda" if torch.cuda.is_available() else "cpu"
    if asked == "cpu":
        return "cpu"
    if asked == "cuda":
        try:
            import torch
        except Exception as exc:
            refuse(f"cuda requested but torch is not importable ({type(exc).__name__})")
        if not torch.cuda.is_available():
            refuse("cuda requested but torch.cuda.is_available() is false")
        return "cuda"
    refuse(f"unknown device {asked}")


def load_expr(device: str):
    weights = os.environ.get("GALION_SAMANTHA_WEIGHTS", "").strip()
    if not weights:
        absent("GALION_SAMANTHA_WEIGHTS is unset; not downloading a model")
    if not os.path.isdir(weights):
        absent("GALION_SAMANTHA_WEIGHTS is not a directory")
    import torch

    torch.set_grad_enabled(False)
    try:
        from chatterbox.tts import ChatterboxTTS
    except Exception as exc:
        absent(f"chatterbox.tts is not importable ({type(exc).__name__}: {one_line(exc)})")
    if not hasattr(ChatterboxTTS, "from_local"):
        absent("ChatterboxTTS.from_local is missing; not calling a download loader")
    try:
        model = ChatterboxTTS.from_local(weights, device)
    except TypeError:
        model = ChatterboxTTS.from_local(weights, device=device)
    except Exception as exc:
        absent(f"failed to load local expressive checkpoint ({type(exc).__name__}: {one_line(exc)})")
    sys.stderr.write(f"galion-s711-ready: expr {device} {RATE}\n")
    sys.stderr.flush()
    return model


def apply_seed(seed: int, device: str) -> None:
    import torch

    torch.manual_seed(int(seed))
    if device == "cuda":
        torch.cuda.manual_seed_all(int(seed))


def generate_expr(model, text: str, ref: str, seed: int, device: str):
    """Condition this sentence on ``ref``. ``ref`` is the original clip."""
    import inspect

    apply_seed(seed, device)
    try:
        params = inspect.signature(model.generate).parameters
    except (TypeError, ValueError):
        params = {}
    if "exaggeration" not in params or ("cfg_weight" not in params and "cfg" not in params):
        refuse("this Chatterbox generate() does not accept the s711 exaggeration and cfg knobs")
    if "audio_prompt_path" in params:
        prompt_key = "audio_prompt_path"
    elif "audio_prompt" in params:
        prompt_key = "audio_prompt"
    else:
        refuse("generate() has no reference argument; refusing an unconditioned voice")
    kwargs = {
        prompt_key: ref,
        "exaggeration": EXAGGERATION,
    }
    if "cfg_weight" in params:
        kwargs["cfg_weight"] = CFG
    else:
        kwargs["cfg"] = CFG
    if "seed" in params:
        kwargs["seed"] = int(seed)
    try:
        return model.generate(text, **kwargs)
    except Exception as exc:
        refuse(f"chatterbox generate failed ({type(exc).__name__}: {one_line(exc)})")


def tensor_to_s16(wav) -> bytes:
    import torch

    audio = wav.detach().to(dtype=torch.float32, device="cpu")
    if audio.ndim == 2 and audio.shape[0] in (1, 2):
        if audio.shape[0] == 2:
            sys.stderr.write("galion-s711-downmix: stereo-to-mono\n")
            audio = audio.mean(dim=0)
        else:
            audio = audio.reshape(-1)
    elif audio.ndim == 2 and audio.shape[-1] == 2:
        sys.stderr.write("galion-s711-downmix: stereo-to-mono\n")
        audio = audio.mean(dim=-1)
    else:
        audio = audio.reshape(-1)
    audio = audio.contiguous()
    if audio.numel() == 0:
        refuse("chatterbox returned no samples")
    audio = audio.clamp(-1.0, 1.0).mul(32767.0).round()
    raw = audio.to(torch.int16).cpu().numpy()
    if sys.byteorder != "little":
        raw = raw.byteswap()
    return raw.tobytes()


def run_real(pcm_fd: int) -> None:
    asked = os.environ.get("GALION_SAMANTHA_DEVICE", "auto").strip().lower() or "auto"
    thread_note = apply_thread_cap() if asked == "cpu" else "unset"
    device = resolve_device(asked)
    sys.stderr.write(f"galion-s711-cpu: device={device} threads={thread_note}\n")
    sys.stderr.flush()
    model = None
    while True:
        msg = read_request()
        if msg is None:
            return
        started = time.perf_counter()
        seed, index = recipe_ok(msg)
        path = reference_path(msg)
        digest = check_ref(msg)
        text = " ".join(str(msg.get("text", "")).split())
        if not text:
            refuse("empty text")
        if model is None:
            model = load_expr(device)
        wav = generate_expr(model, text, path, seed, device)
        pcm = tensor_to_s16(wav)
        sr = int(getattr(model, "sr", 0) or 0)
        if sr and sr != RATE:
            sys.stderr.write(f"galion-s711-resample: from={sr} to={RATE}\n")
            sys.stderr.flush()
            pcm = resample_s16(pcm, sr, RATE)
        dt_us = int((time.perf_counter() - started) * 1_000_000)
        emit(pcm_fd, pcm, seed, index, digest, dt_us, device)


def probe_torch() -> tuple[str, str]:
    try:
        import torch
    except Exception as exc:
        return "cpu", f"torch-missing:{type(exc).__name__}"
    if torch.cuda.is_available():
        return "cuda", "torch"
    return "cpu", "torch"


def self_check() -> int:
    """Recipe, prefix, chain refusal, repo import, stub pack. No wav. No torch required."""
    lines = []
    reuse = "missing"
    try:
        cw = repo()
        if not callable(getattr(cw, "sha256_file", None)):
            raise RuntimeError("sha256_file missing")
        if not callable(getattr(cw, "wav_to_s16le", None)):
            raise RuntimeError("wav_to_s16le missing")
        if not callable(getattr(cw, "write_all", None)):
            raise RuntimeError("write_all missing")
        reuse = "chatterbox_worker"
    except Exception as exc:
        lines.append(f"reuse_error={type(exc).__name__}")

    good = REF_PREFIX + ("ab" * 28)
    bad = "e4bc60c3" + ("ab" * 28)
    ok = {
        "mode": MODE,
        "exaggeration": EXAGGERATION,
        "cfg_weight": CFG,
        "raw": True,
        "effects": False,
        "mastering": False,
        "slowdown": False,
        "denoise": False,
        "consent": CONSENT,
        "chain": False,
        "seed": SEED_BASE,
        "index": 0,
    }
    seed, index = recipe_ok(dict(ok))
    second = dict(ok)
    second["seed"] = seed_for(1)
    second["index"] = 1
    recipe_ok(second)

    def refused(msg: dict) -> bool:
        try:
            recipe_ok(msg)
        except SystemExit as exc:
            return exc.code == 3
        return False

    chained = dict(ok)
    chained["chain"] = True
    trained = dict(ok)
    trained["train"] = True
    wrong_seed = dict(ok)
    wrong_seed["seed"] = 700
    started = time.perf_counter()
    pcm = stub_pcm("Hello.", SEED_BASE)
    stub_ttfa = int((time.perf_counter() - started) * 1_000_000)
    resampled = resample_s16(struct.pack("<hhhh", 0, 1000, 2000, 3000), 48_000, RATE)
    device, note = probe_torch()
    lines.extend(
        [
            "galion-s711-self-check: ok" if reuse == "chatterbox_worker" else "galion-s711-self-check: fail",
            f"mode={MODE}",
            f"exaggeration={EXAGGERATION}",
            f"cfg={CFG}",
            f"seed0={seed}",
            f"seed1={seed_for(1)}",
            f"index0={index}",
            f"prefix_ok={str(prefix_ok(good) and prefix_ok(good.upper())).lower()}",
            f"prefix_bad={str(prefix_ok(bad) or prefix_ok(REF_PREFIX)).lower()}",
            f"chain={('refused' if refused(chained) else 'accepted')}",
            f"train={('refused' if refused(trained) else 'accepted')}",
            f"wrong_seed={('refused' if refused(wrong_seed) else 'accepted')}",
            f"reuse={reuse}",
            f"stub_samples={len(pcm) // 2}",
            f"stub_ttfa_us={stub_ttfa}",
            f"resample_out={len(resampled) // 2}",
            "format=24000,1,16,s16le,raw",
            f"consent={CONSENT}",
            f"device_probe={device}",
            f"device_note={note}",
        ]
    )
    sys.stdout.write("\n".join(lines) + "\n")
    sys.stdout.flush()
    if reuse != "chatterbox_worker":
        return 1
    if not refused(chained) or not refused(trained) or not refused(wrong_seed):
        return 1
    if len(pcm) != FRAME * 2 or len(resampled) // 2 != 2:
        return 1
    if not prefix_ok(good) or prefix_ok(bad):
        return 1
    return 0


def main() -> None:
    if "--self-check" in sys.argv[1:]:
        raise SystemExit(self_check())
    try:
        cw = repo()
    except Exception as exc:
        absent(f"could not import chatterbox_worker ({type(exc).__name__}: {one_line(exc)})")
    pcm_fd = cw.arm_pcm_channel()
    if os.environ.get("GALION_SAMANTHA_STUB") == "1":
        run_stub(pcm_fd)
        return
    run_real(pcm_fd)


if __name__ == "__main__":
    main()
