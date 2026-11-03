#!/usr/bin/env python3
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
"""Streaming variant of worker/nano_live_worker.py (LOCKS v2s14 exception:
PyTorch CPU, out of process). Same JSON protocol plus chunked output.

stdin  : {"op":"cond","voice":v,"ref":path} | {"op":"say","voice":v,"text":t}
         | {"op":"say_stream","voice":v,"text":t}
stdout : say        -> one JSON line + pcm_bytes raw 24 kHz s16le (as before)
         say_stream -> per chunk {"op":"chunk","seq":k,"t_ms":..,"pcm_bytes":n} + pcm,
                       then {"op":"say","done":true,"ms":..,"first_ms":..,"audio_ms":..}
Each chunk is watermarked (Perth) before it leaves the process.
Env: GALION_CLONE_CKPT, GALION_CLONE_THREADS (<=4 per lane), GALION_CLONE_STEPS (1|2, default 1),
     GALION_CLONE_Q8T3 (1 = int8 dynamic T3, default 1), GALION_CLONE_PROMPT_S (default 0 = full ref; 3 s cut drops similarity, see 03 report),
     GALION_CLONE_ORT (1 = ONNX Runtime for flow estimator + HiFT core, default 0),
     GALION_CLONE_FIRST / GALION_CLONE_EVERY (tokens; default 12 / 36).
CC0 donors only; no celebrity reference; no downloads (HF offline)."""
import json, os, sys, time
os.environ.setdefault("TQDM_DISABLE", "1")
_thr = os.environ.get("GALION_CLONE_THREADS", "4").strip()
for _k in ("OMP_NUM_THREADS", "MKL_NUM_THREADS", "OPENBLAS_NUM_THREADS"): os.environ[_k] = _thr
real_fd = os.dup(1); os.dup2(2, 1)
def write_all(b):
    v = memoryview(b)
    while len(v): v = v[os.write(real_fd, v):]
def reply(pcm=b"", **kw):
    if pcm: kw["pcm_bytes"] = len(pcm)
    write_all((json.dumps(kw) + "\n").encode() + pcm)
t0 = time.time()
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import numpy as np, torch
import nano_fast as N
if os.environ.get("GALION_CLONE_CKPT"): N.CKPT = os.environ["GALION_CLONE_CKPT"]
E = lambda k, d: os.environ.get(k, d)
STEPS = int(E("GALION_CLONE_STEPS", "1")); PROMPT_S = float(E("GALION_CLONE_PROMPT_S", "0")) or None
FIRST = int(E("GALION_CLONE_FIRST", "12")); EVERY = int(E("GALION_CLONE_EVERY", "36"))
M, _ = N.load(int(_thr))
if E("GALION_CLONE_Q8T3", "1") == "1": N.quant_t3(M)
if E("GALION_CLONE_ORT", "0") == "1":
    import onnx_parts as O; O.use_ort(M, int(_thr))
V = {}
reply(ok=True, op="ready", load_ms=round((time.time() - t0) * 1000), threads=torch.get_num_threads(), sr=M.sr,
      steps=STEPS, prompt_s=PROMPT_S)
import signal; signal.signal(signal.SIGPIPE, signal.SIG_DFL)
pcm16 = lambda a: (np.clip(a, -1.0, 1.0) * 32767.0).round().astype("<i2").tobytes()
for line in sys.stdin:
    try: r = json.loads(line)
    except Exception: reply(ok=False, err="bad json"); continue
    op = r.get("op"); t = time.time()
    if op == "quit": break
    try:
        if op == "cond":
            V[r["voice"]] = N.Voice.__new__(N.Voice)
            # Voice() asserts CC0 donor names for experiments; the live lane passes persona names
            # whose reference.wav the operator placed (sha-pinned upstream). Same code path otherwise.
            vv = V[r["voice"]]; M.prepare_conditionals(r["ref"]); vv.conds = M.conds; vv.name = r["voice"]
            if PROMPT_S:
                g = vv.conds.gen; n = int(PROMPT_S * 25)
                if g["prompt_token"].size(1) > n:
                    g["prompt_token"] = g["prompt_token"][:, -n:].contiguous(); g["prompt_feat"] = g["prompt_feat"][:, -2 * n:].contiguous()
                    g["prompt_token_len"] = torch.LongTensor([n]); g["prompt_feat_len"] = torch.LongTensor([2 * n])
            with torch.inference_mode():
                vv.cond_emb = M.t3.prepare_conditioning(vv.conds.t3)
                vv.kv = M.t3.tfmr(inputs_embeds=vv.cond_emb, use_cache=True).past_key_values
            reply(ok=True, op="cond", ms=round((time.time() - t) * 1000))
        elif op == "say":
            a, st = N.generate(M, V[r["voice"]], r["text"], steps=STEPS)
            reply(pcm=pcm16(a) or b"\0\0", ok=True, op="say", ms=round((time.time() - t) * 1000),
                  audio_ms=round(st["audio_ms"]), sr=M.sr)
        elif op == "say_stream":
            seq = 0; first = None; am = 0
            for a, st in N.generate_stream(M, V[r["voice"]], r["text"], first=FIRST, every=EVERY, steps=STEPS, wm_chunks=True):
                if not len(a): continue
                ms = round((time.time() - t) * 1000); first = first if first is not None else ms
                am += len(a) * 1000 / M.sr
                reply(pcm=pcm16(a), ok=True, op="chunk", seq=seq, t_ms=ms); seq += 1
            reply(ok=True, op="say", done=True, ms=round((time.time() - t) * 1000), first_ms=first, audio_ms=round(am), chunks=seq, sr=M.sr)
        else:
            reply(ok=False, err="unknown op")
    except Exception as e:
        reply(ok=False, err=f"{type(e).__name__}: {e}"[:300])
