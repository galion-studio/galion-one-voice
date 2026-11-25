# S62 — stale archived-crate references

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Swarm | HORBM S62. Point voice-stack comments at crates that are already archived. |
| Written | 2026-11-25. No weight download, no voice file, no paid API, no merge. |
| Base | `main` `3c6751141f510f4c6538cd1cb07bafa2f63a98b5` |
| Persona | Samantha only. Aria is not loaded. |
| Consent | `remember=deny`, `learn=deny`, `learn_in_band=false`. Not edited. Ear prosody stays on (`GALION_EAR_SENSE` is the off switch). |
| Locks | `LOCKS.toml` was not edited. `SOUL.md` is not in this tree and was not created. `LEDGER.md` was not edited. |

## What this change decides

Three crates are already off the live tree. Comments in `voice-stack/` and one preclone file still named two of them as if a programmer should import them. Those mentions now use the dated archive path. No type, constant, or manifest dependency changed.

| Crate | Former path | Archive | What runs now |
|---|---|---|---|
| `galion-v2-live` | `crates/galion-v2-live` | `archive/2026-11-24/crates/galion-v2-live` | Formulas copied into `voice-stack` (`vad`, `audio-io`, `tts-piper-stream`, `latency-harness`). The loopback server answers with `horbm::Brain::turn`. |
| seam package `horbm` | `voice-stack/horbm-bridge/seam/horbm` | `archive/2026-11-24/voice-stack/horbm-bridge/seam/horbm` | `voice-stack/horbm-bridge` links `horbm/crates/horbm`. |
| `galion-live-demo` | `crates/galion-live-demo` | `archive/2026-11-25/crates/galion-live-demo` | `horbm/crates/horbm-duplex` (`Duplex::tick`). Retired in S55, PR #317. |

`galion-live-demo` does not appear under `voice-stack/` or in `tools/cloning-v2/preclone/src/vad.rs`. There was no owned line to retarget. The live loop remains `horbm-duplex`.

## Real dependencies

None. Nothing in this change was left in place because it builds.

Searched Cargo manifests, `Cargo.lock`, `build.rs`, and `use` / `mod` paths:

- No `path =` into `crates/galion-v2-live`, `crates/galion-live-demo`, or `voice-stack/horbm-bridge/seam`.
- No live `use galion_v2_live` or `use galion_live_demo`. After this change those idents do not appear outside `archive/` and `docs/`.
- The two owned `Cargo.toml` lines that name an archived tree are comments, and they already used the archive path: `voice-stack/contract/Cargo.toml` and `voice-stack/horbm-bridge/Cargo.toml`. Not edited.
- The live bridge dependency stays `horbm = { path = "../../horbm/crates/horbm" }`.

## Changes

Comments and docs only. The code under each comment is the same.

