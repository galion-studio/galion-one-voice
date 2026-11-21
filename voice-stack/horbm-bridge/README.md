<!--
SPDX-License-Identifier: MIT
Copyright (c) 2026 Galion Labs
-->

# galion-sam-horbm

Sector S04 of the Samantha voice stack. The directory is `voice-stack/horbm-bridge` and the package is `galion-sam-horbm`, the names in the Swarm 16 architecture. This crate is the reply bridge:

```text
ASR text → ReplyEngine → reply text + Pad mood + sources
         → speakable sentence chunks → streaming TTS
```

The mood keeps moving on the 80 ms voice heartbeat between turns. The `horbm` crate is at `../../horbm/crates/horbm`. `FakeEngine` is the
stand-in the `say` command uses when the feature is off. Feature `horbm`
compiles `HorbmEngine` against that crate. The live turn is
`horbm::Brain::turn`.

This directory is its own Cargo package. It is not a member of the root
workspace. It does not synthesize audio, does not read `GALION_SAMANTHA_REF`,
and does not call a model API.

## Layer 5 router

`ReasoningRouter` is the reasoning router. The 7881 server calls
`horbm::Brain::turn`, which uses the same card-then-gateway order as
[`ReasoningRouter::route`](src/router.rs) after the empathy rewrite. Cards
answer first. It does not call the research gateway first.

`Brain::open` in `galion-sam-orch` returns
`horbm is not linked; Brain::open stays in crate horbm`. The layer 5 router
does not call that function. The card index crate is `horbm-memory` in
`galion-knowledge` `v2/memory` at `761a7f225304fc010379219e5b88c6669dd48f40`.
That crate uses rusqlite, returns at most 4 hits, answers a miss with
`I don't know`, and queues the gap advice `ask_galion_research`. This loader
does not open it: `Stack::open` creates a directory and a sqlite file.
Knowledge-base development is paused. The JSON cards in that revision are
`v2/data/cards/horbm` (90 files) and `v2/data/cards/world` (1416 files).
They are read-only and they are not copied into this repo.

1. HORBM cards answer first. `MemoryCards` reads `*.json` with `File::open`
   and `read_dir` only. It does not create or write the knowledge directory.
   A card matches when every content token of the line is in that card, the
   same rule as `horbm-memory` `Stack::query` at that revision. Two equally
   specific cards are not a match. `covering` returns at most 4 hits and
   does not pick one.
2. No card, and the line is not a deep question: the spoken line is
   `I don't know.` The miss is appended to the in-memory gap queue with
   advice `ask_galion_research`. There is no S0, no LLM, and no Ollama.
3. A deep question (a `?`, and either eight words or the word why, how, or
   explain) may go to the Galion Research gateway on `127.0.0.1:7911`, with
   a 3 second cap. That gateway dials Hermes at `127.0.0.1:8642` and no
   other host. It does not use 8787. On timeout, or when Hermes has no
   answer, the spoken line is still `I don't know.`

`speakable_chunks` is the reply builder. The orchestrator sends those
sentences to the mouth. `RouterMouth` in `galion-sam-orch` is that path:
finished ASR, router, chunks, then `Tts` (Piper when the host plugs it in).

```bat
cd /d G:\galion\galion-one-voice\voice-stack\horbm-bridge
cargo test
```

`ReasoningRouter::from_env` reads `GALION_HORBM_KNOWLEDGE`. On the laptop
that directory is `G:\galion\knowledge\v2\data\cards` (the `horbm` and `world`
trees) or `G:\galion\horbm\knowledge` when a local copy is already there.
The loader does not create it and does not write `C:`.
`GALION_RESEARCH_BIND` defaults to off when unset. Set it to `127.0.0.1:7911`
to allow deep questions to reach the gateway. The router does not dial 8642.

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
cd voice-stack/horbm-bridge
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
cd /d G:\galion\galion-one-voice\voice-stack\horbm-bridge
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

`CardEngine` reads `.md` and `.json` knowledge cards and does not write them. A turn is `horbm::Brain::turn`: empathy mood, then cards, then the gateway. JSON cards use the layer 5 token rule. A markdown card answers when JSON has no hit. A miss is `I don't know.` `GALION_RESEARCH_BIND` is used only when it is `127.0.0.1` and the port is not 8787, 8642, 11434, 7881, or 7878. Unset, the router stays on cards. This process does not bind a port. The card index crate is `horbm-memory` in `galion-studio/galion-knowledge` `v2/memory` at `761a7f225304fc010379219e5b88c6669dd48f40`. This tree does not fetch it and does not open its sqlite file. `Brain::open` in `voice-stack/pipeline-orchestrator` still returns `horbm is not linked`.

The dependency, relative to this `Cargo.toml`, is:

```toml
horbm = { path = "../../horbm/crates/horbm" }
```

`seam/horbm` is the old type seam. It is not the dependency.

## Mood between turns

`Session::tick` on `FakeEngine` does not move the pad. The live `horbm::Brain::tick_frame` is one empathy frame and does not speak. Do not run both clocks for one session.

## Measured

One release run on this Linux x86_64 box, rustc 1.83.0, `cargo run --release`.
Not a Windows laptop run. Not an ASR or TTS latency.

`tick`, 2000 calls after 64 warm-up calls:

| op | min ns | p50 ns | p95 ns | max ns |
|---|---:|---:|---:|---:|
| empty `tick` | 22 | 24 | 25 | 106 |
| `speakable_chunks` on a two-sentence line | 1587 | 1612 | 1656 | 14371 |
| `FakeEngine::turn` | 122 | 124 | 135 | 281 |

`pace` slept until each 80 ms deadline, then called `Session::tick`. Eight
wake-to-wake gaps, microseconds: 80139, 80111, 80110, 80140, 80066, 80114,
80217, 80161. After a `hey!` turn the pad stayed at arousal 0.7, valence
0.55, dominance 0.5 across those 8 frames. `MoodTick` completed 6 frames in
a 500 ms sleep (the first frame waits 80 ms).

`say 'The kettle is on. It can wait.'` printed `engine: fake`, two chunks,
resting mood, wall 5 µs. `took_us` printed 0 because that field counts whole
microseconds.

`cargo test` passed (15 library tests, 5 integration tests).
`cargo test --features horbm` passed (17 library tests, 5 integration tests).
Clippy `--all-targets -D warnings` was clean for both feature sets. Cargo
prints `ignoring resolver config table without -Zmsrv-policy` because the
repo `.cargo/config.toml` targets rustc 1.85 and this cargo is 1.83. The
build still finished.

## Layout

| Path | Role |
|---|---|
| `src/engine.rs` | `ReplyEngine`, `Reply` |
| `src/fake.rs` | `FakeEngine` |
| `src/chunks.rs` | speakable sentences |
| `src/pad.rs` | `Pad`, resting triple, 80 ms frame |
| `src/session.rs` | `Session`, `MoodTick` |
| `src/linked.rs` | `HorbmEngine` (`--features horbm`) |
| `seam/horbm` | old type seam, not the linked brain |
| `../../horbm/crates/horbm` | linked `horbm` crate |
