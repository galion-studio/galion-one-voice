# Experiment driver (not shipped). Usage: bench.py <config> [config...]
# configs: "+"-joined options: base s1 q8t3 q8s3 q8enc q8dec kvoff p<sec> stream compile
import sys, json, os, time, statistics as S
sys.path.insert(0, os.path.dirname(__file__))
import nano_fast as N, torch, soundfile as sf
OUT = os.environ.get("LAT240_OUT", "/workspace/swarm/voice-v2-sprint/results/lat240-clone")
SB = int(os.environ.get("LAT240_SEED_BASE", "1000"))
TH = int(os.environ.get("LAT240_THREADS", "4"))
TEXTS = ["The train leaves at seven, so pack the night before and set two alarms.",
         "I found the old letters in a shoebox under the stairs, still tied with string.",
         "Honestly, the quiet part of the morning is my favourite hour of the day.",
         "Sure, give me a second.",
         "Okay, that makes sense."]
def pct(xs, p): xs = sorted(xs); return xs[min(len(xs) - 1, int(round(p / 100 * (len(xs) - 1))))]
def run(cfg):
    M, load_ms = N.load(TH)
    opts = set(cfg.split("+"))  # base, s1, q8t3, q8s3, q8enc, q8dec, kvoff, p3/p4/p5, stream, compile
    steps = 1 if "s1" in opts else 2
    if "q8t3" in opts: N.quant_t3(M)
    if "q8s3" in opts: N.quant_s3(M)
    if "q8enc" in opts: N.quant_s3(M, ("encoder",))
    if "q8dec" in opts: N.quant_s3(M, ("decoder",))
    if "compile" in opts:
        M.s3gen.flow.decoder.estimator = torch.compile(M.s3gen.flow.decoder.estimator, dynamic=True)
    if "ortdec" in opts or "orthift" in opts or "ort" in opts:
        import onnx_parts as O
        O.use_ort(M, TH, tuple(x for x, k in (("dec", "ortdec"), ("hift", "orthift")) if k in opts or "ort" in opts))
    if "compileh" in opts:
        M.s3gen.mel2wav.decode = torch.compile(M.s3gen.mel2wav.decode, dynamic=True)
    kv = "kvoff" not in opts
    ps = next((float(o[1:]) for o in opts if o[0] == "p" and o[1:].replace(".", "").isdigit()), None)
    os.makedirs(f"{OUT}/{cfg}", exist_ok=True)
    V = {v: N.Voice(M, v, prompt_s=ps) for v in N.VOICES}
    # warmup (not counted)
    N.generate(M, V["amy"], "Warm up the model please.", steps=steps, kv_cache=kv, seed=0)
    rows = []
    for v in N.VOICES:
        for i, t in enumerate(TEXTS[:int(os.environ.get("LAT240_NTEXT", "5"))]):
            if "stream" in opts:
                fa = int(os.environ.get("LAT240_FIRST", "12")); ev = int(os.environ.get("LAT240_EVERY", "24"))
                parts = []
                for a, st in N.generate_stream(M, V[v], t, first=fa, every=ev, steps=steps, kv_cache=kv, seed=SB + i): parts.append(a)
                import numpy as np; a = np.concatenate(parts)
                st = {k: st[k] for k in ("first_audio_ms", "total_ms", "n_tok", "prefill_ms", "chunks")}
                st["audio_ms"] = len(a) * 1000 / M.sr
                st["first_chunk_audio_ms"] = st["chunks"][0]["audio_ms"]
            else:
                a, st = N.generate(M, V[v], t, steps=steps, kv_cache=kv, seed=SB + i)
            f = f"{OUT}/{cfg}/{v}-{i}.wav"; sf.write(f, a, M.sr, subtype="PCM_16")
            rows.append(dict(voice=v, i=i, text=t, file=f, **st)); print(cfg, v, i, {k: (round(x) if isinstance(x, float) else x) for k, x in st.items() if k != "chunks"}, flush=True)
    summ = dict(cfg=cfg, prompt_s=ps, threads=TH, load_ms=round(load_ms), steps=steps, kv_cache=kv,
                cond_ms={v: round(V[v].cond_ms) for v in V}, kv_prefix_ms={v: round(V[v].kv_ms) for v in V},
                loadavg=os.getloadavg())
    for k in ("total_ms", "t3_ms", "prefill_ms", "flow_ms", "hift_ms", "wm_ms", "first_audio_ms"):
        xs = [r[k] for r in rows if k in r]
        if xs: summ[k] = dict(p50=round(S.median(xs), 1), p95=round(pct(xs, 95), 1))
    long_ = [r for r in rows if r["i"] < 3]; short = [r for r in rows if r["i"] >= 3]
    summ["long_total_p50"] = round(S.median(r["total_ms"] for r in long_), 1)
    if short: summ["short_total_p50"] = round(S.median(r["total_ms"] for r in short), 1)
    summ["rtf_long"] = round(sum(r["total_ms"] for r in long_) / sum(r["audio_ms"] for r in long_), 3)
    json.dump(dict(summary=summ, rows=rows), open(f"{OUT}/{cfg}/bench.json", "w"), indent=1)
    print("SUMMARY", json.dumps(summ), flush=True)
for c in sys.argv[1:]: run(c)
