# 02 — Core scheduling & CPU pinning

> **OWNER RULING 2026-11-04 20:45 CET (in force; supersedes everything below that names Qwen, Llama, llama.cpp, Groq or a cloud LLM).**
> No Qwen anywhere in the architecture, no fallback to it. Qwen, Groq and any cloud LLM are third party.
> The reasoning sector is fully local and in-house: the S0 student (~0.5B, `.gq`, W8A8) on `crates/galion-tensor`, with `CannedLlm` until trained weights exist.
> The S0 teacher / training source is an **open owner decision** (no third-party model is assumed).
>
> | core | role | threads |
> |---|---|---|
> | 0 | bus / heartbeat / audio I/O / VAD-endpoint / orchestrator + low-priority context/retrieval worker | 1 |
> | 1 | Vosk ASR | 1 |
> | 2-5 | in-house reasoning (S0 on galion-tensor; CannedLlm until weights) | 4 |
> | 6 | Piper Samantha | 1 |
> | 7 | Piper Jarvis | 1 |
> | — | clone workers | none during a live call |
>
> In code: `swarm/core-map-v3` (PR #68), `CoreRole::{Bus, Retrieval}` 0, `Asr` 1, `Reasoning` 2-5, `PiperSamantha` 6, `PiperJarvis` 7.
> Qwen / Llama / llama.cpp / Groq numbers below are **historical / superseded** measurements, kept for the record only.


Agent 02. Branch `swarm/lat240-core-pinning` (pushed, no PR):

| commit | what it does |
|---|---|
| `5f6a13a` | ClonePlan session/split4/split3, pinned in Rust with `sched_setaffinity`, thread pools sized to the pinned cores |
| `461f862` | ClonePlan::Local (superseded) |
| `7625657` | V2 live core map plus clone plan `off` |

Host: 8 cores, no SMT, 15 GB RAM. The box was shared and noisy (load 8–22; see Blockers).

## Historical core split (V2 fully local, owner ruling 19:40) — superseded by the 20:45 map above

This is the Symphony map. Agent 05 gets its 4 LLM cores.

| core | role | threads | in code |
|---|---|---|---|
| 0 | heartbeat / bus / call loop | 1 | `core_map::CoreRole::Bus` |
| 1 | Vosk ASR | 1 | `CoreRole::Asr` |
| 2-5 | local LLM (historical: Qwen2.5-1.5B Q4_K_M, llama.cpp `-t 4`; now in-house S0) | 4 | `CoreRole::Llm` (now `Reasoning`) |
| 6 | Piper TTS (now: Samantha only; Jarvis on 7) | 1 | `CoreRole::Piper` (now `PiperSamantha`); `PiperWorker::spawn` pins itself |
| 7 | retrieval (now: retrieval moved to core 0; 7 = Piper Jarvis) | 1 | `CoreRole::Retrieval` (now core 0) |
| — | clone workers | 0 during a call | `ClonePlan::Off`: spawn refuses and the stack falls back to Piper |

Clone voices are offline/batch only. They use `split3` (Samantha 0-2, Jarvis 3-5, 3 threads; 6-7 left for Piper/ASR) or `split4` (0-3 / 4-7, 4 threads).

Env knobs:

| knob | values / effect |
|---|---|
| `GALION_CORE_MAP` | `v2` / `off` / `auto`. `auto` is on when `GALION_LLM_LOCAL` is set and the host has ≥ 8 cores. |
| `GALION_CORES_<ROLE>` | per-role core override (BUS, ASR, LLM, PIPER, RETRIEVAL) |
| `GALION_CLONE_CPU_PLAN` | `session` / `split4` / `split3` / `off` (alias `live`) / `auto`. `auto` gives `off` under a local LLM, `split4` on ≥ 8 cores, `session` otherwise. |
| `GALION_CLONE_CPUS_<PERSONA>` | per-persona clone core override |
| `GALION_CLONE_THREADS` | clone thread count, clamped to the pinned cores |

`OMP_NUM_THREADS`, `MKL_NUM_THREADS`, `OPENBLAS_NUM_THREADS` and torch threads are all set from one value, which never exceeds the pinned cores. The python worker enforces the same clamp (`min(asked, len(sched_getaffinity))`). The old 1-thread clamp is kept only for `session`.

## Measured: clone synth, scripted dual-voice load (not the live call)

Load: 8 real live clauses × 2 reps per voice, Chatterbox nano (`nano_live_worker.py`), run through `lat240-pin-bench` (bench-02, not shipped) under `flock BENCH.lock`. "First clause" is the p50 of clauses 0 and 3, the short reply openers.

| config | n | RTF agg | synth p50 ms | synth p95 ms | max ms | first clause p50 ms |
|---|---|---|---|---|---|---|
| BEFORE live call (call-live clause_ready, Groq) | 48 | 2.29 | 3706 | 9040 | 12189 | ~3259 (endpoint→first clause) |
| BEFORE offline gen-log, 1 voice, quiet | – | 0.575 | 2235 | 2804 | – | – |
| solo8 (1 voice, 8 threads, unpinned) | 16 | 0.76 | 1805 | 2971 | 3442 | 1470 |
| dual8x8 unpinned (the live default) | 10 | **52.6** | 99343 | 156419 | 156419 | 95405 (killed) |
| dual4x4 unpinned | 32 | 1.27 | 3031 | 5253 | 6521 | 1926 |
| **split4** 0-3 / 4-7, 4 threads | 32 | 0.96 | 1867 | 2806 | 17157* | **1283** |
| **split3** 0-2 / 3-5, 3 threads | 32 | **0.87** | 2002 | 3201 | 4493 | 1339 |
| split4 + Piper loop unpinned | 32 | 2.92 | 5157 | 17929 | 23130 | 4427 |
| split3 + Piper loop pinned 6-7 | 32 | 1.49 | 3493 | 5320 | 6846 | 2246 |

\* One outlier with RTF 6.1, caused by an unlocked external job.

Clone plus a local LLM (Llama-3.2-1B Q4_K_M, llama-bench, 4 threads):

| config | clone RTF | clone p50 / p95 ms | LLM pp32 tok/s median (min) | LLM tg16 tok/s median (min) |
|---|---|---|---|---|
| LLM alone, 4-7 | – | – | 392 | 32.6 |
| clones 8+8 + LLM, all unpinned | **103.6** | 99412 / 105735 | 190 (13) | 13.5 (**0.8**) |
| clones 0-1 / 2-3 at 2 threads, LLM 4-7 | 1.37 | 3266 / 5017 | 307 (22) | 24.9 (2.9) |
| clones share 0-3 at 2 threads each, LLM 4-7 | 1.24 | 3079 / 4350 | 320 (234) | 23.6 (15.3) |
| clones share 0-3 at 4 threads each, LLM 4-7 | 33.2 (thrash) | 66443 / 83659 | 289 (20) | 19.6 (0.7) |

What this shows:

- Unpinned oversubscription causes a 50–100× slowdown. This is the live call's failure mode.
- The thread == pinned-cores rule removes that slowdown.
- With the cores split between clones and the LLM, clones only reach RTF 1.2–1.4, and the LLM still loses 20–30 % of its decode speed.
- That is why clones are **off** during a live call.

## Measured: V2 core map (LLM + Piper, no clone)

Setup: `run-coremap.sh` → `out3/`.

- LLM (historical, third-party, superseded): Qwen2.5-1.5B Q4_K_M, llama-bench, `-t 4 -p 24 -n 32 -r 10`.
- Piper: back-to-back cold CLI runs of a short reply, wall ms.

Run window: 20:06–20:14 CET under BENCH.lock. Load was 2.3–5 for the first four configs and 17 during clone_intrusion. TTFT est = 24 prompt tokens / pp tok/s (estimate). Each config has n = 10 llama-bench reps.

| config | LLM pp24 avg tok/s (min) | TTFT est p50 / worst ms | LLM tg32 avg tok/s (min) | ms/token | Piper short-reply p50 / p95 / max ms (n) |
|---|---|---|---|---|---|
| quiet: LLM 2-5 alone | 262.9 (232) | 90 / 103 | 27.0 (26.3) | 37.0 | – |
| piper_quiet: Piper 6 alone | – | – | – | – | 560 / 659 / 746 (53) |
| **unpinned**: LLM + Piper loop, no affinity (BEFORE) | 223.8 (76.7) | 93 / **313** | 16.6 (13.4) | 60.2 | 578 / 684 / 895 (39) |
| **pinned**: LLM 2-5 + Piper 6, map v2 (AFTER) | 214.7 (115.2) | 108 / **208** | **19.0** (16.4) | 52.6 | 593 / 687 / 726 (32) |
| clone_intrusion: map v2 + one unpinned 8-thread clone | 30.5 (6.0) | **3134 / 3978** | **1.3** (0.5) | 797 | 832 / 1007 / 1408 (457 runs) |

What this shows:

- **Pinning trims the tail, not the median.** Worst-case TTFT fell from 313 to 208 ms (−33 %). The pp minimum went from 77 to 115 tok/s. Decode improved +14 % (16.6 → 19.0 tok/s, 60 → 53 ms/token). The pp p50 is within noise (93 vs 108 ms; n = 10). Piper is unchanged (it is single-threaded either way).
- **Some interference remains with every role on its own core.** Decode drops from 27.0 (alone) to 19.0. The likely cause is shared memory bandwidth and L3 (an estimate, not proven).
- **A clone in a live call is fatal.** One unpinned clone made TTFT ~3.1 s, decode 1.3 tok/s, and Piper +49 % (p50 832 ms). The clone itself ran at RTF 48–72. This hard-confirms the "no clone workers during a live call" ruling. `ClonePlan::Off` now enforces it whenever `GALION_LLM_LOCAL` is set.


## First clone audio

Offline, first-clause synth p50:

| config | p50 ms |
|---|---|
| solo, unpinned | 1470 |
| split4 | **1283** |
| split3 | 1339 |
| unpinned dual 8+8 (today's live default) | 95405 |

The live call's endpoint→first content clause was 3259 ms p50 (before).

In a V2 live call the first audio is **Piper**, not a clone. Clone audio is offline only, so it no longer sits on the 240 ms path.

## Production checklist (core scheduling & CPU pinning)

| # | item | status |
|---|---|---|
| 1 | Clone threads never exceed pinned cores (code-enforced and tested; python worker clamps too) | PASS |
| 2 | Clones pinned in Rust with `sched_setaffinity` in pre_exec (no taskset, no python in shipped path) | PASS |
| 3 | OMP / MKL / OPENBLAS / torch threads set from one value | PASS |
| 4 | 1-thread clamp reconciled (`session` only; split plans get their pinned cores) | PASS |
| 5 | Dual-clone offline RTF < 1 measured (split3 0.87, split4 0.96) | PASS |
| 6 | Dual-clone RTF back to 0.55 | FAIL (best dual 0.87; solo 0.76 on this noisy box) |
| 7 | V2 core map in code (`speech::core_map`, roles 0 / 1 / 2-5 / 6 / 7, env switch, tests) | PASS |
| 8 | Piper pinned to core 6 when the map is active | PASS |
| 9 | Clone off during live calls (`auto` + local LLM → `off`, spawn refuses, Piper fallback) | PASS |
| 10 | Local LLM launched on cores 2-5 at 4 threads (now: in-house S0, `galion-pin reasoning`) | FAIL (historical): placement API is ready (`core_map::placement(Llm)`), but agent 05's launcher must call it |
| 11 | Vosk ASR pinned to core 1 | FAIL: ASR lives in `galion-v2-asr` / `galion-v2-live`, outside this crate |
| 12 | Bus / heartbeat pinned to core 0 | FAIL: live driver `galion-v2-live` is not in git and is owned by the live agent |
| 13 | Retrieval pinned to core 7 (now core 0, low priority) | FAIL: no retrieval process exists yet |
| 14 | Pinned map measured against unpinned under LLM + Piper load (worst TTFT 313→208 ms, tg +14 %) | PASS |
| 15 | Verified on a real live call | FAIL: not run (out of scope; no live call allowed) |
| 16 | Crate tests green under default, `GALION_LLM_LOCAL=1` and `GALION_CLONE_CPU_PLAN=session` (65/65 each) | PASS |

**Completion: 10 / 16 PASS = 62.5 %**

## Blockers / caveats

- **Noisy box.** BENCH.lock is held in a long queue, and some other agents' builds and benches ran without the lock (load 8–22). Absolute ms are pessimistic, but the ratios between configs are what matter.
- **`galion-v2-live` is not in git.** The live agent's clone.rs already pins Samantha 0-3 / Jarvis 4-7 at 4 threads with taskset. Under V2 it should spawn no clone at all and adopt `core_map`. I left that driver untouched.
- **Persona TOML** still says `threads=1`. The env/plan now overrides it, so the TOML is cosmetic.
- **The Rust core map covers Piper only.** The LLM, ASR, bus and retrieval launchers need one call each (`core_map::apply_affinity(&mut cmd, &placement(role).cpus)`) from their owners.
