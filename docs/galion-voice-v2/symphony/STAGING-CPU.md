# Staging CPU cascade — Titanaxe

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb, 2026-11-03.

**Status: plan.** Titanaxe stays as it is. Nothing in this landing opened a
socket to `51.38.137.193`. `LOCKS.toml` is unchanged, and `paid_tiers` stays
false. This page sizes the CPU cascade. It is not a storefront and not a
release.

The operational track is still [Plan V5](../../ops/PLAN-V5.md) and the
[Titanaxe note](../../deploy/TITANAXE-VPS.md). Those files own ports, the
edge, and the dry-run refusal. This file owns how many voice sessions that
host may run.

## 1. Host

Staging, once a human has finished access setup, is Titanaxe
`51.38.137.193`. Plan V5 H1 is **approved-pending-setup**: per-user
anonymized access codes, installed by a human. This tree holds no code and
no key. An agent does not SSH, does not probe the address, and does not
read a key out of the environment to learn the plan size.

`51.38.0.0/16` is the range domain 10 reads as OVH. The invoice (vCPU, RAM,
disk) is not in this repository. Until a human writes those three numbers
into `LEDGER.md` from the provider console, staging uses the Plus cap in
section 5: **one voice session**.

"Titanaxe Max 3XL" in the CPU foundation is the owner's name for the Ultra
capacity class, the same class as CX43. It is not a second address.

## 2. Where the numbers come from

CPU foundation note, Maciej Grajczyk, 2026-11-03 (Europe/Zagreb), t284u. It
summarises the datacenter pack's `HARDWARE.md`. That file is not in this
repository. The voice figures below are that note. They are indicative until
the host is measured. The flagship in the same pack (1U AMD EPYC 9655P,
96 cores / 192 threads, 768 GB on an app/voice node, about 250 decode tok/s
at 70% util) sizes a later fleet. Staging is one shared-vCPU VPS.

The pack's decode rates were computed for a foreign weight container. Those
rates stay a capacity reference. Voice on this host stays `.gq` and PCM.
This plan does not import that container.

A late heartbeat frame is a pause. `Heartbeat::step` already treats `late`
that way: nothing new starts on that frame, and the next frame does not
catch up. The non-inference bar from the same foundation is ≤1% of host CPU,
with tick work p99 well under 800 µs inside the 80 ms frame. The in-tree
Rika step test already requires p95 under 800 µs on the machine that runs
it. That test is not a measurement of Titanaxe. The lock's compute budget
inside the frame stays 60 ms p95. Inference stays on the turn worker, off
that tick.

## 3. One voice session

| Slice | Budget |
|---|---|
| ASR | 0.40 vCPU |
| TTS | 0.30 vCPU |
| VAD + SER | 0.15 vCPU |
| Reserve (1.00 − 0.85) | 0.15 vCPU |
| **Average** | **1.0 vCPU** |
| Burst | ~4 vCPU |
| RAM, pooled | 0.65–1.15 GB |

The heartbeat is outside this 1.0. It is the ≤1% non-inference engine:
0.02 vCPU on a 2-vCPU host, 0.08 vCPU on an 8-vCPU host.

Hosted base-plan peak in the same note: about 806 voice streams, about
806 vCPU and 190 GB across about five app nodes. That is the fleet. It is
not this VPS.

## 4. Plus is CX23, Ultra is CX43

CX23 and CX43 are Hetzner cost-optimized shared-x86 shapes, used here as
the ruler for the two hosted classes. Domain 10 already records that the CX
line is limited and no longer something to order casually after the 15 June
2026 price change. This plan does not order a server. It sizes the box that
already has the address above.

| Class | Shape | vCPU | RAM | Disk | Traffic | Public price band |
|---|---|---|---|---|---|---|
| Plus | CX23 | 2 shared, Intel or AMD | 4 GB | 40 GB NVMe | 20 TB | €5.49 net monthly cap, product page from €5.99/month excl. VAT; about $6.49 list (checked 2 Nov 2026). Marked limited / not available. |
| Ultra | CX43 | 8 shared, x86-64 | 16 GB | 160 GB NVMe | 20 TB | €15.99 net monthly cap; €16.49 vendor monthly cap on 2026-11-02; about $18.49 list (checked 2 Nov 2026). No GPU. Marked limited. |

