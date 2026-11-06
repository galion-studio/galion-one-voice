# Heartbeat

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-06. Symphony section. Index: [README.md](README.md).

The spelling is **Galion**. The product is Galion Voice.

The path that runs is Rust. One `Heartbeat::step` per frame, in `crates/galion-heartbeat`. The step carries the you band, the me band, and the feeling band. Python is a cross-check of that step. It is not inside it.

## The frame

The 80 ms frame is the existing lock. `LOCKS.toml` `[clock]` has `frame_ms = 80`, `frame_hz = 12.5`, and `configurable = false`. `FRAME_MS` is 80. `SAMPLE_RATE` is 24000. `FRAME_SAMPLES` is 1920. The latency bar for sectors that share this clock is [80MS.md](80MS.md). This page does not edit the lock.

`Heartbeat::step` returns on the frame it was given. It does not block, allocate, wait on another thread, read the clock, or touch the network. State is fixed-size inside the struct (`tests/zero_alloc.rs`). A frame the scheduler marks late, or a you-band buffer that is not exactly 1920 samples, is delivered with `paused = true`. The me band starts nothing new on that frame. The next frame does not catch up.

The feeling decision made in frame *t* is audible in frame *t+1*. Rika, inside the step, needs an open session, consent bits `sense` and `adapt`, and a permitted purpose. Default config leaves Rika off. Without `sense`, PCM is not read for affect. Consent starts denied.

The turn worker, Vosk, Piper, and the clone child are outside `step`. They run on their own threads and hand PCM back as frames. `scripts/run-live-demo.sh` builds `galion-live-demo` and calls this step. `scripts/run-v1-screen.sh` builds the Rust console. That is the path that runs.

## Python is a cross-check

| Tool | What it checks |
|---|---|
| `galion-demo/galion_demo.py` | Canned stdlib port of the emotion and Rika policy. Offline wav. The live demo is the Rust binary. |
| `scripts/symphony/eval.py` | Reads `LOCKS.toml` and scores the tree. It does not take a frame. |
| `docs/galion-voice-v2/symphony/scripts/ttfa_check.py` | Reads the lock and `FRAME_MS`. A stub replay is not this step. |
| `scripts/symphony/check-g-latency.sh` | Uses `python3` to read the constant and the cargo line. The call that exercises `step` is `cargo test -p galion-heartbeat`. |

A Python interpreter inside `Heartbeat::step` fails this section.

## CPU and latency

**BLOCKED.** This page does not record a tick time, a p99, a host CPU share, or a session vCPU. Those figures were not measured for this page. `STEP_BUDGET_US` is a harness ceiling in source. A measured time against it, on this machine, is **BLOCKED**.

## Run

```bash
scripts/run-live-demo.sh
scripts/run-live-demo.sh --showcase
LIVE_REALTIME=1 scripts/run-live-demo.sh
cargo run --offline -q -p galion-heartbeat --bin galion-heartbeat-presence
cargo test --offline -p galion-heartbeat
```

The live demo writes a stereo WAV (left = you, right = me) and prints `GALION_LIVE_DEMO_OUT=<dir>`. `--realtime` paces the wall clock at one frame per 80 ms. The presence binary reads the spec file (default `specs/presence.json`) and prints a summary. Longer notes: [LIVE-DEMO.md](../../LIVE-DEMO.md), [EUREKA.md](EUREKA.md).

This page did not run these commands here. A pasted presence summary, and any microsecond line from `frame_budget`, stays **BLOCKED** until a run on the machine being scored records it.

## Gate

**PASS** when `cargo test --offline -p galion-heartbeat` is green on the commit, including the zero-allocation test and the late-frame pause, and the presence summary is pasted into [SELF-EVAL.md](SELF-EVAL.md).

**FAIL** when `step` allocates, blocks, or a late frame is rewritten into a catch-up. **FAIL** when Python runs inside `step`.

**BLOCKED** when the crate has not been built on the machine being scored, and for every CPU or latency figure this page would otherwise print.

— Maciej Grajczyk

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