| File | Was | Now |
|---|---|---|
| `tools/cloning-v2/preclone/src/vad.rs` | `` `crates/galion-v2-live/src/vad.rs` `` | `` `archive/2026-11-24/crates/galion-v2-live/src/vad.rs` `` |
| `tools/cloning-v2/preclone/src/vad.rs` | `` `galion_v2_live::audio::rms_db` `` | `` `audio::rms_db` in `archive/2026-11-24/crates/galion-v2-live/src/audio.rs` `` |
| `voice-stack/ARCHITECTURE.md` S01 | `` `galion_v2_live::audio` `` | linear resample in `` `archive/2026-11-24/crates/galion-v2-live/src/audio.rs` ``, copied into `voice-stack/audio-io` |
| `voice-stack/ARCHITECTURE.md` S02 | `` `galion_v2_live::vad::Vad` and `endpoint::Endpointer` `` | `voice-stack/vad`, following `Vad` and `Endpointer` in `` `archive/2026-11-24/crates/galion-v2-live` `` (`src/vad.rs`, `src/endpoint.rs`) |
| `voice-stack/ARCHITECTURE.md` S05 | `` `galion_v2_live::piper::Piper` `` | `voice-stack/tts-piper-stream`, following `Piper` in `` `archive/2026-11-24/crates/galion-v2-live/src/piper.rs` `` |
| `voice-stack/INTEGRATION.md` | `` `galion-v2-live` is a standalone workspace `` | the archive path was that workspace; the lane is archived. `galion-sam-contract` still uses that shape. |
| `voice-stack/INTEGRATION.md` | `` `galion-v2-live` is already a nested workspace `` | the archive path was a nested workspace for the same reason. The lane is archived. |
| `voice-stack/audio-io/README.md` | `` `galion-v2-live` `PIPER_HZ` ``, `` `to_mono` ``, `` `resample` `` | same three formulas, archive path |
| `voice-stack/audio-io/src/lib.rs` | `` `galion-v2-live` (`FRAME` 320 `` and `` `galion_v2_live::piper::PIPER_HZ` `` | archive path, and `PIPER_HZ` in `` `.../src/piper.rs` `` |
| `voice-stack/audio-io/src/resample.rs` | `` length in `galion-v2-live` `` | `` `archive/2026-11-24/crates/galion-v2-live/src/audio.rs` `` |
| `voice-stack/contract/src/audio.rs` | `` `galion_v2_live::endpoint::EpEv` `` | `` `endpoint::EpEv` in `archive/2026-11-24/crates/galion-v2-live/src/endpoint.rs` `` |
| `voice-stack/pipeline-orchestrator/README.md` | `` `galion-v2-live` `Vad` `` | `` `voice-stack/vad`, from `archive/2026-11-24/crates/galion-v2-live` `src/vad.rs` `` |
| `voice-stack/pipeline-orchestrator/src/clock.rs` | `` `galion-v2-live` `Vad` `` | `` `Vad` in `archive/2026-11-24/crates/galion-v2-live/src/vad.rs` `` |
| `voice-stack/pipeline-orchestrator/src/samantha_traits.rs` | `Vad::push`, `VadEv`, `audio::rms_db`, `Vad` thresholds attributed to the live crate name | the same four, each with the archive file (`src/vad.rs` or `src/audio.rs`) |
| `voice-stack/vad/src/audio.rs` | `` `galion_v2_live::audio::resample` `` and `` `rms_db` `` | `` `audio::resample` `` and `` `audio::rms_db` `` in `` `archive/2026-11-24/crates/galion-v2-live/src/audio.rs` `` |
| `voice-stack/ci/check-no-archived-crate-refs.sh` | absent | the check below |

## Already the archive path

These owned lines already named `archive/2026-11-24/...`. Left as written.

| File | What it already says |
|---|---|
| `voice-stack/audio-io/src/lib.rs` | Piper rate pair at the archived lane |
| `voice-stack/audio-io/src/resample.rs` | linear resample source file |
| `voice-stack/audio-io/src/wav.rs` | PCM16 WAV layout |
| `voice-stack/ARCHITECTURE.md` | `tts_first` note points at archived `piper.rs` |
| `voice-stack/INTEGRATION.md` | running VAD copy is `voice-stack/local-server`; original lane and Piper notes are the archive |
| `voice-stack/contract/Cargo.toml` | comment: same workspace shape as the archived lane |
| `voice-stack/horbm-bridge/Cargo.toml` | comment: old type seam is the archive path and is not linked |
| `voice-stack/horbm-bridge/README.md` | old type seam path, twice; linked brain is `../../horbm/crates/horbm` |
| `voice-stack/vad/README.md` | energy gate, endpointer, call bin, and `audio.rs` |
| `voice-stack/vad/src/lib.rs` | `call.rs` frame size and `vad.rs` energy gate |
| `voice-stack/vad/src/turn.rs` | onset, barge-in, `EndpointCfg`, unfinished-word list |
| `voice-stack/vad/src/energy.rs` | energy-gate source file |
| `voice-stack/vad/src/audio.rs` | module note on the WAV and resample source |
| `voice-stack/latency-harness/README.md` | Piper spawn flags from archived `piper.rs` |
| `voice-stack/latency-harness/src/piper.rs` | JSON-line protocol from archived `piper.rs` |

## Not edited

`voice-stack/local-server/**` has no mention of these crates. It was not opened.

Other swarms own `galion-console`, `galion-cascade`, `galion-gates`, `horbm/**`, and `galion-tensor`. No matches there except `galion-gates`, which S55 already rewrote. Those gates, and the demo scripts beside them, still say `galion-live-demo` in a sentence that records the retirement (`is archived`, `retired with`, `does not build`, or `belonged to` plus `horbm-duplex`):

- `galion-gates/lv-live-demo.sh`
- `galion-gates/run-gates.sh`
- `galion-gates/v1-demo.sh`
- `scripts/run-live-demo.sh`
- `scripts/run-v1-demo.sh`
- `scripts/run-en-cascade-demo.sh`
- `scripts/eval/symphony-heartbeat.sh`
- `scripts/v1/soft-checks.sh`

They are not Cargo dependencies. This swarm does not own them. Historical notes under `docs/` stay history. The trees under `archive/` stay the archived sources, including their package names.

Not edited: root `Cargo.toml`, `Cargo.lock`, `LEDGER.md`, `LOCKS.toml`, `SOUL.md`, `.github/workflows/*`. The check is not wired into a workflow. No new crate dependency.

## Check

`voice-stack/ci/check-no-archived-crate-refs.sh` walks `git ls-files`. It skips `archive/**` and `docs/**`. Needles are the crate basenames in `archive/**/ARCHIVED.md` (`Former path`), the underscore form of a hyphenated basename, and `<bridge>/seam` when the former path contains `/seam/`.

A tracked file fails the check when it contains a needle unless:

- that occurrence sits in one `archive/YYYY-MM-DD/` path token, or
- the line is retirement prose (`is archived`, `retired with`, `does not build`, `is not built`, or `belonged to` together with `horbm-duplex`).

A Rust ident (`galion_v2_live`, `galion_live_demo`) fails even on a retirement line. `name = "galion-live-demo"` fails. A bare `` `galion-v2-live` `` fails. The script builds its own samples from pieces, so the script source is scanned too.

```bash
bash voice-stack/ci/check-no-archived-crate-refs.sh
bash voice-stack/ci/check-no-archived-crate-refs.sh --self-test
```

## Tests

Host: rustc 1.85.0 (`4d91de4e4` 2025-02-17), Linux x86_64. `horbm` is a virtual workspace, so those lines pass `--workspace`. Toolchain flag on this machine: `cargo +1.85.0`.

| Command | Result |
|---|---|
| `bash voice-stack/ci/check-no-archived-crate-refs.sh --self-test` | exit 0. `self-test ok` |
| `cargo test --workspace` | 1062 passed, 0 failed, 5 ignored |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean |
| `cargo test --manifest-path horbm/Cargo.toml --workspace` | 267 passed, 0 failed, 2 ignored |
| `cargo clippy --manifest-path horbm/Cargo.toml --workspace --all-targets -- -D warnings` | clean |
| `cargo test --manifest-path tools/cloning-v2/preclone/Cargo.toml` | 15 passed, 0 failed |
| `cargo test --manifest-path voice-stack/Cargo.toml -p galion-sam-contract -p galion-sam-audio-io -p galion-sam-orch` | 62 passed, 0 failed |
| `cargo clippy --manifest-path voice-stack/Cargo.toml -p galion-sam-contract -p galion-sam-audio-io -p galion-sam-orch --all-targets -- -D warnings` | clean |

`voice-stack/vad` (`galion-sam-vad`) already requires rustc 1.92 because of `ort`. Its `Cargo.toml` was not edited. The two comment lines in `vad/src/audio.rs` were not compiled on 1.85. `voice-stack/Cargo.lock` is unchanged: a test resolve wanted to refresh it, and that refresh was discarded. Root `Cargo.lock` and `horbm/Cargo.lock` are unchanged.

## Linux

The new file is bash plus `git`, `grep`, and `sed`. The repo root is `dirname` of the script, two levels up. No `.exe`, no drive letter, no new address, no device open, no new dependency. ALSA and the `live` feature are untouched. Galion's host is Ubuntu; the script does not call a Linux-only binary beyond the usual coreutils those tools already use.
