# 06 · Core map v3 (Piper gets 6-7; pins for LLM, Vosk and bus in code)

Branch `swarm/core-map-v3`, stacked on `swarm/lat240-core-pinning`. Written 2026-11-04, Europe/Zagreb (CET).
Host: 8 cores, no SMT. Every timed number ran under `flock latency-240/BENCH.lock`.
Raw lines: `06-core-map-v3.bench.ndjson`. Labels there: `v3` and `v3nospin` are the time-share 4-7 runs (named before the decision); `v3p2` and `v3final` are the shipped v3; `v2b` and `offb` are run 3.

## Problem
The v2 map put Piper alone on core 6. galion-piper-rt needs ~4 cores at burst: a 4-word clause took 79 ms (lane 01) / 88-102 ms (here), against ~40 ms unpinned.

## Map v3 (default when the map is on)
| core | role | threads | applied by |
|---|---|---|---|
| 0 | heartbeat / bus / call loop | 1 | `galion_cores::pin_current_thread_to(Bus)` in `dot_console_demo` (pins the loop thread only; web and turn-worker threads keep 0-7) |
| 1 | Vosk ASR | 1 | `VoskClient::open` (`sched_setaffinity` in the child before exec; `GALION_VOSK_THREADS` defaults to the placement) |
| 2-5 | local LLM (llama-server) | 4 | `galion-pin llm -- llama-server … -t {threads}` |
| 2-5 | retrieval | 4 | runs before the LLM prompt, so it shares the LLM cores in sequence |
| 6-7 | Piper (galion-piper-rt) | 2 | `PiperWorker::spawn` (affinity + `GALION_PIPER_THREADS`) |
| — | clone workers | 0 | `ClonePlan::Off` unchanged: no clone workers during a live call |

The map and pinning moved into the new std-only crate `galion-cores`, so Vosk, the call loop and `galion-pin` share one table. `galion_ml_voice::speech::core_map` re-exports it.
Knobs: `GALION_CORE_MAP=v3|v2|off|auto` (auto = v3 with `GALION_LLM_LOCAL` on 8+ cores), `GALION_CORES_<ROLE>`, `GALION_THREADS_<ROLE>`.

## Method
`scripts/bench-core-map-v3.sh` runs one config at a time. It starts the real llama-server (Qwen2.5-1.5B Q4_K_M, llama.cpp b12076) through `galion-pin llm`, then `examples/core_map_bench`, then kills the server.
The bench has one warm PiperWorker (rt, ljspeech-medium) and runs 12 reps per clause:
- Piper alone;
- a fixed 48-token LLM decode alone (`ignore_eos`);
- both at once: Piper starts 350 ms after the LLM request, after the ~110-150 ms prompt eval, so it overlaps the decode as in a turn.

Clauses: 4w "That sounds really hard." and 12w. First PCM = request write to first PCM byte.

## Results (p50 / p95 ms, LLM decode tok/s p50)
| config | LLM cores (t) | Piper cores (t) | Piper 4w alone | **Piper 4w with LLM** | Piper 12w with LLM | LLM tok/s alone / with Piper | prompt ms |
|---|---|---|---|---|---|---|---|
| v2 (run 1) | 2-5 (4) | 6 (1) | 102.1 / 118.2 | 108.1 / 133.8 | 236.4 / 276.3 | 24.2 / 24.6 | 113 |
| v2 (run 3) | 2-5 (4) | 6 (1) | 87.7 / 94.9 | 92.6 / 95.2 | 214.7 / 236.9 | 26.0 / 26.1 | 108 |
| time-share 4-7 (owner's suggestion) | 4-7 (4) | 4-7 (4) | 48.3 / 54.4 | **138.3 / 179.5** | 323.7 / 373.7 | 24.1 / 22.1 | 170 |
| time-share 4-7, rt spin off | 4-7 (4) | 4-7 (4) | 39.6 / 42.6 | 104.7 / 114.9 | 204.3 / 224.3 | 26.3 / 24.9 | 108 |
| overlap LLM 2-5 / Piper 4-7 | 2-5 (4) | 4-7 (4) | 38.8 / 48.9 | 92.7 / 111.2 | 218.9 / 292.5 | 25.7 / 24.7 | 147 |
| split 3+3 (run 1) | 2-4 (3) | 5-7 (3) | 47.3 / 49.8 | 49.5 / 74.8 | 115.5 / 166.7 | 19.0 / 19.3 | 154 |
| split 3+3 (run 2) | 2-4 (3) | 5-7 (3) | 43.9 / 51.5 | 44.3 / 47.3 | 114.8 / 132.7 | 19.8 / 20.0 | 142 |
| LLM 1-4 (shares Vosk core) | 1-4 (4) | 5-7 (3) | 41.8 / 56.6 | 57.2 / 79.7 | 115.1 / 174.9 | 25.9 / 26.6 | 114 |
| **v3: LLM 2-5 / Piper 6-7** (run 2) | 2-5 (4) | 6-7 (2) | 56.1 / 70.5 | 57.1 / 66.5 | 127.6 / 149.7 | 26.0 / 25.8 | 117 |
| **v3** (run 3, the shipped default) | 2-5 (4) | 6-7 (2) | 52.8 / 55.5 | **56.8 / 68.0** | **123.2 / 147.1** | 25.8 / 26.3 | 108 |
| off (unpinned, run 1) | 0-7 (4) | 0-7 (4) | 39.2 / 48.3 | 47.9 / 79.7 | 108.7 / 157.8 | 25.0 / 25.7 | 115 |
| off (run 3) | 0-7 (4) | 0-7 (4) | 40.2 / 48.0 | 50.8 / 67.9 | 137.7 / 180.4 | 26.6 / 26.2 | 115 |

The host loadavg was 1-6 during the runs. The bench adds part of that, so compare rows within a run first.

## Decision
**v3 = LLM 2-5 (4 threads), Piper 6-7 (2 threads).** Against v2 in the same run, Piper with the LLM decoding went from 92.6 to 56.8 ms at 4 words (-39 %) and from 214.7 to 123.2 ms at 12 words (-43 %). LLM decode stayed at 26 tok/s and prompt eval at ~108 ms.

- **Time-sharing 4-7 was measured and rejected.** llama.cpp keeps 4 threads hot through the decode, so Piper got about half the cores: 138 ms at 4 words, worse than v2.
- **Split 3+3 has the fastest Piper (44-50 ms) but loses on first audio.** The LLM drops to ~20 tok/s and prompt eval rises ~35 ms. With a ~6-token first clause, that costs about +70 ms of LLM time to save ~10 ms of Piper (*estimate*).
- **Unpinned** is about as fast for Piper at p50 but has a worse 12w p95 (180 ms) and gives no isolation for the bus and Vosk.

## Live pin check (real llama-server, demo, Vosk turn)
`results/core-map-v3/pincheck.sh`, `GALION_CORE_MAP=v3`. A real Vosk `/turn` returned `asr=vosk … err=-`. Affinity per process:
- llama-server, all 16 threads: 2-5
- demo bus loop thread: 0
- demo web and turn-worker threads: 0-7
- galion-piper-rt: 6-7
- galion-asr-vosk: 1

## Open
- The 30 ms TTFA target at 4 words is not met with 2 Piper cores (53 ms alone). 3 cores give 44 ms. Lane 01's 31.8 ms used 4 unpinned cores on a quiet host.
- The LLM decode rate here is the fixed 48-token `/completion` rate. Live first-clause timing through `galion-v2-live` was not re-run on this map.
- Retrieval has no process yet. Its placement is reserved only.
