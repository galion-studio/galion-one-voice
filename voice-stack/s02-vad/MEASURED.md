<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright (c) 2026 Galion Labs -->

# Measured on 2026-11-21

One Linux machine. Not the owner's Windows laptop. Windows was not run.

- Host: Linux 6.12.113+ x86_64, 4 cores, `/proc/cpuinfo` model name `Intel(R) Xeon(R) Processor` (that string has no SKU).
- rustc 1.100.0 (0ef1bc386 2026-11-09), crate built `--release`.
- Process environment for the bench: `OMP_NUM_THREADS=1`, `OMP_WAIT_POLICY=PASSIVE`.
- Clock: `CLOCK_THREAD_CPUTIME_ID` for `cpu_ns`, `Instant` for `wall_ns`.
- Command: `cargo run --release --bin vad-bench` in `voice-stack/s02-vad`, after `scripts/fetch-cc0.sh`.
- Model sha256 `2623a2953f6ff3d2c1e61740c6cdb7168133479b267dfef114a4a3cc5bdd788f` (Silero VAD v5.1.2). Inputs observed at load: `input` f32, `state` f32 `[2, batch, 128]`, `sr` i64 scalar. Outputs: `output`, `stateN`.
- Clips: the five CC0 wavs in `scripts/sources.tsv`. Timing repeated them until 2272 frames (2272 × 20 ms = 45.44 s of audio).

## Per-frame time, release

| backend | frames | cpu min | cpu p50 | cpu p95 | cpu max | cpu mean |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| energy | 2272 | 510 ns | 519 ns | 545 ns | 968 ns | 522 ns |
| silero, every frame | 2272 | 531 ns | 68187 ns | 79523 ns | 138084 ns | 44677 ns |
| silero, model windows only | 1420 | 65487 ns | 66890 ns | 86166 ns | 135669 ns | 69207 ns |

Wall time for the same frames tracks the CPU time (Silero wall p50 68379 ns, p95 79724 ns, max 163897 ns). The timed Silero pass took 102.1 ms of wall clock for 45.44 s of audio.

A frame that does not finish a 512-sample Silero window only runs the turn rules, which is why the minimum (531 ns) sits next to the energy figure and the median (68.2 µs) sits on a model window. 1420 / 2272 frames ran the model, which is the 32 ms window against a 20 ms hop.

Silero load warmup, one silent window, then state cleared: 276546 ns (0.277 ms). Not included in the table.

A 20 ms frame is 20_000_000 ns. 20_000_000 / 79_523 = 251.5, so the Silero p95 sits that many times inside the frame. An 80 ms heartbeat is 80_000_000 ns. 80_000_000 / 79_523 = 1006.

## Events on one pass of each clip

`me_speaking` was false, so this pass does not contain a barge-in. Hangover was the default 500 ms. `t_ms` is the start of the 20 ms frame.

| clip | duration | energy start | energy end | silero start | silero end | silero start prob |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| just.wav | 973 ms | 240 | 1240 | 280 | 1260 | 0.6249 |
| mistreatment.wav | 1240 ms | 280 | 1500 | 280 | 1580 | 0.9815 |
| numerically.wav | 1280 ms | 280 | 1600 | 320 | 1660 | 0.8679 |
| sleepwalker.wav | 1403 ms | 260 | 1680 | 320 | 1720 | 0.7605 |
| so.wav | 757 ms | 260 | 920 | 280 | 1020 | 0.8109 |

Every clip produced one speech start and one speech end on both backends. Every end is later than the clip duration, so the hangover did not close the turn while the file was still playing. The bench then fed silence; the end is that silence reaching 500 ms, including whatever quiet tail the clip already had.

## Debug test, same machine, same clips

`cargo test` (debug, not release) printed, in microseconds:

- energy, 200 synthetic frames: min 8, p50 8, p95 9, max 23
- Silero frame p50 / p95 / max: just 88 / 117 / 158, mistreatment 92 / 114 / 123, numerically 92 / 119 / 157, sleepwalker 90 / 107 / 129, so 89 / 96 / 109

The release table is the one to quote. The debug lines are a second run, not a second machine.

## Not measured

- Windows.
- A microphone device. The API takes frames the live call already cuts.
- Barge-in on recorded overlap. Unit tests check the 80 ms frame count on a synthetic tone. The CC0 bench does not set `me_speaking`.
- The 300 ms hang from `call.rs` (`GALION_VAD_HANG_MS=300`). The run used the default 500 ms.
