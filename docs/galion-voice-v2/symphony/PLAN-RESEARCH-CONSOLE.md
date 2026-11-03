# Galion Research console

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-03. Symphony plan. The console is not built here.
`LOCKS.toml` is not edited.

The spelling is **Galion**.

## 1. Where this product sits

Galion Research is a product in the lineup with Galion One, Obsidian Open Mind, and Galion Voice. The research product was Hermes Research. People who say "Hermes" mean this path. The name they meet is Galion Research.

| Product | What it is | Surface |
|---|---|---|
| **Galion One** | Companion voice | Speaks on Galion Voice |
| **Obsidian Open Mind** | Rust agent stack for a cheap CPU-only machine. Nerve, incident cortex, `galion-one` | Its own repo. Loopback nerve `127.0.0.1:8670` when a human has deployed it. Observation. It does not answer a research question |
| **Galion Voice** | The voice product | One dot. `crates/galion-console/assets/v1-screen.html` |
| **Galion Research** | The research product (ex-Hermes Research) | This console |

Hermes stays the access layer behind the product (`crates/galion-sideband`, V2-S15, [`../OPENMIND-NERVE-CORTEX.md`](../OPENMIND-NERVE-CORTEX.md)). The console is the desk. The gateway process is not the desk. The speaking voice, when a session is a voice session, is [`../personas/GALION-RESEARCH.md`](../personas/GALION-RESEARCH.md). That persona's bus id `galion-research` is still a candidate. This plan does not pretend a `Persona` message already lists it.

Galion Voice keeps the slogan: "The seamless connection with the human being, on the voice only." The screen for that product is one dot. Galion Research is the desk where a lookup that outlives the voice turn is still visible.

## 2. Console surface

One document. A title line, **Galion Research**. Under it, the queue. Under the selected row, the open job. One ask field. Enter enqueues.

The desk has three regions and nothing else.

1. **Queue.** One row per job: age, one state word, the question in one line.
2. **Open job.** The answer first, then the limit, then stop. Three piles, in that order: Found, Inferred, Unknown. A source is a short name the host already handed over (author, title, or the system that returned it). An empty source list stays empty.
3. **Ask.** One field. It enqueues a job. It does not open a voice screen, a model picker, or a persona costume. Samantha and Jarvis stay internal clone tests. They are not console modes.

State words, and only these:

| State | When the desk shows it |
|---|---|
| `Queued` | The research host accepted the job. It is waiting on the async queue. |
| `Running` | A worker holds it. |
| `Returned` | `FromHermes::Knowledge` came back. The text and the optional source are the row. |
| `Timeout` | The job's own deadline passed (plan default below). |
| `Unreachable` | Nothing is listening. Today that is `NullTransport`. |
| `NotFound` | The gateway answered and had nothing usable. |
| `Denied` | The lookup is not allowed. |

Those last four match `NoKnowledgeReason` in `crates/galion-sideband`. The desk says them in the persona's ordinary words when a voice session speaks the same outcome. The desk itself uses the state word, so a row can be scanned.

What the page is made of: text. One list, one open job, one field.

The voice page stays the voice page. `v1-screen.html` is the one dot: a vector ellipse, fill `#F5F5F7`, amber-to-violet wash on the background, Luti on the radius and the brightness. The Research console is a different product surface, so it has its own process and its own assets. It is not a route on the voice server, and it is not registered in the HTML that `galion-console` embeds for `/`. `galion-console` remains the developer tool (`LOCKS.toml` `[ui] console_is_user_ui` is false). Promoting that tool into a user page would be a Voice UI. This console is not that promotion.

No port is chosen here. Choosing one on `:7878` would invite the desk to hang off the dot server. A later binary binds its own loopback address. ngrok, Caddy, and Titanaxe access codes stay the human edge work already written in Plan V5. This plan does not open SSH and does not add a second tunnel.

## 3. How the desk meets the one dot

Voice has one user interface: the dot. A person speaks, the page records, the turn worker runs. There is no second AI panel on that page, no menu of models, and no research drawer beside the mark. `[ui]` already says so: one dot, `chrome_allowed` false, chrome meaning menus, buttons, text, orbs, faces, waveforms, logos. This plan leaves that lock as it stands.

The two products meet at a turn boundary, and only there.

