#!/usr/bin/env python3
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Chunked PCM for Chatterbox Nano. This module does not call generate().

Verified against resemble-ai/chatterbox master on 2026-11-03 (Nano commit
5de7a54, 2026-07-21, and the master tree still current that day):

* ``ChatterboxTurboTTS.generate`` samples every speech token, then calls
  ``s3gen.inference(..., finalize=True)`` and returns. First audio is the
  finished utterance. That is the 1.8–2.4 s clock in the clone smoke.
* ``ChatterboxTurboTTS.stream`` exists only on unmerged PR 528. It is not
  on master and not in PyPI chatterbox-tts 0.1.7. If a local install has
  it, this module uses it.
* Master ``S3Token2Wav.forward`` already takes ``finalize=False`` (the last
  3 speech tokens are held back), ``skip_vocoder=True``, and ``noised_mels``.
  ``hift_inference`` accepts a source cache. ``T3.inference_turbo`` returns
  only after the last token, so the token loop below yields instead of
  calling that method.

Speech tokens are 25 Hz (``S3_TOKEN_RATE``). Lookahead is 3 tokens.
The first vocoder call waits for lookahead plus 2 tokens: 80 ms of stable
speech. That is the 80 ms heartbeat, not a measured model TTFA. A model
number is recorded only when a flagged run actually streams PCM.

