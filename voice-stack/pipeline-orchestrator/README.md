<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright (c) 2026 Galion Labs -->

# S09 pipeline orchestrator (`galion-sam-orch`)

The real-time conversation loop for Samantha. The directory is `voice-stack/pipeline-orchestrator` and the package is `galion-sam-orch`, which is the path the swarm workspace map lists. Mic audio comes in, an endpointer closes the turn, a reply engine answers, and speech goes out in sentence chunks. If you start talking while she is thinking or speaking, `AudioOut::stop` runs on that voice start.

This crate is its own Cargo workspace. It is not a member of the Galion Voice workspace. It does not link Piper, Vosk, Chatterbox, or `horbm`. Those stay behind the traits in `samantha_traits`.

## What it reuses

The numbers and the stage shapes come from the tree that already runs:

| Contract | Where it already lives | What this loop does with it |
|---|---|---|
| 80 ms heartbeat, 12.5 Hz | `LOCKS.toml` `[clock]`, `galion-heartbeat` | Mood advances on that tick. `Heartbeat::step` is not called. That step does not block and does not allocate. |
| Capture 16 kHz, 20 ms, 320 samples | `galion-asr-vosk` | `AudioFrame::capture` is one Vosk frame. |
| Playback 24 kHz, 1920 samples per 80 ms | `galion-heartbeat`, Piper worker | TTS chunks are PCM for that rate. This crate does not resample. |
| Energy VAD, 60 ms minimum voiced | `voice-stack/vad`, from `archive/2026-11-24/crates/galion-v2-live` `src/vad.rs` | `EnergyVad` uses the same floor, margin, and RMS formula. Default hang is 160 ms. |
| Clause flush at punctuation or 8 words | `galion-ml-voice` `speech/clause.rs` | The speaker receives those chunks, one at a time. |
| Phases idle / listening / thinking / speaking | `galion_console::bus::OrbMode` | `Handle::subscribe` is the in-process feed. The HTML dot polls `/state` every 80 ms. |
| Barge-in ceiling 200 ms | `LOCKS.toml` `[gates] barge_in_max_ms` | `AudioOut::stop` does not wait on the chunk channel. The fake-speaker time is what the tests print. |
| Loopback only | `galion-render` bind check, `deploy/systemd/templates/galion-voice-stage.service` | The dot refuses any non-loopback bind. This sector does not install or edit a unit. |
| Samantha reference | `GALION_SAMANTHA_REF` | A path, read by a TTS adapter at runtime. This crate only checks that the path is a file. |

`ReplyEngine` is the brain seam in the `samantha_traits` module. `HorbmReply` adapts a type that matches `horbm::Brain::turn(&str) -> TurnOut`. `Brain::open(memory_db, knowledge_dir, cases)` is recorded on `BrainOpen` and returns an error here: crate `horbm` is not in this tree, and this loop does not call Hermes. The memory pin is `galion-studio/galion-knowledge` rev `761a7f225304fc010379219e5b88c6669dd48f40`. The resting pad is `{ arousal: 0.3, valence: 0.25, dominance: 0.5 }`. `HoldMood` keeps the last pad. It does not invent a curve. Emotion is that pad on the TTS call, not words added to the reply.

## Bounded channels

| Queue | Bound | Policy |
|---|---|---|
| Capture frames | 8 (160 ms) | The mic task waits. The queue does not grow past one heartbeat of slack plus a little. |
| Turns | 1 | A newer end-of-turn replaces the single overflow slot. |
| Sentence chunks | 4 | Barge-in does not wait for this queue to drain before `AudioOut::stop`. |

## Run

Rust 1.75 or newer. CPU only. No model download, no API key.

### Linux

```bash
cd voice-stack/pipeline-orchestrator
cargo test
cargo run --bin galion-sam-orch
```

The one-dot page, loopback only:

```bash
cargo run --bin galion-sam-orch -- --ui
```

Then open `http://127.0.0.1:7879/`. The process exits after the scripted turn. The page is one dot. It polls `/state` and `/dot`.

### Windows

From the repo root on the owner machine (`G:\galion` is the home; the repo directory is wherever this tree is checked out):

```bat
cd voice-stack\pipeline-orchestrator
cargo test
cargo run --bin galion-sam-orch
cargo run --bin galion-sam-orch -- --ui
```

Reference clip, not committed, not opened by this binary:

```bat
set GALION_SAMANTHA_REF=G:\galion\refs\samantha.wav
```

```bash
export GALION_SAMANTHA_REF="$HOME/refs/samantha.wav"
```

`galion-sam-orch` prints `reference_present=true` when that path is a file, and `false` when it is unset or missing. It does not read the audio. A missing file still runs the fake mouth.

## Tests

`cargo test` runs the clause splitter, the energy endpointer, the turn overflow slot, the HORBM adapter, the loopback refusal, and the end-to-end fake loop. `cargo test -- --nocapture` prints one `MEASURE` line per timed run. Stage times on the fakes are wall clock. `listen_audio_ms` is capture-frame time (speech plus hang), not a handoff. The 80 ms bar is asserted on ASR, think, first PCM, barge-in cancel, and the mood `advance` call. The mood interval is the 80 ms tick; the test checks that the gaps sit on that heartbeat.

## What is not in this crate

- `horbm::Brain` and any card lookup
- A live microphone, Vosk process, Piper process, or Chatterbox worker
- A hardware speaker measurement (the cancel time is `AudioOut::cancel` on the in-memory speaker)
- Samantha's voice. No reference clip is in git.