```text
Galion Voice                              Galion Research
one dot                                   own process, own page
Heartbeat::step  (80 ms)                  stays outside this call
        │
        │  turn boundary only
        ▼
CortexHandoff::offer
  try_send on the voice queue
  depth 8 in CortexHandoff::local()
        │
        ├── QueueFull → drop, voice continues, desk gains no fake Running row
        ▼
hermes-sideband thread
  exchange may block; it is off the tick
        │
        ├── poll ≤ 3 s (RETRIEVAL_CAP)
        │     inside the cap, Knowledge can be spoken
        │     past the cap, voice Timeout, local knowledge, labelled local
        │
        └── the same job may still be Running on the desk
              Returned lands on the desk
              a later voice turn may speak it, once the host has handed it back
```

The voice cap and the desk deadline are two clocks.

- Voice clock: `galion_sideband::RETRIEVAL_CAP`, 3 000 ms. `poll` past the cap returns `NoKnowledge { reason: Timeout }` and drops a late reply for that turn. The persona says the lookup did not come back, then speaks what it already holds, marked local. The heartbeat is not waiting.
- Desk clock: the job keeps `Running` after the voice has moved on. The desk's own deadline is a plan default of **30 s** from accept. Thirty seconds covers the prefill examples in §5 (8 s nominal, 10 s at the conservative rate, on 8 000 source tokens) and ends a stuck worker. It is a product default. It is not a hardware measurement and it is not a lock. Past 30 s the row becomes `Timeout` and leaves the session queue.

A voice `Timeout` at 3 s does not rewrite a desk row that is still `Running`. A late `Returned` does not rewind the turn the voice already spoke. The next turn may say the correction first, which is the persona's rule for a finding that arrives after an earlier one.

Money stays refused. `Intent::voice_may_request` is false for `ActionTier::Money`. `offer` records `RefusedMoney`. The desk has no purchase control that would route around that.

Affect on the side band is whatever the caller was already allowed to send. `sense` defaults deny. The desk does not draw valence, arousal, or a mood. Empathy for a voice session stays the live OKLCH weight on the voice, in [`../EMPATHY.md`](../EMPATHY.md). The desk does not grow a meter.

A nerve line, when a human runs `galion-nerve`, stays observation: `voice.frame_miss`, `voice.backend_up`, `voice.fallback`. No PCM, no transcript, and no job text on that line. Research text goes to Hermes on the side band, and only when the host has already decided to send the turn text.

Today the transport is a trait. `CortexHandoff::local` uses `NullTransport`, and every lookup comes back `Unreachable`. This landing adds no client, no URL, and no key. V2-S15 stands: the approval record is not a call. Until a human wires `HermesTransport` on the side thread, the desk's far-side state is `Unreachable`, and local knowledge stays labelled local.

## 4. Research activity is the async queue

Research is the async side band. It is off the 80 ms heartbeat.

`Heartbeat::step` advances one 80 ms frame. It does not take locks, channels, or sleeps, and it does not allocate. The frame budget inside that call stays 60 ms (`[clock] frame_budget_p95_ms`). Non-inference work on the tick stays inside the one-percent rule from the CPU foundation: ≤1% of host CPU, tick work p99 well under 800 µs, and a late frame pauses. A console poll inside `step` would spend that budget. The call site for research is the turn boundary, after `step` returns.

The code already encodes the split.

| Witness | What it holds |
|---|---|
| `crates/galion-heartbeat` | `step` never blocks and never allocates |
| `crates/galion-sideband` `SidebandClient` | `submit` is `try_send`. `poll` is `try_recv`. The worker thread is named `hermes-sideband`. A full queue is `SubmitError::QueueFull`. The hot path does not wait |
| `crates/galion-openmind` `CortexHandoff::offer` | Turn, intent, and affect only. Documented as forbidden inside `Heartbeat::step` |
| `CortexHandoff::local` | Queue depth **8**, `NullTransport` |
| `tests/handoff.rs` `offer_never_blocks` | Six offers against a transport that sleeps 200 ms, queue depth 1, finish in under 80 ms |
| `tests/client.rs` `submit_and_poll_never_block` | Five submit/poll pairs on a slow transport finish in under 80 ms, and a further submit returns `QueueFull` |

Re-run on 2026-11-03 with rustc 1.85.0 (the crates declare `rust-version = "1.85"`; the image default 1.83.0 refuses them):