``basic_euler`` prints ``S3 Token -> Mel Inference...`` on stdout. The
worker seals stdout onto stderr before this module loads the model.
Perth watermarking stays inside upstream ``generate()``. Streamed chunks
are not watermarked. This module does not vendor Perth and does not call
``generate()`` to obtain a watermark.
"""

from __future__ import annotations

import hashlib
import os
import struct

# 25 Hz speech tokens. Two stable tokens are one 80 ms heartbeat of audio.
TOKEN_RATE_HZ = 25
LOOKAHEAD_TOKENS = 3
CHUNK_TOKENS = 2
TOKEN_MEL_RATIO = 2
N_CFM_TIMESTEPS = 2
# Master const.py. Imported again from the package when the model is loaded.
SPEECH_VOCAB_SIZE = 6561
S3GEN_SIL = 4299
S3GEN_SR = 24_000


class NoStreamHook(RuntimeError):
    """The install can only speak by blocking in generate(). Refuse that."""


def flush_due(
    n_tokens: int,
    *,
    lookahead: int = LOOKAHEAD_TOKENS,
    chunk: int = CHUNK_TOKENS,
    final: bool = False,
) -> bool:
    """True when the stable prefix should be vocoded.

    Non-final flushes land on multiples of ``chunk`` stable tokens.
    The first one is ``lookahead + chunk`` tokens (5 at the defaults),
    which is 80 ms of speech plus the 3-token lookahead the flow drops.
    """
    if chunk <= 0 or n_tokens <= lookahead:
        return False
    stable = n_tokens - lookahead
    if final:
        return stable > 0
    return stable > 0 and stable % chunk == 0


def select_path(model: object) -> str:
    """``stream_api``, ``chunked_meanflow``, or ``refuse``.

    A model whose only speech method is ``generate`` is ``refuse``.
    The worker then writes no PCM and the host falls back to Piper.
    """
    stream = getattr(model, "stream", None)
    if callable(stream):
        return "stream_api"
    if getattr(model, "t3", None) is not None and getattr(model, "s3gen", None) is not None:
        return "chunked_meanflow"
    return "refuse"


def f32_to_s16le(samples) -> bytes:
    """Mono float samples in about [-1, 1] to little-endian s16."""
    out = bytearray()
    pack = struct.pack
    for raw in samples:
        x = float(raw)
        if x > 1.0:
            x = 1.0
        elif x < -1.0:
            x = -1.0
        out += pack("<h", int(x * 32767.0))
    return bytes(out)


def flatten_audio(audio) -> list:
    """PCM floats from a tensor, a chunk object, or a nested list."""
    if hasattr(audio, "detach"):
        audio = audio.detach()
    if hasattr(audio, "cpu"):
        audio = audio.cpu()
    if hasattr(audio, "float"):
        try:
            audio = audio.float()
        except Exception:
            pass
    if hasattr(audio, "reshape"):
        try:
            audio = audio.reshape(-1).tolist()
        except Exception:
            pass
    while isinstance(audio, (list, tuple)) and audio and isinstance(audio[0], (list, tuple)):
        audio = audio[0]
    return list(audio)


def protocol_chunks(text: str) -> list:
    """Two PCM writes. The first is one sample so a reader can clock TTFA.

    Same bytes as the old single write: a length sample, then 200 tone
    samples. Not a voice. The gap is in the worker, between the writes.
    """
    n = max(1, min(len(text), 32767))
    tone = 1000 + (n % 50)
    head = struct.pack("<h", n)
    tail = struct.pack("<" + "h" * 200, *([tone] * 200))
    return [head, tail]


def write_chunks(pcm, chunks, *, engine: str, persona: str, path: str, err) -> tuple:
    """Frame a PCM iterator. Chunk marker, then bytes, then the utt marker.

    The marker is written before the bytes so a reader that polls stderr
    does not deadlock on a full stdout pipe. Each chunk stays far under
    the 64 KB pipe buffer. ``generate()`` is not called here.
    """
    err.write(f"galion-clone-engine: {engine}\n")
    err.write(f"galion-clone-path: {path}\n")
    if persona:
        err.write(f"galion-clone-persona: {persona}\n")
    err.flush()
    total = 0
    count = 0
    failed = ""
    try:
        for chunk in chunks:
            if not chunk:
                continue
            err.write(f"galion-clone-chunk: {len(chunk)}\n")
            err.flush()
            pcm.write(chunk)
            pcm.flush()
            total += len(chunk)
            count += 1
    except Exception as exc:
        failed = str(exc).replace("\n", " ")
    if failed:
        err.write(f"galion-clone-note: stream failed: {failed}\n")
    if total < 2:
        err.write("galion-clone-unwired\n")
        err.write("galion-clone-engine: unwired\n")
        err.write("galion-clone-path: none\n")
    err.write(f"galion-clone-utt: {total}\n")
    err.flush()
    return count, total


def write_unwired(err, reason: str) -> None:
    note = " ".join(str(reason).split())
    err.write("galion-clone-unwired\n")
    err.write("galion-clone-engine: unwired\n")
    err.write("galion-clone-path: none\n")
    if note:
        err.write(f"galion-clone-note: {note}\n")
    err.write("galion-clone-utt: 0\n")
    err.flush()


def sha256_file(path: str) -> str:
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        while True:
            block = handle.read(1 << 20)
            if not block:
                break
            digest.update(block)
    return digest.hexdigest()


def allowlist() -> dict:
    """persona -> set of sha256 hex. Empty unless the operator listed some.

    ``GALION_CLONE_REF_SHA=samantha:<hex>,jarvis:<hex>``
    ``GALION_CLONE_REF_ALLOW`` is a file of ``persona hex`` lines.
    """
    found: dict = {}

    def add(persona: str, digest: str) -> None:
        persona = persona.strip().lower()
        digest = digest.strip().lower()
        if len(digest) != 64 or any(c not in "0123456789abcdef" for c in digest):
            return
        if persona not in ("samantha", "jarvis"):
            return
        found.setdefault(persona, set()).add(digest)

    raw = os.environ.get("GALION_CLONE_REF_SHA", "")
    for item in raw.split(","):
        if ":" not in item:
            continue
        persona, digest = item.split(":", 1)
        add(persona, digest)
    path = os.environ.get("GALION_CLONE_REF_ALLOW", "").strip()
    if path and os.path.isfile(path):
        with open(path, "r", encoding="utf-8") as handle:
            for line in handle:
                line = line.strip()
                if not line or line.startswith("#"):
                    continue
                parts = line.split()
                if len(parts) >= 2:
                    add(parts[0], parts[1])
    return found


def ref_decision(persona: str, msg: dict) -> tuple:
    """Return ``(path or None, None)`` or ``(None, refuse_reason)``.

    No reference field: ``(None, None)``. A reference with the local flag
    off, or a hash that is not on the allowlist, is a reason string. The
    worker exits 3. LOCKS.toml is not read.
    """
    present = [(key, msg.get(key)) for key in ("reference", "audio_prompt", "audio_prompt_path", "clip") if msg.get(key)]
    if not present:
        return None, None
    if os.environ.get("GALION_CLONE_LOCAL_REFS") != "1":
        return None, "reference clips are on the rights hold"
    if len(present) != 1 or present[0][0] != "audio_prompt_path":
        return None, "reference clips are on the rights hold"
    path = str(present[0][1])
    if not os.path.isfile(path):
        return None, "reference clip is not on the local allowlist"
    digest = sha256_file(path)
    allowed = allowlist().get(persona, set())
    if digest not in allowed:
        return None, "reference clip is not on the local allowlist"
    return path, None


def checkpoint_dir() -> tuple:
    """Flat directory ``from_local`` can read, or ``(None, reason)``.

    Does not download and does not copy weights into the repo.
    """
    model = os.environ.get("GALION_CLONE_MODEL", "nano").strip().lower()
    if model not in ("nano", "turbo"):
        model = "nano"
    t3 = f"t3_{model}_v1.safetensors"
    needed = ("ve.safetensors", "s3gen_meanflow.safetensors", t3, "vocab.json")
    raw = os.environ.get("GALION_CLONE_CKPT", "").strip()
    if not raw:
        return None, "GALION_CLONE_CKPT is unset; pinned files are not a flat Nano checkpoint"
    missing = [name for name in needed if not os.path.isfile(os.path.join(raw, name))]
    if missing:
        return None, "checkpoint missing " + ",".join(missing)
    return raw, ""


def try_load():
    """Load Nano (or Turbo) on CPU. Torch is imported only here.

    Returns ``(model, "")`` or ``(None, reason)``. A failure does not
    download and does not call generate().
    """
    ckpt, reason = checkpoint_dir()
    if ckpt is None:
        return None, reason
    try:
        import torch
        from chatterbox.tts_turbo import ChatterboxTurboTTS
    except Exception as exc:
        return None, "chatterbox import failed: " + " ".join(str(exc).split())
    nano = os.environ.get("GALION_CLONE_MODEL", "nano").strip().lower() != "turbo"
    threads = os.environ.get("GALION_CLONE_THREADS", "").strip()
    if threads.isdigit():
        torch.set_num_threads(int(threads))
    try:
        model = ChatterboxTurboTTS.from_local(ckpt, "cpu", nano=nano)
    except TypeError:
        try:
            model = ChatterboxTurboTTS.from_local(ckpt, "cpu")
        except Exception as exc:
            return None, "from_local failed: " + " ".join(str(exc).split())
    except Exception as exc:
        return None, "from_local failed: " + " ".join(str(exc).split())
    return model, ""


def iter_pcm(model, text: str, ref_path: str | None):
    """Yield s16le chunks. Raises ``NoStreamHook`` rather than calling generate()."""
    path = select_path(model)
    if path == "refuse":
        raise NoStreamHook("no streaming hook; refusing whole-utterance generate")
    if ref_path:
        model.prepare_conditionals(ref_path, exaggeration=0.0, norm_loudness=True)
    if path == "stream_api":
        yield from _pcm_from_stream_api(model, text)
        return
    yield from _pcm_from_chunked_meanflow(model, text)


def _pcm_from_stream_api(model, text: str):
    import inspect

    fn = model.stream
    kwargs = {}
    try:
        params = inspect.signature(fn).parameters
    except (TypeError, ValueError):
        params = {}
    chunk = _chunk_tokens()
    if "chunk_tokens" in params:
        kwargs["chunk_tokens"] = chunk
    if "crossfade_ms" in params:
        kwargs["crossfade_ms"] = 12.0
    for chunk_obj in fn(text, **kwargs):
        audio = getattr(chunk_obj, "audio", chunk_obj)
        raw = f32_to_s16le(flatten_audio(audio))
        if raw:
            yield raw


def _chunk_tokens() -> int:
    raw = os.environ.get("GALION_CLONE_CHUNK_TOKENS", "").strip()
    if raw.isdigit() and int(raw) > 0:
        return int(raw)
    return CHUNK_TOKENS


def _pcm_from_chunked_meanflow(model, text: str):
    import torch
    from chatterbox.tts_turbo import punc_norm

    if model.conds is None:
        raise NoStreamHook("conditionals absent; reference clips stay on the rights hold")
    text = punc_norm(text)
    text_tokens = model.tokenizer(text, return_tensors="pt", padding=True, truncation=True)
    text_tokens = text_tokens.input_ids.to(model.device)
    lookahead = int(getattr(model.s3gen.flow, "pre_lookahead_len", LOOKAHEAD_TOKENS))
    ratio = int(getattr(model.s3gen.flow, "token_mel_ratio", TOKEN_MEL_RATIO))
    chunk = _chunk_tokens()
    state = _DecodeState(model, lookahead=lookahead, ratio=ratio)
    stop = int(model.t3.hp.stop_speech_token)
    vocab = SPEECH_VOCAB_SIZE
    try:
        from chatterbox.models.s3tokenizer import SPEECH_VOCAB_SIZE as packed_vocab

        vocab = int(packed_vocab)
    except Exception:
        pass
    with torch.inference_mode():
        for token in _iter_turbo_tokens(model, text_tokens):
            value = int(token.view(-1)[0].item())
            if value == stop:
                break
            if value >= vocab:
                continue
            state.tokens.append(value)
            if flush_due(len(state.tokens), lookahead=lookahead, chunk=chunk, final=False):
                raw = state.emit(finalize=False)
                if raw:
                    yield raw
        for _ in range(3):
            state.tokens.append(S3GEN_SIL)
        raw = state.emit(finalize=True)
        if raw:
            yield raw


class _DecodeState:
    """Prefix re-decode with stable meanflow noise and a HiFT source cache.

    Same shape as PR 528's streamer: each flush vocodes the whole prefix
    and yields only the new tail. Noise already drawn is kept so the tail
    does not resample the head. This is the in-tree hook while ``stream``
    is absent. Boundary clicks are still possible; ``model.stream`` wins
    when the install has it.
    """

    def __init__(self, model, *, lookahead: int, ratio: int):
        self.model = model
        self.lookahead = lookahead
        self.ratio = ratio
        self.tokens: list = []
        self.noise = None
        self.cache = None
        self.emitted = 0

    def emit(self, *, finalize: bool) -> bytes:
        import torch

        if not self.tokens:
            return b""
        device = self.model.s3gen.device
        speech = torch.tensor([self.tokens], dtype=torch.long, device=device)
        effective = speech.shape[-1] if finalize else max(0, speech.shape[-1] - self.lookahead)
        need = int(effective) * self.ratio
        if need <= 0:
            return b""
        self.noise = _grow_noise(self.noise, need, device=device, dtype=self.model.s3gen.dtype)
        mels = self.model.s3gen(
            speech_tokens=speech,
            ref_wav=None,
            ref_sr=None,
            ref_dict=self.model.conds.gen,
            n_cfm_timesteps=N_CFM_TIMESTEPS,
            finalize=finalize,
            skip_vocoder=True,
            noised_mels=self.noise[:, :, :need],
        )
        mels = mels.to(dtype=self.model.s3gen.dtype)
        if self.cache is None:
            self.cache = torch.zeros(1, 1, 0, device=device, dtype=self.model.s3gen.dtype)
        wav, source = self.model.s3gen.hift_inference(mels, self.cache)
        self.cache = source.detach()
        wav = wav.squeeze(0)
        fade = self.model.s3gen.trim_fade
        n_fade = int(fade.shape[-1])
        if int(wav.shape[-1]) >= n_fade:
            wav = wav.clone()
            wav[:n_fade] *= fade.to(dtype=wav.dtype)
        samples = wav.detach().float().cpu().reshape(-1)
        if int(samples.shape[-1]) <= self.emitted:
            return b""
        tail = samples[self.emitted :]
        self.emitted += int(tail.shape[-1])
        return f32_to_s16le(tail.tolist())


def _grow_noise(noise, need: int, *, device, dtype):
    import torch

    if noise is None:
        return torch.randn(1, 80, need, device=device, dtype=dtype)
    have = int(noise.shape[-1])
    if have >= need:
        return noise
    extra = torch.randn(1, 80, need - have, device=device, dtype=dtype)
    return torch.cat([noise, extra], dim=-1)


def _iter_turbo_tokens(model, text_tokens):
    """Yield each sampled speech token. Does not call ``inference_turbo``.

    The loop matches the public KV-cache body of ``T3.inference_turbo``
    on master: one prefill, then one token per step. tqdm is not used.
    Nothing is written to stdout.
    """
    import torch
    import torch.nn.functional as F
    from transformers.generation.logits_process import (
        LogitsProcessorList,
        RepetitionPenaltyLogitsProcessor,
        TemperatureLogitsWarper,
        TopKLogitsWarper,
        TopPLogitsWarper,
    )

    temperature = 0.8
    top_k = 1000
    top_p = 0.95
    repetition_penalty = 1.2
    max_gen_len = 1000
    processors = LogitsProcessorList()
    if temperature > 0 and temperature != 1.0:
        processors.append(TemperatureLogitsWarper(temperature))
    if top_k > 0:
        processors.append(TopKLogitsWarper(top_k))
    if top_p < 1.0:
        processors.append(TopPLogitsWarper(top_p))
    if repetition_penalty != 1.0:
        processors.append(RepetitionPenaltyLogitsProcessor(repetition_penalty))

    t3 = model.t3
    speech_start = t3.hp.start_speech_token * torch.ones_like(text_tokens[:, :1])
    embeds, _ = t3.prepare_input_embeds(
        t3_cond=model.conds.t3,
        text_tokens=text_tokens,
        speech_tokens=speech_start,
        cfg_weight=0.0,
    )
    generated = []
    outputs = t3.tfmr(inputs_embeds=embeds, use_cache=True)
    hidden = outputs[0]
    past = outputs.past_key_values
    logits = t3.speech_head(hidden[:, -1:])
    processed = processors(speech_start, logits[:, -1, :])
    next_token = torch.multinomial(F.softmax(processed, dim=-1), num_samples=1)
    generated.append(next_token)
    yield next_token
    current = next_token
    for _ in range(max_gen_len):
        embed = t3.speech_emb(current)
        outputs = t3.tfmr(inputs_embeds=embed, past_key_values=past, use_cache=True)
        hidden = outputs[0]
        past = outputs.past_key_values
        logits = t3.speech_head(hidden)
        ids = torch.cat(generated, dim=1)
        processed = processors(ids, logits[:, -1, :])
        if torch.all(processed == -float("inf")):
            break
        next_token = torch.multinomial(F.softmax(processed, dim=-1), num_samples=1)
        generated.append(next_token)
        current = next_token
        if torch.all(next_token == t3.hp.stop_speech_token):
            break
        yield next_token
