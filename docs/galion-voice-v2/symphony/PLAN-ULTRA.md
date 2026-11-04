# Ultra — symphony host plan

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb, 2026-11-03. Owner: Maciej Grajczyk. Source note: t284u.

The spelling is **Galion**. The product is Galion Voice.

This page is the Ultra SKU. It is a plan. It does not boot a machine, it does
not order one, and it does not record a benchmark. `LOCKS.toml` is not edited.

## 1. Price

**$220 per month.** That figure is closed. This page does not propose another
monthly price, a setup fee, or a discount.

The same owner note prices Plus at $20. Plus is not specified here. The $20
appears only so the disk conflict in section 5 can be read in the sentence
that produced it.

## 2. The computer

Ultra is one full cloud computer for one subscriber.

| Seat | What it is on this plan |
|---|---|
| **Galion OS** | The tenant operating environment. This repository does not ship the image. The image spec stays where the OS work already put it. |
| **Agents** | CPU agents on that computer. The voice-side seam is the Open Mind nerve and the Hermes gateway already described in `OPENMIND-NERVE-CORTEX.md`. Hermes stays off the 80 ms tick. |
| **Voice** | Galion Voice on the same computer: the V2 path, CPU, out of process for the heavy stages. |

Galion Research is a separate hosted console in the same owner note. Ultra
does not include a second machine for it.

The subscriber's computer is the CX43-class host below. It is not a slice of
the fleet voice pool. The fleet number in the CPU pack (about 806 concurrent
base-plan streams across app nodes) is a datacenter total. It is not what one
Ultra computer serves.

## 3. Host class

Owner name: **Titanaxe Max 3XL**. Class: **CX43**.

Published CX43 shape, read on 2026-11-03 from Hetzner's European cloud page
and from the Gen3 table that matches it:

| | CX43 class |
|---|---|
| vCPU | 8, shared |
| RAM | 16 GB |
| Local disk | 160 GB NVMe |
| GPU | none |
| CPU label on the public page | Intel or AMD (Gen3 cost-optimized shared vCPU) |

That shape is the class. A listing dated 26 October 2026 marked the Hetzner
SKU code CX43 unavailable, and the hardware note already in this tree says
the CX line was not orderable after the 15 June 2026 price change. HostCritiq
lists 20 TB included traffic for the Nuremberg and Helsinki CX43. Traffic
stays a listing detail. It is not an Ultra quota. Titanaxe Max 3XL means a
host of this class under our name. No Titanaxe Max 3XL spec sheet is in this
repository, so this page adds no Titanaxe numbers beyond the class row.

### CPU-first, EPYC and CX43

Two machines are in the CPU pack. They are not the same box.

**Fleet node.** 1U, one socket, AMD EPYC 9655P: 96 cores / 192 threads, on
the order of 614 GB/s memory bandwidth. The pack sizes an LLM/app node at
384 GB DDR5-6400 and an app/voice/research node at 768 GB. Air cooling.
Ethernet. No GPU. Those figures are the pack's sizing, and the pack says
they must be benchmarked before anyone treats them as a result.

**Ultra tenant.** One CX43-class computer: 8 shared vCPU, 16 GB RAM, 160 GB
NVMe, no GPU. The public CX43 label is shared Intel or AMD. The 96-core
EPYC 9655P stays the fleet node. If Titanaxe later places the guest on EPYC
capacity, the guest still has the CX43 shape.

The code path is the same either way. CPU only. No GPU kernel, no GPU
assumption, in the fleet node or in the tenant. Voice weights stay in
Galion `.gq` or the formats this tree already allows. A capacity figure
that was computed on a foreign container is a reference in the CPU pack.
It is not imported into Voice.

## 4. What the computer spends

Voice is the light part. The CPU pack's indicative session, which it says
must be benchmarked:

- about 1.0 vCPU average (ASR 0.4, TTS 0.3, VAD/SER 0.15, plus reserve)
- a burst near 4 vCPU
- about 0.65–1.15 GB RAM for the session

On an 8 vCPU / 16 GB computer a 4 vCPU burst is half the machine. Ultra is
one person's computer, so the design is one live voice plus the OS and the
agents, with headroom. It is not a density target.

The clock is the one this tree already has: 80 ms frame, 60 ms compute
budget inside the frame. The pack adds a bar for non-inference work: at most
1% of the host CPU, tick work p99 well under 800 µs, and a late tick pauses.
Heavy speech stays off `Heartbeat::step`, as the phone path already does.

The pack's datacenter cascade (Silero, Parakeet, Supertonic, and the rest of
that list) is how that session was sized. This tree's phone path is letter A:
Vosk out of process, Piper A21 out of process, canned reply until a student
is ready. Sizing numbers travel. Model names on the phone path do not change
on this page.

