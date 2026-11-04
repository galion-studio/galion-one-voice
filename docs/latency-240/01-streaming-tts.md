# 01: Streaming & TTS latency (Piper)

Agent 01. Stage budget (BUDGET.md): **TTS first audio 71.3 → 30 ms p50**; playback handoff 0 → 0.
Branch `swarm/lat240-streaming-tts` (worktree `wt-01-streaming-tts`). Owner directive t612u applies: keep Piper, no architecture change, no other TTS library. Tune it, keep it warm, fix integration bugs.
**Owner approval t613s2992 (2026-11-04 02:34 CET):** the LOCKS onnxruntime pin for the Piper path is widened to 1.31.1 and the Rust wrapper `galion-piper-rt` is now the default Piper runtime (same ljspeech-medium voice, same espeak-ng phonemes, PCM parity checked). The legacy piper CLI (ORT 1.14.1) stays as fallback with `GALION_PIPER_LEGACY=1`.
All numbers below are **measured** on this box under BENCH.lock, unless a line says *estimate*. The box is shared, so each run lists its 1-min load average. Raw data is in `01-data/*.ndjson`.

## Before / after (headline)

"Before" is the legacy piper CLI (ORT 1.14.1, `ORT_DISABLE_ALL`, 8 threads). "After" is the new default: `galion-piper-rt`, ORT 1.31.1, Level3, **4 intra-op threads, spin on, unpinned**. The before and after runs were back to back in one session (2026-11-04 02:44–03:05 CET, load 0.7–4.4). Raw data: `01-data/rt-threads-pin.ndjson`, `01-data/rt-replay-pin.ndjson`.

| metric | before (legacy piper) p50 / p95 | after (galion-piper-rt t4) p50 / p95 | load |
|---|---|---|---|
| Cold start, spawn to end of first utterance | 392 / 406 / 357 ms (3 runs) | 1405 / 1437 / 1440 ms (3 runs). It is **paid once at call start behind the pre-warm** and never inside a call; respawn after a crash also re-warms | 0.7–3.8 |
| Warm TTFA 1 word | 19.0 / 27.4 | **14.4 / 16.1** | 3.8 / 3.6 |
| Warm TTFA 4 words | 41.9 / 46.7 | **31.8 / 35.6** | 3.8 / 3.6 |
| Warm TTFA 8 words | 81.9 / 100.6 | **66.1 / 73.9** | 3.8 / 3.6 |
| Piper in-loop replay (24 real first clauses from `full-loop.ndjson`, each after "Mm-hm.") | 89.4 / 129.6 and 95.5 / 146.2 (2 runs). Real full loop baseline: **71.3 / 188.5** | **41.6 / 68.8, 45.3 / 63.6, 40.6 / 67.3** (3 runs) | 0.7–2.2 |

Earlier session (load ~9–10): legacy 4w 37.8/44.0, rt t4 27.4/35.3, rt t8 21.2/23.4. In-loop replay earlier: legacy 59.7/148.4 (load 4.1), rt t4 39.2/69.3 (load 4.4).

### Threads × pinning (galion-piper-rt warm TTFA p50 / p95 ms, 15 reps, one run each)

| config | 1w | 4w | 8w | cold ms | load |
|---|---|---|---|---|---|
| t1 pinned core 6 | 34.7 / 40.6 | 79.0 / 93.2 | 161.1 / 171.2 | 2063 | 4.4 |
| t2 pinned core 6 | 66.9 / 74.8 | 146.1 / 167.1 | 274.8 / 370.9 | 1724 | 4.1 |
| t4 pinned core 6 | 137.4 / 155.0 | 291.8 / 332.5 | 519.5 / 547.7 | 2243 | 4.0 |
| t1 unpinned | 29.2 / 35.3 | 68.6 / 85.7 | 152.4 / 190.2 | 1457 | 4.0 |
| t2 unpinned | 20.1 / 22.7 | 49.5 / 56.0 | 92.6 / 107.1 | 1562 | 3.7 |
| **t4 unpinned (default)** | 14.4 / 16.1 | 31.8 / 35.6 | 66.1 / 73.9 | 1405 | 3.6 |
| t8 unpinned | 11.4 / 14.1 | 22.5 / 25.0 | 45.2 / 54.0 | 1472 | 3.6 |
| t8 unpinned, spin off | 19.9 / 21.8 | 36.5 / 42.9 | 72.4 / 80.3 | 1541 | 0.7 |
| legacy piper pinned core 6 | 41.3 / 46.7 | 84.9 / 92.0 | 177.0 / 190.3 | 405 | 3.9 |

