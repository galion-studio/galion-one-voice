# 01: Streaming & TTS latency (Piper)

Agent 01. Stage budget (BUDGET.md): **TTS first audio 71.3 → 30 ms p50**; playback handoff 0 → 0.
Branch `swarm/lat240-streaming-tts` (worktree `wt-01-streaming-tts`). Owner directive t612u applies: keep Piper, no architecture change, no other TTS library. Tune it, keep it warm, fix integration bugs.
All numbers below are **measured** on this box under BENCH.lock, unless a line says *estimate*. The box is shared, so each run lists its 1-min load average. Raw data is in `01-data/*.ndjson`.

## Before / after (headline)

| metric | before p50 / p95 | after p50 / p95 | how |
|---|---|---|---|
| Cold TTFA (spawn → first PCM, 1st utterance) | 334.3 (baseline file); 415–472 re-measured at load 9 | **not on the call path any more**: the pre-warm runs before `ready`. Process cold cost is unchanged at 415–472 (C++ piper) | `runtime-matrix.ndjson` `cold_spawn_to_done_ms` |
| Warm TTFA, C++ piper, all cores, 1w / 4w / 8w | 25.9 / 42.3 / 74.2 p50 (baseline, quiet) | 20.1/22.2 · 37.8/44.0 · 70.8/77.1 (load 9.9), repeat 18.7/21.3 · 37.4/46.6 · 68.3/87.7 (load 8.9) | same binary; no regression from the changes |
| Warm TTFA, Piper models on ORT 1.31 t8, Level3 graph opt (**needs owner/LOCKS decision**) | n/a | 11.1/14.1 · 21.2/23.4 · 39.1/42.8 (load 9.2) | `01-piper-rt` prototype, same models, bit-identical PCM |
| Piper in-loop TTFA (24 real first clauses from `full-loop.ndjson`, each after a "Mm-hm." backchannel) | **71.3 / 188.5** (real full loop) | C++ piper replay: **59.7 / 148.4** (load 4.1). ORT-1.31 t4: **39.2 / 69.3** (load 4.4). ORT t2 pinned 6-7: 59.3 / 93.0 | `replay-loop.ndjson`. Replay is Piper-only (no concurrent ASR/LLM), so treat it as a proxy, not a full-loop rerun |

A full-loop rerun was not possible. `v2-latency` needs an OpenRouter key, which I do not have and did not look for. The local llama-server belongs to agent 05 and is busy with its benches.

## What changed (shipped, gate-clean, Rust)

1. **Streaming PCM** (`galion-ml-voice/src/speech/piper_worker.rs`): `synthesize_stream(text, on_pcm)` passes each s16 chunk to the caller as soon as it comes off Piper's stdout. It keeps an odd trailing byte for the next chunk. `synthesize()` delegates to it. `first_chunk_ms()` is now exposed, and cold/warm ms now measure time to the first chunk. A test checks that streamed PCM equals buffered PCM. Limit: Piper is VITS and emits a whole sentence at once, so the gain is in handoff (playback can start on the first `read()`, without waiting for the end marker), not sub-sentence audio.
2. **Pre-warm** (`speech/mod.rs`): `PREWARM_LINES` holds 3 lines of different lengths (1, 4 and 8 words). `prewarm()` speaks all of them and sets `prewarmed` only on success. The first ORT runs at a new input shape are slow, so warming several lengths matters.
3. **Optional core pinning** for the Piper child (`GALION_PIPER_CPUS=a-b`, `sched_setaffinity` in `pre_exec`). The default is off, and the data says **keep it off for the C++ piper**: it always starts 8 ORT threads, and pinned to 6-7 it gets 2× slower (4w 77.3/87.6 vs 37.4/46.6 at load 8.9). Under heavy load (~16) pinning to 4-7 helped (4w 299→167 p50), so pinning only pays when other processes are confined elsewhere.
4. **Integration bugs fixed in the live harness** `galion-v2-live` (outside git; backups `*.bak-01`):
   - `piper.rs collect_until_done` always idled a full 30 ms `recv_timeout` after the utterance finished. That is the 30.1–34.6 ms gap seen in every research run. It now breaks as soon as it is finished (8 ms grace, 5 ms otherwise).
   - `bin/call.rs` had **no pre-warm**, so the first live clause paid the cold spawn (~330–470 ms). It now speaks the 3 warm lines before the lane reports `ready`.
   - `p.say(..).unwrap()` panicked the whole TTS lane if Piper died. It now respawns and re-warms.
   - `cargo check` is clean. **This code is not in the repo**, so the owner of galion-v2-live has to accept it. I did not touch the live voice.galion.app deployment.
5. Eval tools (python, eval-only): `scripts/eval/piper_lat240_bench.py` (TTFA by word count, cold spawn, `--cpus`, `--replay` of real loop clauses) and `scripts/eval/piper_ort_threads.py`.

## CPU tuning findings (AVX-512, threads, pinning)

