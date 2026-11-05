# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Experiment helper (not shipped, LOCKS v2s14 out-of-process lane only).
Instrumented, optionally quantized / fewer-step / cond-KV-cached / streaming
Chatterbox Nano generate. Monkeypatch-free: the upstream package is untouched;
this file re-implements the generate path with timers around each stage.
CC0 donors only (amy, alan, pdx). No downloads (HF offline)."""
import os, time, copy, math
os.environ.setdefault("TQDM_DISABLE", "1")
os.environ.setdefault("HF_HUB_OFFLINE", "1"); os.environ.setdefault("TRANSFORMERS_OFFLINE", "1")
import torch, torch.nn as nn, torch.nn.functional as F, numpy as np
from transformers.generation.logits_process import (LogitsProcessorList, TemperatureLogitsWarper,
    TopKLogitsWarper, TopPLogitsWarper, RepetitionPenaltyLogitsProcessor)
from chatterbox.tts_turbo import ChatterboxTurboTTS, punc_norm
from chatterbox.models.s3gen.const import S3GEN_SIL
import chatterbox.models.s3gen.flow_matching as _fm
_fm.print = lambda *a, **k: None  # silence the per-call "S3 Token -> Mel" print

CKPT = "/workspace/swarm/voice-v2-sprint/ckpt-nano"
REFS = "/workspace/swarm/voice-v2-sprint/refs"
VOICES = ("amy", "alan", "pdx")  # CC0 only. Never add a celebrity ref here.

def now(): return time.perf_counter()

def load(threads=4):
    torch.set_num_threads(threads)
    t = now(); M = ChatterboxTurboTTS.from_local(CKPT, "cpu", nano=True)
    return M, (now() - t) * 1000

def conv1d_to_linear(mod):
    """HF GPT2 uses transformers Conv1D (weight [in,out]); dynamic quant needs nn.Linear."""
    from transformers.pytorch_utils import Conv1D
    for name, ch in list(mod.named_children()):
        if isinstance(ch, Conv1D):
            lin = nn.Linear(ch.weight.shape[0], ch.weight.shape[1])
            lin.weight.data = ch.weight.data.t().contiguous(); lin.bias.data = ch.bias.data
            setattr(mod, name, lin)
        else:
            conv1d_to_linear(ch)

def quant_t3(M):
    conv1d_to_linear(M.t3.tfmr)
    M.t3.tfmr = torch.ao.quantization.quantize_dynamic(M.t3.tfmr, {nn.Linear}, dtype=torch.qint8)
    M.t3.speech_head = torch.ao.quantization.quantize_dynamic(nn.Sequential(M.t3.speech_head), {nn.Linear}, dtype=torch.qint8)[0]

def quant_s3(M, parts=("encoder", "decoder")):
    fl = M.s3gen.flow
    if "encoder" in parts:
        fl.encoder = torch.ao.quantization.quantize_dynamic(fl.encoder, {nn.Linear}, dtype=torch.qint8)
    if "decoder" in parts:
        fl.decoder.estimator = torch.ao.quantization.quantize_dynamic(fl.decoder.estimator, {nn.Linear}, dtype=torch.qint8)

class Voice:
    """Per-voice cache: conditionals plus (optionally) the T3 KV cache of the
    constant cond prefix (speaker emb + prompt speech tokens)."""
    def __init__(self, M, name, ref=None, prompt_s=None):
        assert name in VOICES, "CC0 donors only"
        t = now()
        M.prepare_conditionals(ref or f"{REFS}/{name}.wav")
        self.conds = M.conds; self.name = name
        if prompt_s:  # keep only the last prompt_s seconds of the S3Gen prompt (25 tok/s, 50 mel/s)
            g = self.conds.gen; n = int(prompt_s * 25)
            if g["prompt_token"].size(1) > n:
                g["prompt_token"] = g["prompt_token"][:, -n:].contiguous()
                g["prompt_feat"] = g["prompt_feat"][:, -2 * n:].contiguous()
                g["prompt_token_len"] = torch.LongTensor([n]); g["prompt_feat_len"] = torch.LongTensor([2 * n])
        self.cond_ms = (now() - t) * 1000
        with torch.inference_mode():
            self.cond_emb = M.t3.prepare_conditioning(self.conds.t3)  # (1, Lc, D)
            t = now(); out = M.t3.tfmr(inputs_embeds=self.cond_emb, use_cache=True)
            self.kv = out.past_key_values; self.kv_ms = (now() - t) * 1000

def _procs(temperature=0.8, top_k=1000, top_p=0.95, repetition_penalty=1.2):
    lp = LogitsProcessorList()
    lp.append(TemperatureLogitsWarper(temperature)); lp.append(TopKLogitsWarper(top_k))
    lp.append(TopPLogitsWarper(top_p)); lp.append(RepetitionPenaltyLogitsProcessor(repetition_penalty))
    return lp

@torch.inference_mode()
def t3_tokens(M, V, text, kv_cache=True, max_gen_len=1000, stats=None):
    """Generator of speech tokens (ints); same sampling as inference_turbo."""
    t3 = M.t3; lp = _procs()
    text = punc_norm(text)
    tt = M.tokenizer(text, return_tensors="pt").input_ids
    start = t3.hp.start_speech_token * torch.ones_like(tt[:, :1])
    t0 = now()
    te = t3.text_emb(tt); se = t3.speech_emb(start)
    if kv_cache:
        pkv = copy.deepcopy(V.kv)
        out = t3.tfmr(inputs_embeds=torch.cat([te, se], 1), past_key_values=pkv, use_cache=True)
    else:
        out = t3.tfmr(inputs_embeds=torch.cat([V.cond_emb, te, se], 1), use_cache=True)
    pkv = out.past_key_values
    logits = t3.speech_head(out[0][:, -1:])
    nxt = torch.multinomial(F.softmax(lp(start, logits[:, -1, :]), -1), 1)
    if stats is not None: stats["prefill_ms"] = (now() - t0) * 1000
    gen = [nxt]
    for _ in range(max_gen_len):
        if int(nxt) == t3.hp.stop_speech_token: break
        yield int(nxt)
        out = t3.tfmr(inputs_embeds=t3.speech_emb(nxt), past_key_values=pkv, use_cache=True)
        pkv = out.past_key_values
        logits = t3.speech_head(out[0])
        nxt = torch.multinomial(F.softmax(lp(torch.cat(gen, 1), logits[:, -1, :]), -1), 1)
        gen.append(nxt)

@torch.inference_mode()
def flow_mel(M, V, toks, finalize, steps, noise):
    """Copy of CausalMaskedDiffWithXvec.inference for B=1 with one fix: for
    finalize=False upstream trims h by the lookahead but not h_masks, so the
    decoder mask is 6 frames too long (shape error). Here both are trimmed."""
    from chatterbox.models.s3gen.utils.mask import make_pad_mask
    fl = M.s3gen.flow; rd = V.conds.gen
    tok = torch.tensor(toks, dtype=torch.long)[None]
    emb = fl.spk_embed_affine_layer(F.normalize(torch.atleast_2d(rd["embedding"]), dim=1))
    pf = rd["prompt_feat"]; tok = torch.cat([rd["prompt_token"], tok], 1)
    tl = torch.LongTensor([tok.size(1)])
    h, hm = fl.encoder(fl.input_embedding(tok), tl)
    if not finalize:
        cut = fl.pre_lookahead_len * fl.token_mel_ratio
        h = h[:, :-cut]; hm = hm[:, :, :-cut]
    ml1 = pf.shape[1]; ml2 = h.shape[1] - ml1
    h = fl.encoder_proj(h)
    conds = torch.zeros([1, ml1 + ml2, fl.output_size], dtype=h.dtype); conds[:, :ml1] = pf
    mask = hm.to(h.dtype)
    feat, _ = fl.decoder(mu=h.transpose(1, 2).contiguous(), mask=mask, spks=emb, cond=conds.transpose(1, 2),
                         n_timesteps=steps, noised_mels=noise[:, :, :ml2], meanflow=True)
    return feat[:, :, ml1:]

def watermark(M, a):
    return M.watermarker.apply_watermark(a, sample_rate=M.sr)

@torch.inference_mode()
def generate(M, V, text, steps=2, kv_cache=True, wm=True, seed=None):
    """Whole-sentence generate with a stage split. Returns (float32 wav, stats)."""
    if seed is not None: torch.manual_seed(seed)
    st = {}; t0 = now()
    toks = list(t3_tokens(M, V, text, kv_cache=kv_cache, stats=st))
    st["t3_ms"] = (now() - t0) * 1000; st["n_tok"] = len(toks)
    toks = [x for x in toks if x < 6561] + [S3GEN_SIL] * 3
    noise = torch.randn(1, 80, len(toks) * 2)
    t = now(); mel = flow_mel(M, V, toks, True, steps, noise); st["flow_ms"] = (now() - t) * 1000
    t = now(); wav, _ = M.s3gen.hift_inference(mel, None); st["hift_ms"] = (now() - t) * 1000
    wav[:, :len(M.s3gen.trim_fade)] *= M.s3gen.trim_fade
    a = wav.squeeze(0).float().numpy()
    if wm:
        t = now(); a = watermark(M, a); st["wm_ms"] = (now() - t) * 1000
    st["total_ms"] = (now() - t0) * 1000; st["audio_ms"] = len(a) * 1000 / M.sr
    return a.astype(np.float32), st

@torch.inference_mode()
def generate_stream(M, V, text, first=12, every=24, steps=2, kv_cache=True, seed=None,
                    mel_cache=4, wm_chunks=False):
    """Chunked output. Yields (pcm float32 chunk, stats-so-far). The first chunk
    is vocoded after `first` T3 tokens, then every `every` tokens. Flow is
    re-run over all tokens so far with finalize=False (last 3 tokens are
    lookahead), only new mel frames go to HiFT, which keeps a short mel+source
    cache and crossfades (CosyVoice-style)."""
    if seed is not None: torch.manual_seed(seed)
    s3 = M.s3gen; hop = 480; src_cache = mel_cache * hop
    win = torch.from_numpy(np.hamming(2 * src_cache)).float() if mel_cache else None
    t0 = now(); st = {"chunks": []}
    noise = torch.randn(1, 80, 2 * 1200)
    toks = []; emitted = 0; c_mel = None; c_src = torch.zeros(1, 1, 0); c_wav = None; first_done = False
    gen = t3_tokens(M, V, text, kv_cache=kv_cache, stats=st)
    def flush(final):
        nonlocal emitted, c_mel, c_src, c_wav, first_done
        tk = [x for x in toks if x < 6561] + ([S3GEN_SIL] * 3 if final else [])
        mel = flow_mel(M, V, tk, final, steps, noise)
        new = mel[:, :, emitted:]
        if new.size(2) == 0 and not final: return None
        emitted = mel.size(2)
        m_in = new if c_mel is None else torch.cat([c_mel, new], 2)
        wav, src = s3.hift_inference(m_in, c_src)
        if not first_done:
            wav[:, :len(s3.trim_fade)] *= s3.trim_fade; first_done = True
        if c_wav is not None and mel_cache:
            n = c_wav.size(1)
            wav[:, :n] = wav[:, :n] * win[:n] + c_wav * win[n:]
        if mel_cache and not final:
            c_mel = m_in[:, :, -mel_cache:]; c_src = src[:, :, -src_cache:]
            c_wav = wav[:, -src_cache:].clone(); out = wav[:, :-src_cache]
        else:
            out = wav
        a = out.squeeze(0).float().numpy()
        if wm_chunks: a = watermark(M, a)
        return a
    for x in gen:
        toks.append(x)
        n = len(toks)
        if n == first or (n > first and (n - first) % every == 0):
            a = flush(False)
            if a is not None and len(a):
                st["chunks"].append(dict(at_ms=(now() - t0) * 1000, n_tok=n, audio_ms=len(a) * 1000 / M.sr))
                if len(st["chunks"]) == 1: st["first_audio_ms"] = st["chunks"][0]["at_ms"]
                yield a, st
    a = flush(True)
    st["chunks"].append(dict(at_ms=(now() - t0) * 1000, n_tok=len(toks), audio_ms=len(a) * 1000 / M.sr))
    st.setdefault("first_audio_ms", st["chunks"][0]["at_ms"])
    st["total_ms"] = (now() - t0) * 1000; st["n_tok"] = len(toks)
    yield a, st
