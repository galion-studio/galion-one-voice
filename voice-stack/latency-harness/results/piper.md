<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright (c) 2026 Galion Labs -->

# S12 latency harness

Backend `piper`. Profile `release`. Schema `galion-s12-latency/v1`.

The pipeline product claim is false. This file is a measurement of the stages named below.

## Machine

- CPU: Intel(R) Xeon(R) Processor (2400.000 MHz)
- nproc: 4
- OS: linux x86_64, kernel 6.12.113+
- Hostname: cursor
- Load average: 0.37 0.12 0.03
- Date (UTC): 2026-11-21T02:49:55Z
- Commit: `f0b39b38f42f016fe620fe3ac1a4a096745f10eb`
- Commit dirty: true
- rustc: rustc 1.83.0 (90b35a623 2024-11-26)

n = 100. Warmup reps discarded: 1.

Percentile index is `ceil(n * p / 100) - 1` on the sorted samples, the same `rank` as `crates/galion-heartbeat/tests/frame_budget.rs`. At this n, p99 is not the maximum sample.

The bar is 80 ms on each stage. `derived` rows are the 80 ms heartbeat grid from `scripts/eval/piper_stream_ttfa.py` (origin = first PCM byte). They are computed from the measured arrival. They are not a second wall clock and they are not a sleep. Each stamp is truncated to microseconds on its own, so the stage deltas can miss the direct end-to-end clock by a few microseconds.

| stage | clock | min ms | p50 ms | p95 ms | p99 ms | max ms | p50 ≤ 80 ms | p95 ≤ 80 ms | p99 ≤ 80 ms |
|---|---|---:|---:|---:|---:|---:|---|---|---|
| vad (offline buffer) | measured | 0.005 | 0.006 | 0.007 | 0.007 | 0.007 | yes | yes | yes |
| end of speech to ASR final | measured | 0.002 | 0.002 | 0.003 | 0.003 | 0.003 | yes | yes | yes |
| ASR final to reply ready | measured | 0.000 | 0.001 | 0.001 | 0.001 | 0.001 | yes | yes | yes |
| reply ready to first TTS audio | measured | 33.457 | 41.180 | 58.275 | 59.087 | 65.066 | yes | yes | yes |
| first TTS audio to first audio out | measured | 0.024 | 0.045 | 0.071 | 0.088 | 0.207 | yes | yes | yes |
| end of speech to first audio out | measured | 33.542 | 41.228 | 58.332 | 59.147 | 65.134 | yes | yes | yes |
| end of speech to heartbeat play | derived | 33.461 | 41.185 | 58.280 | 59.091 | 65.071 | yes | yes | yes |

## Stage engines

- VAD: `energy-rms-80ms`
- ASR: `fake-rms-scan`
- Reply: `fake-horbm-slot`
- TTS: `piper`
- Audio out: `heartbeat-frame-sink`

## Cold Piper clocks

These two numbers are the warmup utterance. They are not in the percentile table.

- Phrase: Hi.
- Spawn to first PCM: 330.370 ms
- Warmup write to first PCM: 329.941 ms

## Piper

- Runtime: `weights/runtime/piper/piper`
- Model: `weights/tts/en_US-ljspeech-medium.onnx`
- Sample rate: 22050 Hz
- Model sha256: `6f52a751e2349abe7a76735eb09dc1875298c77ea2342ffd2fef79ff81b87f22` (pinned: yes)
- Config sha256: `141d612cc0a95ed7efc1ca936b845c2364967f2e9217c5dbfcf69fc4d6c65860` (pinned: yes)
- Prosody: fixed-resting. length_scale 1.0, noise_scale 0, noise_w 0. The reply mood is recorded and is not applied to this child.

Last reply mood: arousal 0.3000, valence 0.2500, dominance 0.5000. Rule `toward-rest-0.1`.

## Notes

- TTS is Piper en_US-ljspeech-medium (LJSpeech, public-domain data, MIT pack) on this CPU. It is not Samantha and not a clone. GALION_SAMANTHA_REF was not read.
- VAD, ASR, and the reply are the fake stages, the same ones as the fake backend. Those lines are not Vosk and not HORBM.
- First TTS audio is the first PCM byte from the warm child. First audio out is the wall time when one resampled 24 kHz heartbeat frame is in the sink. The heartbeat play row is derived from those arrivals on the 80 ms grid (origin = first PCM byte), the same rule as scripts/eval/piper_stream_ttfa.py. The rest of the utterance is drained after the clock stops.
- Prosody is fixed at rest (length_scale 1.0, noise_scale 0, noise_w 0) so the distribution is the CPU, not a noise draw. The reply still carries a Pad. That Pad is not applied to this Piper child.
- Spoken phrase: "I am right here.". Cores were not pinned.

Raw per-rep microseconds are in the JSON file under `reps`.