- This CPU has AVX-512 F/BW/VL/VNNI/BF16 on 8 vCPUs. The pinned Piper ships onnxruntime 1.14.1, whose MLAS already selects AVX-512 kernels at runtime. There is no flag to turn on. The real handicaps are in `piper.cpp`: `ORT_DISABLE_ALL` graph optimization, no mem-pattern or arena, and default intra-op threads with no CLI option. The thread count stays at 8 whatever the affinity. There is no cmake or g++ on the box, so the C++ CLI cannot be rebuilt here.
- The same Piper models and phonemizer on ORT 1.31 with Level3 optimization (`01-piper-rt`, Rust, out of process, same JSON/raw protocol, same PCM, correlation 1.0) are **35–45 % faster warm** at every length (4w 21.2 vs 37.4 ms at t8). Under heavy load (~17), t4 with spin was the sweet spot: 4w 29.9/36.6 vs the C++ piper's 299/343. **It conflicts with LOCKS.toml** (`in_tree_ort=false`, "Do not widen this pin"), and the owner may consider it "switching". It is not on the branch and not wired in; it needs an owner decision. It can be dropped in with no code change via `GALION_A21_RUNTIME`. It also costs about 1.3 s of extra load time, which is fine behind the pre-warm.
- Thread matrix (ORT 1.31, load ~9, 4w p50/p95): t2 41.5/45.4, t4 27.4/35.3, t4 no-spin 32.8/41.7, **t8 21.2/23.4**, t4 pinned 4-7 31.0/65.0.

## Voices (low vs medium), warm TTFA p50/p95, load ~4.1 (C++ piper)

| voice | 1w | 4w | 8w | cold |
|---|---|---|---|---|
| ljspeech-medium (current, 22.05 kHz) | 18.7/21.3* | 37.4/46.6* | 68.3/87.7* | 415–472 |
| lessac-medium (22.05 kHz) | 19.5/23.2 | 25.5/28.3 | 55.8/79.1 | 575 |
| lessac-low (16 kHz) | 16.5/20.7 | 23.6/26.1 | 53.2/62.9 | 249 |
| amy-low (16 kHz) | 12.9/14.4 | 24.9/36.0 | 52.4/56.5 | 456 |

\*measured at load 8.9, not 4.1, so it is not directly comparable. Low voices save roughly 0–15 % warm. They cost bandwidth (16 kHz, duller on a phone/web speaker), and both change the speaker identity away from ljspeech. en_US has no x_low voice. **Recommendation: keep ljspeech-medium.** Better levers are the first-clause length and the runtime.

## First-chunk size → recommendation to agent 04

TTFA grows roughly linearly with words (~8–9 ms per word on the C++ piper at load 9). To keep worst-case TTFA under 50 ms on the current binary, the **first clause must be ≤4 words** (4w p95 44–47 ms). 8 words gives a p95 of 77–88. The `Clauser` first-clause cap is owned by agent 04.

## Production checklist (pass/fail)

| # | item | status |
|---|---|---|
| 1 | Piper is a long-lived warm process; no respawn per utterance | PASS |
| 2 | Pre-warm at call start, covering several lengths, before the lane reports ready (galion-ml-voice) | PASS |
| 3 | Pre-warm in the live call harness (galion-v2-live `call.rs`) | PASS (code outside git; needs adoption) |
| 4 | No cold start inside a live call (respawn re-warms) | PASS in code; not yet verified in a live call |
| 5 | PCM streamed to playback as it is read (no wait for the end marker) | PASS |
| 6 | No fixed idle wait after an utterance (30 ms bug) | PASS (galion-v2-live) |
| 7 | Piper crash does not panic the TTS lane | PASS (galion-v2-live) |
| 8 | Warm TTFA p50 ≤ 30 ms for first clauses as shipped (C++ piper, ≤4 words) | FAIL (3w 26–30, 4w 37.4) |
| 9 | Worst-case TTFA < 50 ms | FAIL on the C++ piper (in-loop replay p95 148). It would PASS for ≤4w on the ORT-1.31 runtime (4w p95 23.4) |
| 10 | ORT threads / graph optimization tunable for this CPU | FAIL: blocked by the LOCKS pin and the missing toolchain; prototype ready |
| 11 | Core pinning evaluated, safe default chosen (off) | PASS |
| 12 | Low/x_low voice tradeoff measured | PASS (keep medium) |
| 13 | Unit/integration tests and forbidden-runtime gates green | PASS (galion-ml-voice 59 + integration tests; check-no-forbidden OK) |
| 14 | Full-loop in-loop rerun with the fixes | FAIL (no LLM access for this agent; replay proxy only) |

**Completion: 10 / 14 = 71 %.**

## Estimated contribution to the 240 ms total (*estimate*)

- 30 ms idle-wait fix: up to −30 ms on any turn where the backchannel precedes the first clause (*estimate*, from the research gap).
- Warm Piper TTFA with ≤4-word first clause on the current binary: about −34 ms p50 vs 71.3 (to ~37) (*estimate from measured standalone*). Replay measured −11.6 p50 / −40 p95 with the current clause lengths.
- If the owner approves the ORT-1.31 runtime: in-loop 39.2/69.3 measured replay (−32 / −119), and about 21 ms p50 for 4w.
- First-call cold start: −330 to −470 ms on the first clause of each call (moved behind `ready`).

## Blockers

1. LOCKS.toml pins Piper's ORT to 1.14.1 out of process ("do not widen"), and `piper.cpp` hard-codes the ORT options. With no cmake or g++ on the box, the Piper binary cannot be retuned. The ORT-1.31 Piper runtime needs an owner decision.
2. The galion-v2-live fixes live outside git. Its owner needs to adopt them.
3. The first-clause word cap belongs to agent 04.
4. Machine contention (load 9–21) makes p95 swing 2–5×. The numbers above note the load for each run.
