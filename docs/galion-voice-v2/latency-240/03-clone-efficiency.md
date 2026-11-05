# 03 · Clone Voice Efficiency (Chatterbox Nano, CPU)

Agent 03, branch `swarm/lat240-clone-efficiency` (worktree `wt-03-clone-efficiency`), code in
`scripts/clone/lat240/`. Raw data: `results/lat240-clone/<config>/{bench,similarity}.json`, logs `batch1.log`, `batch2.log`, `sim1.log`, `sim2.log`.
Written 2026-11-03, Europe/Zagreb (CET).

**Owner rulings applied:** the Python Chatterbox worker is benchmark-only and never shipped. Clone-at-rest is the
planned path: pre-made clips first, then a Piper-class voice per persona. Piper stays the TTS engine. No Armstrong clone:
only the CC0 refs amy/alan/pdx were conditioned. Similarity floor is 0.6 and the production target is 0.75 (ECAPA).
The live voice.galion.app deployment was not touched. Nothing was force-pushed, and nothing was committed to main.
So the numbers below make the **benchmark lane and clip pre-generation** cheaper. They do not put a shipped Python stage in the call path.

## Method
- Every timed number ran under `flock latency-240/BENCH.lock`, with 4 torch threads and ChatterboxTurboTTS nano from `ckpt-nano`.
  Each config used a fresh process, one warm-up generate that is not counted, then 3 voices × 5 texts = 15 generates
  with fixed seeds (1000+i). Texts: 3 long (~4-5 s audio) and 2 short clauses (~1.5 s).
- **Host load was not quiet.** Two live nano workers and llama-server belonged to other agents and did not take the lock.
  Each row lists the 1-min loadavg at the end of its run. Compare rows only roughly. The base row is its own control.
- Similarity uses the same ECAPA recipe as `tools/spk_sim.py`: cosine of each clone wav against its own donor ref.
  "long" means the 3 long sentences. ECAPA on 1.5 s clauses is unreliable: the unmodified baseline drops to 0.50 there.
- Seed robustness: `q-seed2000/`, `q-seed3000/` hold 2 extra seed sets for base, s1, and q8t3+s1 on the long sentences
  (quality only; those runs were niced and **not** under the lock, so their timings are void).

## (a) Stage split (base, 2 meanflow steps, 4 threads, loadavg 7.9)
| stage | p50 ms | share |
|---|---|---|
| T3 prefill (text only; cond prefix KV cached per voice) | 25 | 1 % |
| T3 token loop (~11 ms/token at 25 tok/s of audio) | 985 (incl. prefill) | 47 % |
| S3Gen flow (encoder + 2-step meanflow U-Net over prompt+tokens) | 534 | 25 % |
| HiFT vocoder | 469 | 22 % |
| Perth watermark | 13 | <1 % |
| **total** | **2110 / p95 2667** | long sentence 2266 (RTF 0.553), short clause 1097 |

Caching the cond-prefix KV (`kvoff` vs base) saves about 40 ms of prefill per sentence (66 → 25 ms). The ~6 s refs are
only ~160 cond tokens, so this is a small win, and its similarity is identical.

