<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright (c) 2026 Galion Labs -->

# S02 — voice activity and end of turn

Perpetual Open MIT License. (c) 2026 Galion Labs.

CPU voice activity detection for Samantha. Package `galion-sam-vad`, directory `voice-stack/vad`, the path in `voice-stack/INTEGRATION.md` on the architecture branch. Twenty millisecond frames go in. Speech start, speech end, and barge-in come out. The score is Silero VAD v5 on the CPU, through the same `ort` 2.0.0-rc.15 pin as `runtimes/galion-piper-rt`. If that model is missing or will not load, the score is the energy gate from `archive/2026-11-24/crates/galion-v2-live/src/vad.rs`.

This crate is its own Cargo workspace. It is not a member of the repo root workspace. `ort` stays inside this workspace, the same way the Piper runtime keeps it out of the root. `rust-version` is 1.92 because that `ort` pin will not compile on 1.85.

## What it reuses

| Already in the repo | What this crate keeps |
| --- | --- |
| `archive/2026-11-24/crates/galion-v2-live/src/vad.rs` | Noise floor −60 dB, +12 dB margin, −48 dB clamp. Onset 60 ms. |
| `archive/2026-11-24/crates/galion-v2-live/src/endpoint.rs` | Speculate at 60 ms, commit a finished sentence at 160 ms, hold an unfinished word for 500 ms. Same unfinished-word list. |
| `archive/2026-11-24/crates/galion-v2-live/src/bin/call.rs` | 20 ms frames. 24 kHz ear resampled to 16 kHz. Barge-in once 80 ms of consecutive overlap has built up while she is speaking. The call's own VAD hang is 300 ms (`Vad::new(300.0)`); set `GALION_VAD_HANG_MS=300` to use that. |
| `archive/2026-11-24/crates/galion-v2-live/src/audio.rs` | WAV layout and the linear resample formula. |
| `crates/galion-heartbeat` | The 80 ms heartbeat. Four VAD frames are one heartbeat. This crate stamps the index. It does not call `Heartbeat::step`. |
| `GALION_SILERO_VAD` | Same environment variable the articulation worker already uses for a local Silero file. |
| one-dot | [`FrameReport::level`](src/lib.rs) is a 0..1 speech probability a later `/dot` feed can paint. This crate does not serve the page. |
| `deploy/systemd` | Unchanged. No unit file is added. |

With no transcript yet, the hangover is 500 ms (the hold), not 160 ms. A 160 ms cut is only safe once the words say the sentence is finished. A breath under half a second does not end Mate's turn.

Barge-in fires once per turn, at 80 ms of consecutive voiced overlap, and only if his turn started while `me_speaking` was already true. That flag is the same one the heartbeat carries for the me band. This crate does not hear Samantha's voice in the microphone and it does not duck the speaker. Duck at 80 ms, pause near 200 ms, and yield at 800 ms stay with the call loop.

Nothing here speaks. It does not open `GALION_SAMANTHA_REF`. It does not load a GPU execution provider.

## Requirements

- Rust 1.92 or newer. `ort` 2.0.0-rc.15 requires it. This tree was built with rustc 1.100.0.
- A C linker. ONNX Runtime links `libstdc++`.
- Network on the first build. The `ort` crate downloads the CPU ONNX Runtime library. No model API is called.
- The fetch script downloads the Silero model (MIT) and five CC0 speech clips. Those files are gitignored.

Set these before the process starts. The prebuilt ONNX Runtime uses OpenMP and ignores the intra-op thread count, so without them the VAD can fan out across every core:

```
OMP_NUM_THREADS=1
OMP_WAIT_POLICY=PASSIVE
```

## Linux

From the repo root:

```bash
cd voice-stack/vad
bash scripts/fetch-cc0.sh
export OMP_NUM_THREADS=1 OMP_WAIT_POLICY=PASSIVE
cargo test
cargo run --release --bin vad-bench -- --out measured.json
```

Ubuntu needs the libstdc++ linker input (`libstdc++.so`), which the `libstdc++-14-dev` or `g++` package provides. The shared library `libstdc++.so.6` is what the binary loads at run time.

`cargo test` passes with no downloads: the turn rules and the energy gate use synthetic frames. When `samples/` and `models/silero_vad.onnx` are present, the same `cargo test` also runs the five CC0 clips through both backends.

## Windows

The checkout on the owner's laptop lives under `G:\galion`. From the repo root:

```powershell
cd voice-stack\vad
powershell -ExecutionPolicy Bypass -File scripts\fetch-cc0.ps1
$env:OMP_NUM_THREADS = "1"
$env:OMP_WAIT_POLICY = "PASSIVE"
cargo test
cargo run --release --bin vad-bench -- --out measured.json
```

Use Rust 1.92 or newer (`rustup toolchain install stable`). The first build downloads the Windows ONNX Runtime DLL. No service is installed. This sector was not run on Windows in the session that wrote `MEASURED.md`.

## Environment

| Variable | Default | Meaning |
| --- | --- | --- |
| `GALION_SILERO_VAD` | `models/silero_vad.onnx` | Silero v5 ONNX file. v4 (`h`/`c` states) is refused. |
| `GALION_VAD_SAMPLES` | `samples/` | Directory of 16-bit PCM wavs for the bench. |
| `GALION_VAD_ONSET_MS` | 60 | Sustained voice before speech start. |
| `GALION_VAD_HANG_MS` | 500 | Silence that ends a turn when no transcript is available. |
| `GALION_VAD_SPEC_MS` | 60 | Silence before a speculate event, transcript required. |
| `GALION_VAD_COMMIT_MS` | 160 | Silence that ends a finished sentence. |
| `GALION_VAD_HOLD_MS` | 500 | Silence that ends an unfinished word (`and`, `the`, `um`, …). |
| `GALION_VAD_THRESHOLD` | 0.5 | Probability that opens a turn. |
| `GALION_VAD_KEEP` | 0.35 | Probability that keeps a turn open. |
| `GALION_VAD_BARGE_MS` | 80 | Consecutive overlap before a barge-in event. |

`Listen::open` falls back to the energy gate and records why. `Listen::silero` and `vad-bench` do not fall back: a missing model is an error, so an energy run is never reported as Silero.

## Library

```rust
let mut ear = galion_sam_vad::Listen::from_env();
let report = ear.push(&frame_320, me_speaking, transcript)?;
```

`frame_320` is 20 ms of mono s16 at 16 kHz. `frame_from_24k` converts the 480-sample frame the live call already uses. `report.event` is `SpeechStart`, `SpeechEnd`, `Speculate`, `Resume`, `Hold`, or `None`. `Event::endpoint_name` is the `EndpointEvent` spelling in `galion-sam-contract`: start, commit, speculate, resume, hold. `SpeechEnd` is that contract's `Commit`. `report.barge_in` is independent, so it can share a frame with speech start. `report.level()` is the dot level. `report.cpu_ns` is thread CPU time for that frame.

## Samples

`scripts/sources.tsv` lists every URL, sha256, and license. Five Lingua Libre clips from Wikimedia Commons, each CC0-1.0, plus Silero VAD v5.1.2 (MIT). Not Samantha, not Jarvis, not s711, not the owner's reference clip.