Sources for the shape and the price band:
[Hetzner cost-optimized](https://www.hetzner.com/cloud/cost-optimized/),
[Hetzner Cloud pricing 2026](https://cloudtally.eu/hetzner-cloud-prices),
[CX43 on compute.info](https://compute.info/vm/hetzner-cloud/cx43/) (collected 2026-11-02),
[Hetzner plan table](https://wheretodeploy.com/providers/hetzner) (checked 2 Nov 2026).
Domain 10's CX23–CX53 band of €5.49–€29.49 matches the CX23 and CX53 ends of
that table.

Owner lines from the same foundation, written beside the shapes. They are
capacity notes. They do not turn `paid_tiers` on.

| Class | Owner list (t284) | COGS note in the foundation | Entitlement line, as written |
|---|---|---|---|
| Local | free forever | the person's own Linux or a Raspberry Pi; it does not use this VPS | — |
| Plus | $20/mo, voice-to-voice included | CX23-class, about $6 | 100 GB SSD + 16 GB RAM |
| Ultra | $220/mo | CX43-class / Titanaxe Max 3XL | 2 GB SSD |

The Plus entitlement (16 GB RAM, 100 GB disk) is larger than a CX23 (4 GB,
40 GB). The Ultra line "2 GB SSD" is not the CX43 disk (160 GB). Voice
admission uses the machine shape in the table above. Reconciling those two
entitlement strings is an owner edit. An earlier Ultra figure of 590 € is
overridden by the foundation's $220 / CX43 line; this plan follows the
foundation.

The ~$6 COGS note matches the CX23 public band (about €5.49–€5.99, about
$6.49). The Ultra list of $220 is the owner's price note. The CX43 invoice
is the €15.99–€16.49 band, not $220.

`LOCKS.toml` `[licence]` stays `paid_tiers = false`. V2-S8 still holds: a
freemium split in the product is an owner edit of that table, plus counsel.
The dollars in this file exist so a class can be pointed at a machine.

Galion Research (the Hermes side band) is a separate hosted surface. It is
off the 80 ms path and outside the 1.0 vCPU.

## 5. How many sessions fit

Two caps, both derived here from section 3. The 70% factor is the one the
datacenter note uses for its decode sizing. The 1 GB subtract is a host
reserve for the OS and the edge, outside the voice slice.

- Quiet cap, no session above 1.0 vCPU:
  `min(floor(0.70 × vCPU / 1.0), floor((RAM_GB − 1) / 1.15))`.
- Burst-safe cap: one session at 4 vCPU, every other session at 1.0 vCPU,
  sum ≤ `0.70 × vCPU`.

| Class | Quiet | Burst-safe | RAM at the high end of the pool | What staging admits |
|---|---|---|---|---|
| Plus, CX23 | floor(1.4) = **1** (RAM alone would allow 2) | 4 > 1.4, so a full burst does not fit on the box | 1.15 GB inside 4 GB | **1 session.** The spike shares the 2 vCPU. A second session spends the whole average budget and leaves the edge and the heartbeat nothing. |
| Ultra, CX43 | floor(5.6) = **5** (RAM alone would allow 13) | 4 + 1.0 = 5.0 ≤ 5.6, so **2** | 5 × 1.15 = 5.75 GB inside 16 GB | **2 sessions** while a burst is assumed. **5** only after a measurement on that host shows the spike stays near 1.0 vCPU. |

Unknown Titanaxe, which is the state of `51.38.137.193` in this tree:
**Plus cap, 1 session.** The Ultra cap applies after the ledger line says
the box has at least 8 vCPU and 16 GB.

Thread pool, set by the human in that session, equals the host vCPU count:
2 on a CX23-class box, 8 on a CX43-class box. That is the
`init_thread_pool` comment in `galion-tensor`.

Memory cap for the voice slice, when the human writes the unit. The Titanaxe
note names a 256 MB cap. One session is budgeted at 0.65–1.15 GB, so 256 MB
is below the budget and the human raises it at install:

| Class | systemd `MemoryMax` | Why |
|---|---|---|
| Plus, and any unconfirmed Titanaxe | `1536M` | covers one session at 1.15 GB |
| Ultra, burst-safe (2) | `2560M` | 2 × 1.15 GB |
| Ultra, quiet five, after the measurement | `6144M` | 5 × 1.15 GB, still inside 16 GB |

The cap covers the screen process and the out-of-process ASR and TTS.
Caddy stays outside the slice. This plan does not add a unit file.
`deploy/galion-voice.service` is named in the Titanaxe note and is not in
the tree.

## 6. The cascade that runs

CPU only. The foundation's path (Silero VAD, Parakeet-class ASR, Supertonic
TTS, SenseVoice / emotion2vec SER, WeSpeaker, with sherpa-onnx as a
reference) is the sizing model behind the 1.0 vCPU. Staging runs the
processes that are already in this tree.

| Stage | On staging | Share of the 1.0 |
|---|---|---|
| VAD | The phone's AudioWorklet. A server Silero is the sizing model and is not started. | the 0.15 is reserved with SER |
| ASR | Vosk, letter A, out of process. `vosk-model-small-en-us-0.15` when that model is already on the machine. A missing model is `AsrUnavailable`. | 0.40 |
| Reply | `CannedLlm`. A `.gq` student is later. Its decode budget is the datacenter LLM pool, and it is not added silently on top of this 1.0. | inside the 0.15 reserve |
| TTS | Piper A21, out of process, else flite. Supertonic is a candidate name in the foundation and is not wired. | 0.30 |
| SER, speaker id | Not wired. Consent `sense` stays deny. | the 0.15 is reserved |
| Heartbeat | `Heartbeat::step`, in process. Late frame = pause. | ≤1% of the host, outside the 1.0 |

English, 16 kHz in, on the phone lane. The 80 ms path stays primary. The
cascade feature stays off the default build. sherpa-onnx stays a reference
(K16). A sidecar runtime is the open owner question O1 in the research
brief, and this install does not decide it. Chatterbox stays under its
existing sunset pin, out of process, and this plan does not launch it.

## 7. Order, when a key exists

Until Plan V5 H1–H4 are done, the running demo is the local screen. The
steps below are the CPU check added to the human's list. There is no
connection command in this file. `scripts/deploy/release-dryrun.sh` refuses
`--execute` (exit 2) and stays the checklist.

1. The human reads the provider console and appends vCPU, RAM, disk, and
   provider to `LEDGER.md`. Until that line exists, the cap is 1 session.
2. The dry-run is printed and saved. A key is not approval to publish. H4
   is still the owner's release approval.
3. The unit binds `127.0.0.1:7878`. Public ports stay 22, 80, and 443 as
   the Titanaxe note already lists them. Port 7878 stays closed on the
   firewall. Caddy is the later edge. ngrok, if a human still wants a phone
   URL, points at loopback on the machine that runs the screen.
4. Environment on that unit: `GALION_ASR=vosk` only when the small-en model
   is on that machine, `GALION_LLM=canned`, `GALION_SPEECH=auto`,
   `GALION_CORS_ORIGINS` set to the exact `https://` origin, edge login on.
   No password in git.
5. Thread pool and `MemoryMax` from section 5, for the class the ledger
   line named.
6. A session past the cap gets a busy answer from the turn worker. The
   refusal does not run inside `Heartbeat::step`.
7. From the host itself, loopback
   `scripts/check-phone-pipeline.sh --no-start`. Record steal, one
   session's RSS, and one session's vCPU. RSS above 1.15 GB, or a heartbeat
   tick p99 that reaches 800 µs, stops the trial. The ledger gets that
   result. This plan has no measurement to fill in ahead of time.
8. Rollback is the previous `/opt/galion` symlink, already printed by the
   dry-run. Practice it once before calling the host anything but staging.

## 8. Gate

The CPU foundation says a section is complete when its gate passes and the
pass is written in `docs/galion-voice-v2/symphony/SELF-EVAL.md`. This
section is the plan. The gate has not been run. No pass is recorded.

## 9. This landing

2026-11-03, Europe/Zagreb. Document only. No SSH. No socket to
`51.38.137.193`. `LOCKS.toml` not edited.

— Maciej Grajczyk

---

© GOATSI — Galion Open Automated Technarchy Society Inc. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