## (b) Techniques (whole-sentence, BENCH.lock, 4 threads)
| config | load | total p50/p95 | long p50 | short p50 | T3 | flow | HiFT | first audio | RTF long | long sim mean amy/alan/pdx | long min | all-clip min | verdict |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| base | 7.9 | 2110 / 2667 | 2266 | 1097 | 985 | 534 | 469 | = total | 0.553 | 0.68/0.71/0.62 | 0.598 | 0.499 | control |
| kvoff | 7.3 | 2020 / 2758 | 2287 | 1128 | 1058 | 604 | 322 | = total | 0.565 | 0.68/0.71/0.62 | 0.598 | 0.499 | KV cache kept (prefill -40 ms) |
| s1 (1 meanflow step) | 7.7 | 1915 / 2310 | 2029 | 951 | 1125 | **347** | 328 | = total | 0.484 | 0.70/0.71/0.64 | 0.614 | 0.545 | **keep**: flow -35 %, sim same |
| q8t3 (int8 dynamic GPT2) | 10.9 | 1952 / 3195 | 2056 | 1115 | **729** | 649 | 386 | = total | 0.496 | 0.70/0.74/0.66 | 0.624 | 0.444 | **keep**: T3 -26..-33 % |
| q8s3 (int8 flow enc+dec) | 12.9 | 2103 / 2949 | 2391 | 1188 | 1094 | 563 | 350 | = total | 0.576 | 0.64/0.69/0.59 | **0.576** | 0.450 | **reject**: below floor, little gain |
| p3 (S3Gen prompt cut to 3 s) | 10.4 | 1840 / 2379 | 1886 | 844 | 1010 | 378 | 324 | = total | 0.474 | 0.62/0.68/0.61 | **0.552** | 0.469 | **reject**: below floor |
| ort (ONNX Runtime: flow U-Net + HiFT conv core) | 7.0 | 1929 / 2441 | 2167 | 970 | 962 | 576 | 357 | = total | 0.509 | 0.68/0.71/0.62 | 0.599 | 0.499 | parity 6.6e-6 max abs; HiFT about -24 %, flow no gain; optional |
| **q8t3+s1** | 6.4 | **1363 / 1762** | 1538 | 850 | 662 | 324 | 386 | = total | **0.358** | 0.71/0.75/0.68 | 0.636 | 0.474 | **best whole-sentence** |
| q8t3+s1+ort | 5.3 | 1345 / 1657 | 1523 | 741 | 641 | 334 | 368 | = total | 0.346 | 0.71/0.75/0.68 | 0.636 | 0.474 | same within noise |
| torch.compile (flow U-Net) | — | not measured | | | | | | | | | | | queued; BENCH.lock deadlock (see blockers) |

Seed repeats for q8t3+s1, long sentences: per-seed minima of 0.636 / 0.610 / 0.625, so all 27 clips are ≥ 0.6. Base gives
0.598 / 0.622 / 0.621. s1 gives 0.614 / 0.613 / 0.611. int8 T3 changes the sampled token path, so its per-clip spread is
wider, but its means are not lower than base.

## (c) Chunked / streaming output (`nano_fast.generate_stream`)
- T3 yields tokens. After 12 tokens (and then every 36), S3Gen flow is re-run over all tokens so far with
  `finalize=False` (3-token lookahead held back). Upstream's non-final path has a mask-length bug, fixed in a local copy of
  `flow.inference`. Only new mel frames go to HiFT, which keeps a 4-frame mel/source cache and crossfades
  CosyVoice-style. The fixed noise is sliced per call, so frames stay consistent. Each chunk is Perth-watermarked
  (checked: chunk length is preserved).
- First chunk = 280 ms of audio.

| config | load | **first clone audio p50 / p95** | sentence total p50 | RTF long | long sim mean | long min |
|---|---|---|---|---|---|---|
| stream (2 steps, fp32, every 24) | 8.7 | 604 / 804 | 4050 | **1.242 (underruns)** | 0.68/0.76/0.68 | 0.655 |
| stream+q8t3+s1 (every 36) | 5.0 | 340 / 442 | 2021 | 0.569 | 0.70/0.74/0.70 | 0.679 |
| **stream+q8t3+s1+ort (every 36)** | 4.3 | **336 / 396** | 2005 | 0.521 | 0.70/0.74/0.70 | 0.679 |

Best measured time to first clone audio is **336 ms p50 / 396 ms p95**, against a non-streaming baseline that waits for the
whole sentence (2110 / 2667 ms; short clause 1097 ms). That is −84 % at p50. The first chunk breaks down as roughly:
prefill 17 ms, 12 tokens × ~10 ms, flow over prompt+12 tokens, then HiFT on 14 frames.
Plain streaming without the speedups re-runs flow too often, and its RTF of 1.24 would starve playback.

Protocol check of `scripts/clone/lat240/nano_stream_worker.py` (`say_stream` op): first chunk at 357 ms, 3 chunks, then
`done`. This was a functional run at 2 threads, niced and not under the lock, so it is not a benchmark. Output is in `results/lat240-clone/worker-stream-amy.wav`.

