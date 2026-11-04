# CPU foundation

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb.

The symphony runs CPU only, no GPU. Do not import GGUF into the voice crates. GGUF figures below are a capacity reference from the CPU foundation note, not a dependency and not a measurement.

The tick rules that this repo enforces are in [LATENCY.md](LATENCY.md): 80 ms frame, non-inference p99 ≤ 1% of that frame (800 µs), late = pause. Everything in the tables below is planning data. It was not measured on this host, and it is not an EPYC, CX23, or CX43 benchmark.

## Flagship and hosted classes

| Role | Class | Plan | Measured here |
|---|---|---|---|
| LLM / app node | AMD EPYC 9655P, 96 cores / 192 threads, about 614 GB/s | 384 GB DDR5-6400 on the LLM node; 768 GB on the app / voice / research node | not measured |
| Storage | EPYC 9355P, 32 cores, dense NVMe | Ceph, air cooling, Ethernet | not measured |
| Plus | CX23-class | $20 / month, about $6 COGS, voice-to-voice, 100 GB SSD + 16 GB RAM | not measured |
| Ultra | CX43-class | $220 / month, 2 GB SSD | not measured |
| Local free | The user's own Linux or a small board | Does not consume the hosted pool | not measured |

Plus and Ultra are the owner freemium locks. They override any higher Ultra price in the source note.

## Indicative voice and token figures

These are the source note's sizing numbers. Section G does not check them against a clock.

| Figure | Plan | Status |
|---|---|---|
| Decode / prefill | 250 / 1000 tok/s at 70% util on an LLM node (conservative 200/800, optimistic 300/1300) | not measured; GGUF was the sizing reference |
| Voice session | about 1.0 vCPU average (ASR 0.4, TTS 0.3, VAD/SER 0.15, plus reserve); burst about 4 vCPU; about 0.65–1.15 GB RAM | not measured |
| Base peak | about 806 voice streams | not measured |

Hosted scale in that note (55k users, 8250 hosted, 80 servers) is the same kind of figure: a plan, not a result from `scripts/symphony/self-eval.sh`.

## What the tick check is

```bash
bash scripts/symphony/check-g-latency.sh
```

It measures `galion-heartbeat` `FRAME_MS` on the stub path and fails if that p99 is over 800 µs. Inference (ASR, TTS, clone, LLM) is off that tick. The 1% ceiling is not a claim about those models' CPU share.

— Maciej Grajczyk

---

© GOATSI — Galion Open Automated Technarchy Society Inc. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