```text
cargo +1.85.0 test -p galion-openmind --test handoff
# 4 passed, 0 failed, finished in 0.20s
cargo +1.85.0 test -p galion-sideband --test client
# 3 passed, 0 failed, finished in 0.20s
```

`offer_never_blocks` measures the handoff call. It is not a measurement of `Heartbeat::step`. The plan's rule is the stronger one: `offer` is not placed inside `step` at all.

Plan defaults for the two queues. The first two rows are the code. The last two are defaults for the desk, owner-tunable, and they do not belong in `LOCKS.toml`.

| | Value | Standing |
|---|---|---|
| Voice-facing queue depth | 8 | `CortexHandoff::local` |
| Voice retrieval cap | 3 s | `RETRIEVAL_CAP` |
| Desk job deadline | 30 s | Plan default. Covers the §5 prefill examples. Ends a stuck worker |
| Desk queue depth | 32 jobs per session | Plan default. A full desk queue refuses the new ask the way `QueueFull` does. The row is not invented. Backpressure is the point; the residual RAM in §5 is large enough that RAM is not what this cap protects |

A direct ask from the desk enters the 32-job queue and does not touch the voice channel. A voice offer enters the depth-8 channel only. The desk mirrors that offer for as long as the side band still holds it. A `QueueFull` drop is not mirrored, and it never appears as `Running`.

## 5. CPU and RAM — Research activity

### Source

Primary hardware note, other project, not in this checkout:

`/workspace/split_wte/galion_datacenter_report/agent1_hardware/HARDWARE.md`

What landed with this task is the t284u summary of that file (2026-11-03, Europe/Zagreb), plus the integration note and the voice-cascade reference named beside it. The summary does not print a table titled "Research activity". The owner brief for this plan names Research activity as the higher-tok/s work on an async queue. This section is that name, filled only with figures the summary printed, plus arithmetic on those figures. The summary marks the token rates indicative. They stay indicative until a benchmark is written down. GGUF numbers in the source were a capacity reference for sizing. Voice crates do not import GGUF. The symphony format is `.gq`.

No GPU in this plan. The flagship CPU is one-socket **AMD EPYC 9655P**, 96 cores / 192 threads, about 614 GB/s memory bandwidth. The LLM node carries **384 GB** DDR5-6400. The **App/Voice/Research node** is the same CPU with **768 GB** DDR5. Air cooling, leaf-spine Ethernet.

### The published band

Per LLM node, at 70% utilisation:

| Point | Decode | Prefill |
|---|---|---|
| Conservative | 200 tok/s | 800 tok/s |
| Nominal | 250 tok/s | 1 000 tok/s |
| Optimistic | 300 tok/s | 1 300 tok/s |

Voice session, same summary, on the app nodes:

| | Budget |
|---|---|
| Average | ~1.0 vCPU concurrent (ASR 0.4, TTS 0.3, VAD/SER 0.15, plus reserve) |
| Burst | ~4 vCPU |
| RAM | 0.65–1.15 GB pooled per session |
| Fleet peak in the base plan | 806 streams → ~806 vCPU and ~190 GB across ~5 app nodes |

Voice is the CPU-light cascade. It is not a tok/s path, and the summary says it is not the bottleneck. The heartbeat's non-inference share stays ≤1% of the host, with tick work p99 well under 800 µs.

### Higher tok/s means the optimistic end of this band

A live voice turn cannot batch. It has an 80 ms frame and a 3 s retrieval cap, and it must keep speaking. Research activity can batch, because the desk is the async queue and the voice has already continued. Batching is what moves the same CPU from a latency-held decode toward the high end of the published band.

So the research queue's planning rates, per node, at the same 70% utilisation, are:

| While | Decode | Prefill | Why this row |
|---|---|---|---|
| Until a benchmark exists | 250 tok/s | 1 000 tok/s | Nominal. Capacity planning uses this |
| The queue is free to batch | 300 tok/s | 1 300 tok/s | Optimistic. This is the "higher tok/s" ceiling |
| The node is also serving voice at plan peak | 200 tok/s | 800 tok/s | Conservative floor. See the residual below |

These are not a fifth measured rate. A product claim waits on a CPU run written into the ledger, with the host and the `.gq` id. Until then the ceiling is a ceiling.

