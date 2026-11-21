<!--
SPDX-License-Identifier: MIT
Copyright (c) 2026 Galion Labs
-->

# samantha-brain

Sector S04 of the Samantha voice stack. This crate is the reply bridge:

```text
ASR text → ReplyEngine → reply text + Pad mood + sources
         → speakable sentence chunks → streaming TTS
```

The mood keeps moving on the 80 ms voice heartbeat between turns. The real
HORBM brain is not in this tree. `FakeEngine` is the stand-in tests and the
`say` command use. Feature `horbm` compiles `HorbmEngine` against the `horbm`
crate. That dependency is the type seam in `seam/horbm` until the real crate
is at `../../horbm/crates/horbm`.

This directory is its own Cargo package. It is not a member of the root
workspace. It does not synthesize audio, does not read `GALION_SAMANTHA_REF`,
and does not call a model API.

## What it reuses

- **80 ms frame.** `FRAME_MS` is `galion-heartbeat`'s `FRAME_MS`, the lock
  `frame_ms = 80` in `LOCKS.toml`. `Session::tick` is one frame, called beside
  `Heartbeat::step`, not inside it. A late `MoodTick` wake does not catch up.
- **Mood decay.** One frame uses Samantha's half-life from
  `galion-emotion` (`SAMANTHA.dynamics.half_life_ms` = 20 s) and the same
  exponential step as `PersonaMood::tick`. The rest point is HORBM's
  `Pad::RESTING` (arousal 0.3, valence 0.25, dominance 0.5), not the Piper
  persona rest.
- **Sentence chunks.** The split, the 16-word cap, the abbreviation and
  decimal rules, the `[tag]` strip, and the forbidden-phrase list are the
  ones in `crates/galion-cascade/src/early_reply.rs` and
  `hermes::FORBIDDEN`. Chunks are what the warm Piper path speaks. The
  reply text itself is left as the brain wrote it.
- **Mouths this crate does not own.** Piper (`galion-ml-voice`,
  `runtimes/galion-piper-rt`), Vosk (`galion-asr-vosk`), and the Chatterbox
  worker (`deploy/systemd/galion-clone-cbx.service`) stay where they are.
  Reference audio stays on disk and is loaded by the mouth from
  `GALION_SAMANTHA_REF`. Do not commit that clip.

## Linux

From the repo root:

```bash
cd voice-stack/s04-horbm-bridge
cargo test
cargo test --features horbm
cargo run -- tick
cargo run -- chunks The kettle is on. It can wait.
cargo run -- say the kettle is on
cargo run -- pace
```

`chunks` and `say` also read stdin when you pass no words:

```bash
echo 'The kettle is on. It can wait.' | cargo run --quiet -- chunks
```

## Windows

Rustup's `cargo` on `PATH`. The owner's home is `G:\galion`. From the clone
(the folder name under `G:\galion` is the clone, often `galion-one-voice`):

```bat
cd /d G:\galion\galion-one-voice\voice-stack\s04-horbm-bridge
cargo test
cargo test --features horbm
cargo run -- tick
cargo run -- chunks The kettle is on. It can wait.
cargo run -- say the kettle is on
cargo run -- pace
```

PowerShell uses the same `cargo` lines after `Set-Location` to that directory.

## Feature `horbm`

```bat
set GALION_HORBM_MEMORY=G:\galion\horbm\memory.db
set GALION_HORBM_KNOWLEDGE=G:\galion\horbm\knowledge
set GALION_HORBM_CASES=G:\galion\horbm\cases
cargo run --features horbm -- say hello
```

```bash
GALION_HORBM_MEMORY=$HOME/galion/horbm/memory.db \
GALION_HORBM_KNOWLEDGE=$HOME/galion/horbm/knowledge \
GALION_HORBM_CASES=$HOME/galion/horbm/cases \
cargo run --features horbm -- say hello
```

Those three variables are optional. Unset, `say` stays on `FakeEngine`.

The landing line, relative to this `Cargo.toml`, is:

```toml
horbm = { path = "../../horbm/crates/horbm", optional = true }
```

Cargo 1.83.0 resolves an optional path dependency even when the feature is
off. A missing directory fails `cargo test` before any test runs (measured:
`failed to read .../horbm/crates/horbm/Cargo.toml`). Until that directory
exists, the feature links `seam/horbm`, which has the `Brain::open` /
`Brain::turn` types and does not answer. `build.rs` prints a warning when
the landing `Cargo.toml` appears so the path can be swapped. The seam's
source id is `seam:horbm-not-landed`.

## Mood between turns

`Session::tick` once per heartbeat frame. `MoodTick::spawn` is the same
clock on its own thread for a host that is not inside `galion-heartbeat`
yet. Do not run both for one session.

## Measured

One release run on this Linux x86_64 box, rustc 1.83.0, `cargo run --release`.
Not a Windows laptop run. Not an ASR or TTS latency.

`tick`, 2000 calls after 64 warm-up calls:

| op | min ns | p50 ns | p95 ns | max ns |
|---|---:|---:|---:|---:|
| `pad.advance_frame` | 34 | 36 | 37 | 222 |
| `speakable_chunks` on a two-sentence line | 1578 | 1615 | 1642 | 45835 |
| `FakeEngine::turn` | 120 | 122 | 133 | 161 |

`pace` slept until each 80 ms deadline, then called `Session::tick`. Eight
wake-to-wake gaps, microseconds: 80076, 80079, 80071, 80074, 80088, 80080,
80086, 80070. After a `hey!` turn (arousal 0.7) those 8 frames left arousal
0.6912253 and valence 0.543419. `MoodTick` completed 6 frames in a 500 ms
sleep (the first frame waits 80 ms).

`say the kettle is on` printed `engine: fake`, one chunk, wall 4 µs.
`say 'The kettle is on. It can wait.'` printed two chunks, wall 6 µs.
`took_us` printed 0 because that field counts whole microseconds.

With `--features horbm` and the three path variables set, `say hello` printed
`engine: horbm`, an empty reply, and source `seam:horbm-not-landed`. The seam
does not open those paths.

`cargo test` and `cargo test --features horbm` passed on this box (17 and 19
library tests, 5 integration tests each). The seam package's 2 tests passed.
The background mood-thread test passed on 5 separate runs. Clippy
`--all-targets -D warnings` was clean for both feature sets. Cargo prints
`ignoring resolver config table without -Zmsrv-policy` because the repo
`.cargo/config.toml` targets rustc 1.85 and this cargo is 1.83. The build
still finished.

## Layout

| Path | Role |
|---|---|
| `src/engine.rs` | `ReplyEngine`, `Reply` |
| `src/fake.rs` | `FakeEngine` |
| `src/chunks.rs` | speakable sentences |
| `src/pad.rs` | `Pad` and the 80 ms decay |
| `src/session.rs` | `Session`, `MoodTick` |
| `src/linked.rs` | `HorbmEngine` (`--features horbm`) |
| `seam/horbm` | type seam, not the brain |
