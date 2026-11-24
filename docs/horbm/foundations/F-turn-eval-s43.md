# F — turn-taking eval (S43)

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Swarm | HORBM S43. Closed full-duplex turn-taking eval for Samantha. |
| Crate | `horbm/crates/horbm-eval`. Member of the `horbm` workspace. |
| Base | Merged `main` at `bd2aac9e` (#286, #282, #292). Echo cancellation is inside `Duplex::tick`. |
| Clock | Virtual. One `Duplex::tick` is 80 ms, 16 kHz, 1280 samples. No wall clock in the score. |
| Persona | Samantha. `open_persona("aria")` still refuses her. |
| Speech | In-memory stand-in, generated when the eval runs. Not Piper: this tree has no voice model, and the eval does not download one. No audio file is written. |
| Network | The binary binds nothing. |
| Locks | `LOCKS.toml` was not edited. `SOUL.md` is not in this tree and was not added. |

## What the numbers follow

Pause takeover, turn-taking latency, and backchannel shape follow Full-Duplex-Bench:

Guan-Ting Lin, Jiachen Lian, Tingle Li, Qirui Wang, Gopala Anumanchipalli, Alexander H. Liu, Hung-yi Lee. *Full-Duplex-Bench: A Benchmark to Evaluate Full-Duplex Spoken Dialogue Models on Turn-taking Capabilities.* arXiv:2503.04721. Code: <https://github.com/DanielLin94144/Full-Duplex-Bench>.

Stop time and whether she resumes or yields follow Full-Duplex-Bench v1.5 (arXiv:2507.23159), which scores overlap by stop latency and by resume versus respond. That paper does not publish a single overlap ratio. The ratio here is the share of Samantha's audible frames that are also user-voiced, on the same two-band timeline.

A takeover is non-backchannel model audio. A backchannel in the paper is shorter than one second and shorter than two words. Percentiles are linear: rank `p/100 * (n − 1)`.

Turn-taking latency is the end of the user's last voiced frame to her first audible frame, and only on trials where she answers. Stop time is the first barge frame to the frame that silences her.

## Measured on this clock

rustc 1.85.0, Linux, `cargo run --manifest-path horbm/Cargo.toml -p horbm-eval --bin horbm-turn-eval`, after merging `main` `bd2aac9e`. 24 scenarios, 684800 samples, all generated in the process.

| Metric | p50 | p95 | n | Gate |
|---|---|---|---|---|
| Pause handling TOR | 1.000 | 1.000 | 6 | off, end-of-turn is stub |
| Turn-taking latency | 160 ms | 160 ms | 6 | p50 ≥ 80 ms, p95 ≤ 240 ms |
| Backchannel rate | stub | stub | 2 | off |
| Backchannel offset | stub | stub | 0 | off |
| Interruption stop | 160 ms | 160 ms | 3 | p50 ≥ 80 ms, p95 ≤ 240 ms |
| Interruption resume | 1.000 | 1.000 | 7 | floor = 1 |
| Overlap ratio | 0.196 | 0.500 | 20 | p95 ≤ 0.550 |
| False barge-in | 0.000 | 0.000 | 2 | p95 ≤ 0.000 |

The latency gates are the measured 160 ms plus or minus one frame. The overlap gate is the measured p95 plus 0.05. Resume is the measured set: all 7 trials held. A miss fails, because each trial is one deterministic script, not a noisy draw. False barge-in is gated at the measured p95 of 0, with no slack.

Pause TOR is 1. The conductor's hang is one quiet frame and the commit is the next, so she is audible 160 ms into a 400–1000 ms intra-turn pause. That number is measured. It is not gated, because the prosodic end-of-turn that would hold an unfinished pause is not in this loop.

## Stages that are still stub

| Stage | Pull request | How this build decides | What the probe did |
|---|---|---|---|
| End of turn | #295 | Live only if a 400 ms pause after `um` does not take the floor. `tick_turn` / `TurnMsg` in `horbm-duplex` is the wired bit. | She took the floor. `wired=false`. |
| Backchannel | #301 | Live if she plays a short nod while the floor is still Listening, or if the duplex sources grow `set_backchannel_config` / `ListenCue`. | No nods on two user turns longer than 6 s. `wired=false`. |

Echo cancellation from #292 is on the duplex path. `Duplex::tick` cleans the microphone with the previous playback frame before the ear. Both echo probes opened and neither barged. `FALSE_BARGE_P95_MAX` is `Some(0.0)`.

A live stage with no frozen limit fails closed. `PAUSE_TOR_P95_MAX`, `BACKCHANNEL_RATE_P95_MAX`, and `BACKCHANNEL_OFFSET_P95_MAX` are still `None`. Nothing here is a stand-in for those numbers.

## Scenarios

Six intra-turn pauses from 400 ms to 960 ms. Six finished turns. Three barges. Two listed backchannels while she is speaking. One 80 ms hold. One single-frame tap. Two long listens. One filler probe. Two echo rooms (tone and noise; delay 160 samples, gain 0.08, taps at 48 and 96, the room from the open echo change).

The conversation mouth is `NativeMouth`. The echo far-end is a generated tone or noise, because the native square is quiet once that room gain is applied, and a quiet echo would hide a missing canceller.

## Linux

CI stays the Ubuntu job `voice-stack horbm router`. It tests and clippy-checks `horbm-eval` with default features. No Windows workflow was added. No `.exe`, drive letter, device path, or address literal.
