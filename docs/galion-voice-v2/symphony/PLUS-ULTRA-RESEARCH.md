# Plus, Ultra, and Research

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-03. Symphony section. Index: [README.md](README.md).

The spelling is **Galion**. The product is Galion Voice.

Hosted plan for the symphony, from the CPU note t284 (2026-11-03, Europe/Zagreb). Local stays free. The figures are the plan. A benchmark on the machine replaces a figure when it exists. Until then the row in [SELF-EVAL.md](SELF-EVAL.md) stays **BLOCKED** on "not yet benchmarked" for any number claimed as measured.

| Surface | Plan |
|---|---|
| Local | Free, on the person's own Linux machine or a Raspberry Pi. It does not draw datacenter compute. |
| Plus | $20 / month. CX23-class, about $6 COGS. Voice-to-voice included. 100 GB SSD, 16 GB RAM. |
| Ultra | $220 / month. CX43-class, Titanaxe Max 3XL. 2 GB SSD. |
| Research | Galion Research console. Its own hosted surface. Voice: [GALION-RESEARCH.md](../personas/GALION-RESEARCH.md). |

## CPU the plan is sized for

The hosted fleet in that note is CPU and RAM. Air cooling, leaf-spine Ethernet.

| Node | CPU | Memory |
|---|---|---|
| LLM / app | 1U, 1P AMD EPYC 9655P, 96 cores / 192 threads, ~614 GB/s | 384 GB DDR5-6400 |
| App / voice / research | same CPU | 768 GB DDR5 |
| Storage | EPYC 9355P, 32 cores, dense NVMe, Ceph EC 4+2 | |

Indicative, until measured: about 250 decode tok/s and 1000 prefill tok/s per LLM node at 70% util. A voice session is about 1.0 vCPU average (ASR 0.4, TTS 0.3, VAD/SER 0.15, plus reserve), burst about 4 vCPU, and about 0.65–1.15 GB RAM. The compiler emits a Galion voice file that runs on this class of host and on the smaller Plus and Ultra shapes. See [COMPILER.md](COMPILER.md) and [RUST-TRANSLATION.md](RUST-TRANSLATION.md).

Voice crates read `.gq` and PCM. Capacity sketches that were drawn with another weight container stay sketches.

The one-percent budget and the 80 ms frame apply on every surface, including a Raspberry Pi. See [ONE-PERCENT.md](ONE-PERCENT.md) and [80MS.md](80MS.md).

## Gate

**PASS** when this table matches the owner note and a benchmark appendix, if one exists, is linked from the row.

**FAIL** when a measured number is written here without the machine, the date, and the command.

**BLOCKED** when the row is asked to certify a tok/s or a vCPU figure. The plan is written. The benchmark is not in this landing.

— Maciej Grajczyk
