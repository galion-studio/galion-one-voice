# Samantha voice stack — integration plan (Swarm 16)

SPDX-License-Identifier: MIT

Copyright (c) 2026 Galion Labs

Perpetual Open MIT License.

How the sector trees become one Cargo workspace after they exist. This file does not add that workspace. The root [`Cargo.toml`](../Cargo.toml) stays as it is: `runtimes/galion-piper-rt` remains in `exclude`, and `voice-stack/` is not a member.

[`ARCHITECTURE.md`](ARCHITECTURE.md) is the behaviour contract. [`contract/`](contract/) is the compiled type contract (`galion-sam-contract`).

## What is already wired

The companion does not get a second audio stack, a second emotion type, or a second dot.

| Need | Already in the repo |
|---|---|
| 80 ms frame, 24 kHz, 1920 samples | `LOCKS.toml`, `galion_emotion::rika::FRAME_MS` |
| Mic trait, one frame per tick | `galion_console::mic::MicSource` |
| Energy VAD and the two-stage endpointer | Running copy is in `voice-stack/local-server`. The original lane is archived at `archive/2026-11-24/crates/galion-v2-live` (`vad.rs`, `endpoint.rs`). |
| Vosk, out of process | `crates/galion-asr-vosk` |
| Warm Piper, JSON line in, PCM out | Running child is `voice-stack/tts-piper-stream` plus `runtimes/galion-piper-rt`. The old lane's notes are `archive/2026-11-24/crates/galion-v2-live/src/piper.rs`. |
| Prosody knobs and the Samantha profile | `galion-emotion` `ProsodyControl`, `SAMANTHA` |
| Consent, default deny | `galion-emotion` `ConsentGate`, `EmotionWire` |
| One dot | `galion-render` `DotState`, `LOCKS.toml` `[ui]` |
| Core pins | `crates/galion-cores` |
| Turn beside the heartbeat, not inside it | `crates/galion-openmind` (the rule; the brain here is HORBM, not the Hermes handoff) |
| Stage timer | `galion_console::live::StageTimer` |

`galion-v2-live` is a standalone workspace (empty `[workspace]` table) and is not a root member. `galion-sam-contract` uses that same shape so `cargo test --workspace` at the root does not build it yet.

## Crate names

One package per sector. Directory is `voice-stack/<sector>`. Package name is `galion-sam-<sector>` with the sector's hyphens. Library crate name follows Cargo's underscore rule.

| Sector | Package |
|---|---|
| contract (this pull request) | `galion-sam-contract` |
| audio-io | `galion-sam-audio-io` |
| vad | `galion-sam-vad` |
| asr | `galion-sam-asr` |
| horbm-bridge | `galion-sam-horbm` |
| tts-piper-stream | `galion-sam-tts` |
| samantha-voice-clone | `galion-sam-clone` |
| emotion-prosody | `galion-sam-prosody` |
| emotion-in | `galion-sam-emotion` |
| pipeline-orchestrator | `galion-sam-orch` |
| local-server | `galion-sam-server` |
| one-dot-ui | `galion-sam-dot` |
| latency-harness | `galion-sam-latency` |
| eval-golden | `galion-sam-eval` |

S14 and S15 are scripts and a unit template, not crates, unless they grow a real package. They do not get an empty library just to appear in `members`.

Each `Cargo.toml` carries `license = "MIT"`, `publish = false`, `rust-version = "1.85"`, and the SPDX line. A new `LICENSE` file is the repository `LICENSE` copied byte for byte, and only if that sector is split out as its own tree. Prefer the repo `LICENSE` and do not add a second copy.

## Workspace file, later

When the core crates exist on disk (audio-io, vad, asr, horbm-bridge, tts-piper-stream, emotion-prosody, emotion-in, pipeline-orchestrator), a follow-up draft pull request adds `voice-stack/Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = [
  "contract",
  # plus each sector directory that is actually present
]
exclude = []

[workspace.package]
version = "0.0.1"
edition = "2021"
license = "MIT"
publish = false
rust-version = "1.85"
```

That follow-up is a draft. It is not merged from this sector. It does not edit the root `members` list.

Why a nested workspace, not a root member:

- Root `exclude` exists so `cargo tree --workspace` stays free of `ort`. `galion-piper-rt` loads onnxruntime in its own process (`LOCKS.toml` `formats.exceptions.a21_piper`). Joining the voice stack to the root workspace before every member is checked would put that question on the whole tree.
- `galion-v2-live` is already a nested workspace for the same kind of reason (it depends on `serde_json` and is not a root member).

Before the follow-up lists a member, this command is run on that member's `Cargo.toml` and must print nothing:

```bash
grep -RInE --include=Cargo.toml -e '^\s*(candle[-a-z]*|ort|tract[-a-z]*|burn[-a-z]*|llama[-a-z_]*|ggml[-a-z]*)\s*=' voice-stack
```

