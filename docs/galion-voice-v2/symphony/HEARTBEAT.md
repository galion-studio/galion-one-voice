# Heartbeat

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-03. Symphony section. Index: [README.md](README.md).

The spelling is **Galion**. The product is Galion Voice.

One step per frame. The frame carries the you band, the me band, and the feeling band. The crate is `crates/galion-heartbeat`. The period is 80 ms. The sample rate is 24 kHz. One frame is 1920 samples. The latency bar for sectors that share this clock is [80MS.md](80MS.md).

`Heartbeat::step` returns on the frame it was given. It does not block, allocate, wait on another thread, read the clock, or touch the network. State is fixed-size inside the struct (`tests/zero_alloc.rs`). A frame the scheduler marks late, or a you-band buffer that is not exactly 1920 samples, is delivered with `paused = true`. The me band starts nothing new on that frame. The next frame does not catch up.

The feeling decision made in frame *t* is audible in frame *t+1*. Rika, inside the step, needs an open session, consent bits `sense` and `adapt`, and a permitted purpose. Default config leaves Rika off. Without `sense`, PCM is not read for affect. Consent starts denied.

## Run

```bash
scripts/run-live-demo.sh
scripts/run-live-demo.sh --showcase
LIVE_REALTIME=1 scripts/run-live-demo.sh
cargo run --offline -q -p galion-heartbeat --bin galion-heartbeat-presence
cargo test --offline -p galion-heartbeat
```

The live demo writes a stereo WAV (left = you, right = me) and prints `GALION_LIVE_DEMO_OUT=<dir>`. `--realtime` paces the wall clock at one frame per 80 ms. The presence binary reads the spec file (default `specs/presence.json`) and prints a summary. Longer notes: [LIVE-DEMO.md](../../LIVE-DEMO.md), [EUREKA.md](EUREKA.md).

The turn worker, Vosk, Piper, and the clone child are forbidden inside `step`. They run on their own threads and hand PCM back as frames.

## Gate

**PASS** when `cargo test --offline -p galion-heartbeat` is green on the commit, including the zero-allocation test and the late-frame pause, and the presence summary is pasted into [SELF-EVAL.md](SELF-EVAL.md).

**FAIL** when `step` allocates, blocks, or a late frame is rewritten into a catch-up.

**BLOCKED** when the crate does not build offline on the machine being scored.

— Maciej Grajczyk