In-loop replay by config (p50 / p95): t1 core 6: 106.1 / 177.2. t4: 41.6 / 68.8, 45.3 / 63.6, 40.6 / 67.3. t8: 54.0 / 92.4, 58.4 / 143.5, 33.4 / 51.9. t8 spin off: 46.2 / 75.3.

**A single core cannot hold 30 ms p50 at 4 words.** The best single-core result is t1 on core 6: 79.0 / 93.2 ms at 4w, which is 2.6× the budget (legacy on core 6: 84.9). Putting more threads on one core makes it worse, because the spinning threads fight each other (t2 146, t4 292). VITS inference needs about 4 cores to get near 30 ms at 4 words.

**Default chosen: t4, spin on, unpinned** (`DEFAULT_THREADS = 4`; override with `GALION_PIPER_THREADS`, `GALION_PIPER_SPIN`, `GALION_PIPER_CPUS`). t8 is fastest warm (4w 22.5) but its in-loop replay swings 33–58 p50 and 52–143 p95 across 3 runs, and in a live call 8 spinning threads would compete with ASR and the LLM on all 8 vCPUs. t4 gives the steadiest in-loop numbers (40.6–45.3 / 63.6–68.8) and leaves half the box free. The core map's "Piper on core 6" does not fit this runtime. Piper needs about 4 cores at its burst, so the core map should be revisited with agent 05 (e.g. Piper on 4-7) before pinning is turned on.

A full-loop rerun was not possible. `v2-latency` needs an OpenRouter key, which I do not have and did not look for. The local llama-server belongs to agent 05 and is busy with its benches.

## What changed (shipped, gate-clean, Rust)

