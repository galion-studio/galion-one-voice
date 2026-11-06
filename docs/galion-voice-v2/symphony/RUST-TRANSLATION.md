# Rust translation

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-06. Symphony section. Index: [README.md](README.md).

The spelling is **Galion**. The product is Galion Voice.

The path that runs is Rust. `Heartbeat::step` in `crates/galion-heartbeat` is the live frame. `scripts/run-live-demo.sh` builds `galion-live-demo` and calls that step. `scripts/run-v1-screen.sh` builds the `galion-console` example `dot_console_demo` and `galion-asr-vosk`. Python does not sit on that path.

## What runs

| Crate | Role |
|---|---|
| `galion-heartbeat` | One `step` per frame. You, me, feeling. |
| `galion-emotion` | OKLCH weight, prosody, consent. Default deny. |
| `galion-cascade` | English lane and PhoneLane. Off the tick. |
| `galion-asr-vosk` | Vosk child for the microphone. |
| `galion-ml-voice` | Piper child, flite fallback, clone-worker slot. |
| `galion-console` | The one circle, the turn worker, `/turn` and `/reply`. |
| `galion-render` | The mark and the loopback HTTP surface. |
| `galion-live-demo` | Headless frame demo. |
| `galion-presence`, `galion-presence-eval` | Replay and the virtual user. |
| `galion-symphony` | Reads a clone interchange and writes a pack the Rust stack can open. |
| `galion-openmind` | Nerve notes. The Open Mind tree is not vendored. |
| `galion-student` | Distillation plan. A `.gq` voice file is not in the tree yet. |

Minimum Rust for `galion-heartbeat` is 1.85. This page does not record a `rustc` version from this machine.

The clone child, when it is started, is out of process. `crates/galion-ml-voice/src/speech/clone_worker.rs` keeps PyTorch in that process. It is not linked into these crates. Generate stays in the child. `Heartbeat::step` does not call it. The host drains PCM before the step, into a ring the step already holds. Chatterbox safetensors are that child's weight input. They are not the file the heartbeat plays.

`galion-symphony translate` writes `SYMPHONY.txt`, `SHA256SUMS`, and `tensors/<sector>.gq`. `SYMPHONY.txt` starts with `GALION-SYMPHONY 1`. `load` checks the sums and opens each `.gq`. A clip, when present, is `clip.gclip` with magic `GALIONCL` (`crates/galion-symphony/src/clip.rs`). The payload is the WAV bytes unchanged. The pack's lane rate is `galion_heartbeat::SAMPLE_RATE` (24000). The clip is not fed to `Heartbeat::step`.

`weights/` is gitignored. This page does not commit a pack, a wav, or a weight.

## Python is a cross-check

Python reads the tree and compares a side result. It is not the voice that runs.

| Tool | What it checks |
|---|---|
| `galion-demo/galion_demo.py` | Canned stdlib port of the emotion and Rika policy. Offline wav. The live demo is `galion-live-demo`. |
| `scripts/symphony/eval.py` | Reads `LOCKS.toml` and scores the tree. `scripts/symphony/self-eval.sh` runs it. It does not write the lock. |
| `docs/galion-voice-v2/symphony/scripts/ttfa_check.py` | Reads the lock and the Rust frame constant. A stub replay has no product latency. |
| `scripts/symphony/check-g-latency.sh` | Uses `python3` to read constants and the cargo line. The step under test is `cargo test -p galion-heartbeat`. |
| `python_written_file_verifies` | Ignored test in `galion-ml-voice`. It opens a file the Python writer already produced. It is not the runtime. |

A Python interpreter inside `Heartbeat::step` is a fail of this section.

## The frame

The 80 ms frame is the existing lock. `LOCKS.toml` `[clock]` has `frame_ms = 80`, `frame_hz = 12.5`, and `configurable = false`. `galion_heartbeat::FRAME_MS` is 80. `SAMPLE_RATE` is 24000. `FRAME_SAMPLES` is 1920. `frame_constants_match_locks` asserts the constant against that lock text. This page does not edit `LOCKS.toml`.

The same period is the compile-time assert in `galion-symphony` (`CLOCK_MS`, `LANE_HZ`, `LANE_FRAME_SAMPLES`).

## CPU and latency

**BLOCKED.** This page does not record a host CPU share, a session vCPU, a tick time, a TTFA, or a mouth-to-ear time. Those figures were not measured for this page. `galion_symphony::PRODUCT_CLAIM` is false.

The pack may store a host id: `epyc9655p`, `cx23`, or `cx43`. Core counts, RAM, prices, and session budgets for those ids are **BLOCKED** here. They are labels in `crates/galion-symphony/src/host_cpu.rs`, not a measurement from this machine.

`STEP_BUDGET_US` in `galion-heartbeat` and `frame_budget_p95_ms` in the lock are source entries. A measured time against either one, on this machine, is **BLOCKED**.

## Run

Rust path, from the repo root:

```bash
scripts/run-live-demo.sh
cargo test --offline -p galion-heartbeat
cargo test --offline -p galion-symphony
cargo run --offline -p galion-symphony -- eval
```

`eval` returns 0 when Fails is 0. BLOCKED does not fail that process. A missing owner wav or a missing weight is BLOCKED, not a pass. This page did not run these commands on this machine.

## Gate

**PASS** when a Galion voice file loads through the Rust path and the PyTorch process is still a child, with no torch linked into these crates.

**FAIL** when the runtime path opens a Chatterbox container as its primary voice file, or when a Python interpreter sits inside `Heartbeat::step`.

**BLOCKED** for every CPU or latency figure this page would otherwise print. The lock and the crates are in the tree. A measured tick is not.

— Maciej Grajczyk

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