## (d) Similarity verdict
Every kept config holds the long-sentence floor (≥ 0.61 across 3 seed sets). No config, the unmodified baseline included,
reaches the 0.75 production target on all donors (best means: amy 0.71, alan 0.75, pdx 0.70). That gap belongs to the
model and the ~6 s refs, not to any speedup. q8s3 and p3 are rejected because they fall below 0.6.

## Projection (estimate, not measured)
Bench numbers include contention from about loadavg 5. The offline baseline at 8 threads on a quiet host was 2246 ms. On a quiet
4-core slot, 336 ms is likely an upper bound. A first chunk of 8 tokens (160 ms audio) would remove ~4 token steps
plus some flow cost: **estimate 250-300 ms**. It is still unlikely to reach ≤ 240 ms on CPU, because flow re-encodes the full
ref prompt on every chunk. Hence the owner's clone-at-rest plan stands: the live first word comes from Piper or pre-made clips.
For pre-generating clips or a Piper-voice training set, q8t3+s1 cuts generation wall time by about 35 % (RTF 0.553 → 0.358).

## Production checklist (pass/fail)
| # | item | status |
|---|---|---|
| 1 | T3 vs S3Gen (flow / HiFT / watermark) split instrumented and measured | PASS |
| 2 | Fewer meanflow steps measured with similarity | PASS (1 step kept) |
| 3 | int8 dynamic quant of T3 linears measured with similarity | PASS (kept) |
| 4 | int8 dynamic quant of S3Gen linears measured with similarity | PASS (rejected, < 0.6) |
| 5 | ONNX export of the S3Gen hot parts, parity checked and timed | PASS (6.6e-6, HiFT faster) |
| 6 | torch.compile measured | FAIL (queued behind the stuck BENCH.lock) |
| 7 | ONNX / int8 for the T3 token loop through ORT (the largest stage) | FAIL (not attempted) |
| 8 | Chunked streaming output implemented, with a crossfade and a watermark per chunk | PASS |
| 9 | Time to first clone audio ≤ 400 ms p50 and p95 (locked bench) | PASS (336 / 396) |
| 10 | Time to first clone audio ≤ 240 ms | FAIL (estimate 250-300 ms with an 8-token first chunk) |
| 11 | Streaming RTF < 1 (no underrun) | PASS (0.52) |
| 12 | Kept configs ≥ 0.6 ECAPA on long sentences, 3 seed sets | PASS |
| 13 | ≥ 0.75 ECAPA production target on all donors | FAIL (baseline fails too) |
| 14 | ≥ 0.6 on short clauses (~1.5 s) | FAIL (ECAPA unreliable at this length; baseline 0.50) |
| 15 | Clean re-run at low load under BENCH.lock | FAIL (lock deadlock; queued in `results/lat240-clone/clean*`) |
| 16 | Python worker benchmark-only, Armstrong never cloned, CC0 refs only, no new weights | PASS (onnxruntime 1.31.1 is an MIT package in `latency-240/03-pylib`; the shared venv is untouched; the .onnx files are derived from the MIT weights and not committed) |

**Completion: 10 / 16 = 62.5 %.**

## Blockers
- **BENCH.lock is held by an orphan.** From about 19:41 CET, `llama-server` pid 868780 (qwen2.5-1.5b, port 18080, started by
  another agent's flock'd script) inherited the lock fd and holds it, together with a curl (pid 873792) that has been
  stuck for 13+ minutes. Seven flock waiters are blocked, my clean re-run, `first=8`, and torch.compile among them. Per the rules I did not stop it.
  The owner of that run needs to stop it or close the fd.
- The ~6 s CC0 refs cap similarity near 0.70. Longer CC0 refs (≥ 10 s, the DEC_COND_LEN) might lift it toward 0.75, but each
  would need a licence check.
- Housekeeping: I removed my own untracked scratch `smoke.py` one minute after creating it, then restored it from the same
  content. Nothing else was deleted.
