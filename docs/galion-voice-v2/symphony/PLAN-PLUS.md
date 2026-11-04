# Plus — Galion Voice V2 symphony

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-03. Symphony sizing plan for the Plus SKU (t284).

The spelling is **Galion**. The product is Galion Voice.
Master slogan: "The seamless connection with the human being, on the voice only."

This page sizes Plus. It is the symphony plan for that SKU. The licence table
stays as it is. The README stays without a price list.

## Resources

Plus is **$20 per month**. The cost to serve one subscriber is **CX23-class,
about $6**. The included quota from t284 is **16 GB RAM** and **100 GB SSD**.
Galion Voice voice-to-voice sits inside that $20.

| | Plus |
|---|---|
| Price | $20 / month |
| COGS | CX23-class, about $6 |
| RAM quota | 16 GB |
| SSD quota | 100 GB |
| Voice | voice-to-voice included |
| CPU | pooled, about 1 vCPU per live session |
| GPU | none on this SKU |

The $14 between the price and the COGS is arithmetic on those two figures,
before tax, payment fees, and support.

### The quota is a ceiling

A CX23-class cost is a small shared-vCPU footprint. In this repository the
hardware note prices a dedicated 16 GB cloud box far above $6: CCX23 at
€85.99 (4 dedicated vCPU, 16 GB) and CPX42 at €69.49 (8 vCPU, 16 GB). The
same note records the legacy CX23–CX53 span at €5.49–€29.49, so the bottom
of that class sits next to the $6 COGS. See
[`docs/research/galion-voice-v2/domains/10-hardware-edge.md`](../../research/galion-voice-v2/domains/10-hardware-edge.md).
Those list prices are infra context. Plus holds the ~$6 COGS by treating
16 GB and 100 GB as ceilings on a pooled CPU host.

The working set that fits the cost class is **one live voice session**:
about **1 vCPU** from the pool, and about **0.65–1.15 GB RAM**. Cascade
weights live once on the host image, shared and read-only, and stay off the
subscriber's 100 GB. Live reply audio stays in RAM.

Planning reserve inside the 16 GB ceiling, until a host is measured: 2 GB
for the runtime and the one-percent engine. One session at the high RAM
figure (1.15 GB) leaves the rest of the ceiling for the workspace. A second
concurrent session draws another ~1 vCPU and another ~1 GB from the same
quota. The list price stays $20.

A short burst of about 4 vCPU stays in the pool. It is a fleet spike, and
the $6 footprint does not pin those cores to one subscriber.

The pool may be a rented CPU host at first and the EPYC-class fleet later.
The per-subscriber COGS target stays about $6 either way. Voice stays on
the CPU cascade.

### Disk

100 GB is the durable user volume: Galion-format persona files and the
workspace. Cascade weights are counted once, on the host image.

## Voice inclusion

Voice-to-voice ships inside the same $20. The path is the **CPU cascade
only**.

| Stage | Piece | Share of the ~1 vCPU average |
|---|---|---|
| VAD | Silero | 0.15 together with SER |
| ASR | Parakeet | 0.40 |
| SER | SenseVoice / emotion2vec | 0.15 together with VAD |
| TTS | Supertonic | 0.30 |
| Reserve | headroom up to the 1.0 average | 0.15 |

WeSpeaker rides in that same session budget. The split above is the sizing
figure for a concurrent session (ASR 0.4, TTS 0.3, VAD/SER 0.15, plus
reserve). It becomes a measured number only when the self-eval gate below
passes.

The crates are sherpa-onnx and the Rust Voice crates. When a clone runs,
Chatterbox Nano stays out of process on CPU. The compiler emits the
subscriber's own Galion file. The host runs that file: `.gq` and PCM. The
LLM pool keeps its own capacity reference. Voice crates stay on the Galion
format.

Live sectors keep the **80 ms** frame. Non-inference work on that tick stays
at or under **1%** of host CPU. Tick work at p99 stays far under **800 µs**
inside the frame (1% of 80 ms). A late tick pauses.

One concurrent voice-to-voice session is the steady case for Plus.

Galion Research console is its own hosted surface. It sits outside this SKU.

## Self-eval checklist

A row is done only when its gate passes and the result is written to
[`docs/galion-voice-v2/symphony/SELF-EVAL.md`](SELF-EVAL.md). Every row
below is open. This page is a plan. It is not a measurement.

| Gate | Pass when | Result |
|---|---|---|
| P-price | This page states Plus at $20/month and COGS at about $6, CX23-class, with a single Plus price. | open |
| P-quota | This page states 16 GB RAM and 100 GB SSD as the t284 quota. | open |
| P-cogs | The write-up holds ~$6 with a pooled ceiling. A dedicated 16 GB VM per subscriber fails this gate. | open |
| P-voice | Voice-to-voice is included in the $20, with a single price. | open |
| P-cpu | The live path is the CPU cascade: Silero, Parakeet, Supertonic, SenseVoice/emotion2vec, WeSpeaker. The SKU has no GPU. | open |
| P-session | One live session is budgeted at ~1 vCPU pooled and 0.65–1.15 GB RAM. The ~4 vCPU burst stays in the pool. | open |
| P-tick | The 80 ms frame holds non-inference work to ≤1% of host CPU, with tick p99 far under 800 µs. A late tick pauses. | open |
| P-format | Voice on Plus runs `.gq` and PCM. Voice crates keep the Galion format. Chatterbox stays out of process on CPU. | open |
| P-weights | Cascade weights are host-shared and stay off each 100 GB volume. | open |
| P-research | Galion Research console is named as its own hosted surface, outside Plus. | open |
| P-ledger | Each pass above is copied into `SELF-EVAL.md` with the host and the date. The Result column on this page stays open until then. | open |

— Maciej Grajczyk

---

© GOATSI — Galion Open Automated Technarchy Society Inc. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