Weights sit once on the node. Decode on that CPU is memory-bandwidth bound, which is why the LLM node is specified at 384 GB of DDR5-6400 with the channels filled. This plan does not publish bytes of KV per token. The summary does not. Per-job RAM stays "the residual, after the voice pool, with weights resident once" until a measurement replaces it.

### Where the higher rate is allowed to run

Arithmetic on the base-plan peak (806 voice streams, ~190 GB, ~5 app nodes, 192 threads, 768 GB):

| | Figure |
|---|---|
| Voice vCPU per app node, average | 806 / 5 = **161.2** |
| Share of 192 threads | 161.2 / 192 = **84%** |
| Threads left on that node | 192 − 161.2 = **30.8** |
| 70% of 192 threads | **134.4** |
| Voice RAM per app node | 190 / 5 = **38 GB** |
| RAM left on the 768 GB node | 768 − 38 = **730 GB** |

At that peak the app node is already past the 70% sizing point on threads. The leftover ~31 threads are the one-percent engine, the OS, and a thin desk control plane (the list, the ask field, the state words). They are not the 250 tok/s pool. RAM on the same node is the opposite shape: the voice pool is about 38 GB, and hundreds of gigabytes remain. Text rows fit. Token decode does not fit beside the voice peak.

The rule:

1. Voice reserve is scheduled first. A node that is carrying the 806-stream mix keeps ~1.0 vCPU and up to ~1.15 GB per session, and keeps the burst of ~4 vCPU as a claim that can land. Research workers run on what is left.
2. The higher tok/s queue runs where the 70% band is actually free. That is an LLM node (384 GB, the machine the band was sized on) or an App/Voice/Research node that is not at the voice peak. The desk process can sit on the app node. The worker that spends the token rate drains the async queue on the free band. Leaf-spine Ethernet is the hop between them. The hop is inside `exchange`, on `hermes-sideband`, and the voice path only `try_send`s.
3. A research session is not sized as 768 GB, and it is not sized as a voice session. One voice session stays 0.65–1.15 GB. One research job adds KV on the residual of whichever node holds the weights, shared with the other jobs in the queue. No per-session claim on the whole node.

Workspace disks in the freemium note (Plus at 100 GB SSD and 16 GB RAM, Ultra at 2 GB SSD) belong to those SKU plans. They are not this queue's RAM. `[licence] paid_tiers` stays false. This plan describes a surface and a compute budget. It does not create a price.

### Why the desk exists, in one prefill

Take 8 000 tokens of source, which is an illustration on the published rates, not a measured job:

| Rate | Wall time for that prefill | Against the voice cap of 3 s |
|---|---|---|
| 800 tok/s conservative | 10.0 s | Voice has already returned `Timeout` |
| 1 000 tok/s nominal | 8.0 s | Same |
| 1 300 tok/s optimistic | 6.2 s | Same |

A 300-token answer at the nominal 250 tok/s decode is 1.2 s, which fits the voice cap once prefill is already done. The work that misses the cap is cold prefill and queue wait. That work is the desk. The voice says the lookup did not come back. The row stays `Running` until `Returned` or the 30 s deadline.

## 6. What a later implementation has to satisfy

This landing is the plan and the gate record. A console binary is later, and it is done when all of these are true.

1. `v1-screen.html` gains no research list, no ask field, and no second mark.
2. The desk is its own process and its own assets. It is not embedded in `SCREEN_HTML` and it is not a new route on the voice port.
3. Submitting a desk job does not call `Heartbeat::step`. A voice `offer` stays at the turn boundary and still satisfies `offer_never_blocks`.
4. `QueueFull` on the voice queue drops the offer. The voice continues. The desk does not show `Running` for a drop.
5. The seven state words are the whole status vocabulary. `Unreachable` is the state when the transport is null.
6. No PCM and no affect sample on the job. Turn text only, and only after the host has decided to send it.
7. No GGUF in the Voice crates, no GPU assumption, no edit to `LOCKS.toml`.
8. A measured tok/s, when one exists, is reported against the 200/800, 250/1000, and 300/1300 band. The band stands until that measurement is in the ledger.

## 7. Self-eval

The gate record is [`SELF-EVAL.md`](SELF-EVAL.md), section RC. The plan section is complete when RC-1 through RC-4 pass. RC-5 and RC-6 stay open: there is no measured research tok/s, and there is no console process in this landing.

— Maciej Grajczyk