`horbm-memory` is a git dependency of `galion-sam-horbm` only:

```toml
horbm-memory = { git = "https://github.com/galion-studio/galion-knowledge", rev = "761a7f225304fc010379219e5b88c6669dd48f40" }
```

The follow-up does not add that line until the bridge crate exists and its manifest has been read. If that revision does not resolve, the bridge pull request says so. It does not pick a newer revision silently and it does not reimplement the crate.

Path dependencies inside the nested workspace point at `galion-emotion` and `galion-cores` the way `contract` already points at `galion-emotion`. They do not point at `galion-console` unless the sector is ready to build the terminal UI. `MicSource` can be implemented beside the contract's `AudioIn` (same three methods) without that dependency.

Sectors opened in parallel will not have `voice-stack/contract` on their branch until this pull request is merged, and this sector does not merge it. Until then they match the signatures in `ARCHITECTURE.md` and in `contract/src`. They do not invent a second `TurnOut` field list. The workspace pull request is what makes the path dependency real.

## Build order

The orchestrator is the only crate that calls the others. Everyone else can land against the traits.

1. `galion-sam-contract` — this pull request. `cargo test --manifest-path voice-stack/contract/Cargo.toml`.
2. S01, S02, S03, S05, S07, S08 — devices and pure stages. Each exposes the matching trait and a fake that returns a fixed frame or a fixed transcript. The fake is labelled a fake. It is not a transcript of someone who did not speak.
3. S04 — `Brain` wrapper. `cargo check` of this crate is allowed to fail while `horbm` is unpublished. The pull request says whether `cargo check` ran. A stub that returns a fixed sentence is not a brain and does not satisfy `may_speak_piper` by inventing warmth.
4. S10 and S11 — socket and dot, fed by the fakes.
5. S09 — connects the traits. `Speculate` does not write. `Commit` calls `turn`. `may_speak_piper` gates `Piper::say`. `tick` runs on the frame. Barge-in calls `stop`.
6. S12 — reads `STAGE_BUDGETS` and prints samples. No pass/fail banner for the product.
7. S13 — text cases: empty reply stays silent, `violations > 0` stays silent, a stock comfort line stays silent, `sense = false` yields no inbound frame, a non-loopback bind is refused.
8. S06 — clone path, still not pinned on a live call.
9. S14, S15 — install notes. `GALION_HOME` as in the architecture. No audio copied into git. No unit installed.

Check, once the workspace file exists and the members on disk are the ones listed:

```bash
cargo test --manifest-path voice-stack/Cargo.toml --workspace
cargo test --manifest-path voice-stack/contract/Cargo.toml
```

The root suite is unchanged and still does not include this tree:

```bash
cargo test --workspace
```

Do not add a GitHub workflow in the first workspace pull request. A later workflow may run the nested `cargo test` on a pull request. It does not download weights, call a model API, or deploy.

## Runtime shape

One local process, started by hand, not by the stage unit.

```text
galion-sam-orch
  GALION_HOME          data root (default G:\galion on Windows)
  GALION_SAMANTHA_WS   default 127.0.0.1:7881, loopback only
  GALION_CORE_MAP      passed through to galion-cores
  GALION_REQUIRE_SESSION   same meaning as the phone path
```

Children, all on loopback or stdio, all CPU:

- `galion-asr-vosk` on core 1, spawned the way the phone lane already spawns it.
- Piper on core 6, the `galion-piper-rt` binary when it is on disk, legacy CLI only when `GALION_PIPER_LEGACY=1`. Missing binary is an error, not Flite pretending to be Samantha, unless the operator set the existing `GALION_SPEECH` fallback and the UI's backend label says `flite`.
- No clone child while a turn is live.
- The orchestrator does not dial Hermes. The layer 11 gateway is the only voice-stack client of Hermes, and only at `127.0.0.1:8642` (`/v1/chat/completions`). It does not dial 8787, a remote host, or a paid API.
- Galion Research, layer 11, listens on `127.0.0.1:7911` (`GALION_RESEARCH_BIND`). That is not 8787, which is the Node desk and clashes with the HORBM preview. The layer 5 router in `galion-sam-horbm` calls 7911 only after a card miss, and only for a deep question, with a 3 second cap.

The one-dot page can stay on `127.0.0.1:7878` and open this socket as a second connection, or S11 can be a static page served only as an asset. It does not add a control beside the dot. Dot state arrives as a `dot` text frame. The page already knows `idle`, `listening`, `speaking` from `galion_render::dot::DotState`.

## What this plan refuses to do

- Merge a sector pull request, force-push `main`, or edit branch protection.
- Put `ort` or the Piper runtime into the root workspace.
- Point the orchestrator at Hermes when `horbm` is missing.
- Commit a weight, a reference clip, or a generated voice.
- Treat `STAGE_BUDGETS` as a benchmark that has been run.