0. **galion-piper-rt is the default Piper runtime (t613s2992).**
   - `runtimes/galion-piper-rt` is a Rust binary with its own cargo workspace, excluded from the main workspace, so `cargo tree --workspace` stays free of ort. It uses `ort =2.0.0-rc.15` with `load-dynamic`, which dlopens `libonnxruntime.so.1.31.1`. espeak-ng is dlopened from the existing Piper pack.
   - It speaks the same `--json-input --output_raw` protocol and the same "Real-time factor:" boundary as the CLI. It does 2 warm-ups internally before the first line.
   - `scripts/a21/build-piper-rt.sh` fetches the pinned ORT tarball (sha256 checked), builds the binary, installs it to `weights/runtime/piper-rt/` and smoke-tests it.
   - `speech/weights.rs` picks the runtime in this order: `GALION_A21_RUNTIME`, then `GALION_PIPER_LEGACY=1` (legacy CLI), then galion-piper-rt if it is installed, then legacy. If the new runtime cannot spawn, or dies before its first audio, the lane falls back to the legacy CLI by itself. That path has a unit test.
   - LOCKS.toml `a21_piper` is widened for this path only (`onnxruntime = "1.31.1"`, `ort_crate_path`, `in_process = false`). candle, llama.cpp, ggml and GGUF are untouched, and the global `forbidden_runtimes` still lists ort. `check-locks.sh`, `check-no-forbidden.sh` and gate G3 allow exactly that one `ort` line and still fail on any other.
   - Parity (`runtimes/galion-piper-rt/tests/parity.rs`): on 8 sentences of 1–9 words, sample counts are identical, correlation is 1.000000, and the max diff is 8 LSB of 32767.
   - galion-v2-live (`src/piper.rs`, outside git) also defaults to the new runtime, unless `GALION_PIPER_LEGACY=1` is set.

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
- (History, before t613s2992.) The same Piper models and phonemizer on ORT 1.31 with Level3 optimization (`01-piper-rt`, now moved into the repo as `runtimes/galion-piper-rt`, Rust, out of process, same JSON/raw protocol, same PCM, correlation 1.0) are **35–45 % faster warm** at every length (4w 21.2 vs 37.4 ms at t8). Under heavy load (~17), t4 with spin was the sweet spot: 4w 29.9/36.6 vs the C++ piper's 299/343. It was blocked by LOCKS.toml until the owner approved it (t613s2992); it is now the default. It also costs about 1.3 s of extra load time, which is fine behind the pre-warm.
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
| 4 | No cold start inside a live call (respawn re-warms; rt warms itself twice) | PASS in code; not yet verified in a live call |
| 5 | PCM streamed to playback as it is read (no wait for the end marker) | PASS |
| 6 | No fixed idle wait after an utterance (30 ms bug) | PASS (galion-v2-live) |
| 7 | Piper crash does not panic the TTS lane | PASS (galion-v2-live; galion-ml-voice falls back to legacy if the rt dies before first audio) |
| 8 | Warm TTFA p50 ≤ 30 ms at 4 words as shipped | FAIL by a hair: rt t4 4w 31.8 ms (load 3.6; 27.4 at load 9 earlier). 1w 14.4. t8 would pass (22.5) but is less steady in-loop |
| 9 | Worst-case TTFA < 50 ms | PASS for ≤4-word first clauses (rt t4 4w p95 35.6). FAIL in-loop with today's clause lengths (replay p95 63.6–68.8, down from 129.6–146.2); needs agent 04's first-clause cap |
| 10 | ORT threads / graph optimization tunable for this CPU | PASS (galion-piper-rt: Level3, `GALION_PIPER_THREADS`/`SPIN`/`CPUS`) |
| 11 | Core pinning evaluated, safe default chosen (off) | PASS (single core 6: 4w 79.0 ms, so keep it off) |
| 12 | Low/x_low voice tradeoff measured | PASS (keep medium) |
| 13 | Unit/integration tests, clippy on touched files, lock + forbidden-runtime gates green | PASS (galion-ml-voice lib 85 + integration; rt clippy `-D warnings` clean; check-locks, check-no-forbidden, G3 OK) |
| 14 | Full-loop in-loop rerun with the fixes | FAIL (no LLM access for this agent; replay proxy only) |
| 15 | Audio parity new runtime vs legacy Piper on ≥6 sentences | PASS (8/8, corr 1.000000, equal lengths) |
| 16 | Legacy runtime kept as fallback (env flag + automatic) | PASS (`GALION_PIPER_LEGACY=1`; auto-fallback test) |

**Completion: 13 / 16 = 81 %** (was 10 / 14 = 71 %).

## Estimated contribution to the 240 ms total (*estimate*)

- 30 ms idle-wait fix: up to −30 ms on any turn where the backchannel precedes the first clause (*estimate*, from the research gap).
- Piper in-loop TTFA: measured replay 71.3 / 188.5 (real loop) → about 42 / 67 with galion-piper-rt t4. That is roughly −29 ms p50 and −120 ms p95 (*estimate*: the replay is Piper-only, with no concurrent ASR/LLM).
- With a ≤4-word first clause (agent 04): about 32 / 36 ms (measured standalone).
- First-call cold start: moved behind `ready` (legacy ~0.4 s, rt ~1.4 s), so 0 ms on the call path.

## Blockers

1. Resolved by t613s2992: the ORT 1.31 runtime is wired in on the branch (PR open, not merged, not deployed). The rt binary and the ORT 1.31.1 library are build artifacts under gitignored `weights/runtime/`, so each host needs one run of `scripts/a21/build-piper-rt.sh`. Without it, the lane falls back to the legacy CLI.
5. A single core cannot hold 30 ms at 4 words (79 ms). The "Piper on core 6" core map needs revisiting (about 4 cores at burst).
2. The galion-v2-live fixes live outside git. Its owner needs to adopt them.
3. The first-clause word cap belongs to agent 04.
4. Machine contention (load 9–21) makes p95 swing 2–5×. The numbers above note the load for each run.

---

© GOATSI — Galion Open Automated Technarchy Society Inc. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
