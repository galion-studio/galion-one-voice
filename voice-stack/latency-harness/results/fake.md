<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright (c) 2026 Galion Labs -->

# S12 latency harness

Backend `fake`. Profile `release`. Schema `galion-s12-latency/v1`.

The pipeline product claim is false. This file is a measurement of the stages named below.

## Machine

- CPU: Intel(R) Xeon(R) Processor (2400.000 MHz)
- nproc: 4
- OS: linux x86_64, kernel 6.12.113+
- Hostname: cursor
- Load average: 0.08 0.05 0.01
- Date (UTC): 2026-11-21T02:49:44Z
- Commit: `f0b39b38f42f016fe620fe3ac1a4a096745f10eb`
- Commit dirty: false
- rustc: rustc 1.83.0 (90b35a623 2024-11-26)

n = 100. Warmup reps discarded: 1.

Percentile index is `ceil(n * p / 100) - 1` on the sorted samples, the same `rank` as `crates/galion-heartbeat/tests/frame_budget.rs`. At this n, p99 is not the maximum sample.

The bar is 80 ms on each stage. `derived` rows are the 80 ms heartbeat grid from `scripts/eval/piper_stream_ttfa.py` (origin = first PCM byte). They are computed from the measured arrival. They are not a second wall clock and they are not a sleep. Each stamp is truncated to microseconds on its own, so the stage deltas can miss the direct end-to-end clock by a few microseconds.

| stage | clock | min ms | p50 ms | p95 ms | p99 ms | max ms | p50 ≤ 80 ms | p95 ≤ 80 ms | p99 ≤ 80 ms |
|---|---|---:|---:|---:|---:|---:|---|---|---|
| vad (offline buffer) | measured | 0.003 | 0.003 | 0.005 | 0.005 | 0.005 | yes | yes | yes |
| end of speech to ASR final | measured | 0.002 | 0.002 | 0.003 | 0.003 | 0.015 | yes | yes | yes |
| ASR final to reply ready | measured | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | yes | yes | yes |
| reply ready to first TTS audio | measured | 0.009 | 0.009 | 0.012 | 0.013 | 0.019 | yes | yes | yes |
| first TTS audio to first audio out | measured | 0.005 | 0.005 | 0.007 | 0.008 | 0.008 | yes | yes | yes |
| end of speech to first audio out | measured | 0.017 | 0.019 | 0.024 | 0.028 | 0.037 | yes | yes | yes |
| end of speech to heartbeat play | derived | 0.012 | 0.012 | 0.016 | 0.022 | 0.029 | yes | yes | yes |

## Stage engines

- VAD: `energy-rms-80ms`
- ASR: `fake-rms-scan`
- Reply: `fake-horbm-slot`
- TTS: `fake-sine-frame`
- Audio out: `heartbeat-frame-sink`

Last reply mood: arousal 0.3000, valence 0.2500, dominance 0.5000. Rule `toward-rest-0.1`.

## Notes

- Fake backend. VAD, ASR, reply, and TTS are in-process and labeled fake. No sleep stands in for a model. product_claim is false. The sine is not a voice.

Raw per-rep microseconds are in the JSON file under `reps`.