Nothing in section 4 was measured for this plan.

## 5. The 2 GB disk

t284, verbatim on Ultra storage: **Ultra $220 / 2GB SSD**.

That string is recorded as written. It is not adopted as the workspace.

It conflicts with the host class.

- The published CX43 local disk is **160 GB** NVMe. 2 GB is one eightieth of that drive (160 / 2).
- The same t284 sentence gives Plus **100 GB SSD** and **16 GB RAM** at $20. Ultra's only storage figure is then one fiftieth of the cheaper tier's disk (100 / 2), on a plan that costs eleven times as much ($220 / $20). 16 GB is also the CX43 RAM figure, which makes the two lines look swapped or truncated. This page does not repair them by swapping fields.
- A full computer holds an OS root, an agent home, voice weights, and the user's files. The pack's voice session is already 0.65–1.15 GB of **RAM**. A 2 GB SSD is that order of magnitude, as a disk. The letter-A pack in this tree is about 40 MB for `vosk-model-small-en-us-0.15`, and about 128 MB for the larger graph. Those files are the small end of the voice path. They are not an OS, and they are not an agent workspace.

Three readings, none chosen:

1. **Scratch or cache.** "2GB" is a quota inside the class disk (a tmpfs, a download cap, a voice scratch). The workspace is the rest of the drive. The owner still has to name that quota.
2. **Bad copy.** The figure was truncated or attached to the wrong tier. The Plus line in the same sentence already carries a 100 GB disk and a 16 GB memory number.
3. **Literal workspace.** The persistent disk is 2 GB. That reading cannot carry "full cloud computer" and cannot carry CX43-class. It is rejected as the product sentence. It stays in the record so nobody silently deletes it.

This page does not replace 2 GB with 160 GB, with 100 GB, or with any other
size. Publishing the class drive as the quota would be a new spec. The
monthly price stays $220 in every reading.

### Clarifying note (proposed, not adopted)

> Ultra stays **$220 per month**. The host stays **CX43-class / Titanaxe Max 3XL** (8 shared vCPU, 16 GB RAM, 160 GB NVMe, no GPU). t284's **2GB SSD** is on the record and is not the workspace quota. It conflicts with the class disk and with the 100 GB SSD on the Plus line of the same note. Confirm whether 2 GB is a scratch or cache inside the class disk, or a bad copy. Until that confirmation, the quota is unnamed. Do not change the monthly price. Do not print 2 GB, 160 GB, or any other size as a decided Ultra SSD.

## 6. Closed on this page

- The monthly price is $220.
- The workspace quota is unnamed. Section 5 is the record of why.
- `LOCKS.toml` stays as it is, including `paid_tiers`. A licence change is a later owner edit, with counsel, and a LEDGER line. The one-dot page shows no price.
- Ultra is a plan. The existing Titanaxe address `51.38.137.193` is the V1 deploy target in `docs/deploy/TITANAXE-VPS.md`. It is not identified here as a Max 3XL. Nothing connected.
- The path stays CPU-only, on the formats this tree already allows. Letter A stays Vosk.
- Token rate, voice latency, and COGS are absent. The pack's token rates and its Plus COGS note are left in the pack.

## 7. Gates

Scored in `SELF-EVAL.md`. The document gates pass on this text. The workspace
quota stays open. No host was booted.

## Sources

CPU pack (owner, t284u, 2026-11-03): the symphony CPU foundation. Fleet node
EPYC 9655P, voice-session vCPU and RAM, 80 ms / 1% bar, t284 strings
"Ultra $220 / 2GB SSD" and "Plus $20 / 100GB SSD + 16GB RAM".

CX43 shape:

- [Hetzner European cloud](https://www.hetzner.com/de/european-cloud) — CX43: 8 vCPU, 16 GB RAM, 160 GB drive.
- [CX Gen3 / CPX Gen2 table](https://cloudfleet.ai/blog/partner-news/2025-10-hetzner-cloud-introduces-new-shared-vcpu-server-families-cx-gen3-and-cpx-gen2/) — CX43: 8 vCPU, 16 GB, 160 GB, Intel/AMD, cost-optimized shared.
- [compute.info CX43](https://compute.info/vm/hetzner-cloud/cx43/) — 8 shared vCPU, 16 GiB, 160 GB NVMe, no GPU.
- [HostCritiq CX43 (Nuremberg and Helsinki)](https://hostcritiq.net/providers/hetzner/plans/cx43-eu) — same shape, 20 TB listed traffic; marked currently unavailable as of 26 October 2026.
- `docs/research/galion-voice-v2/domains/10-hardware-edge.md` — CX line not orderable after 15 June 2026. That page's prices are infra notes. They are not the Ultra price.

— Maciej Grajczyk

---

© GOATSI — Galion Open Automated Technarchy Society Inc. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
