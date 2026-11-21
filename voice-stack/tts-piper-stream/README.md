<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright (c) 2026 Galion Labs -->

# S05 tts-piper-stream

Streaming CPU TTS fallback for Samantha. One warm Piper process, clause by clause, stock female voice. This is not Samantha's voice and not a clone.

The mouth contract is `Tts` in this crate. Sector S06 (the clone) implements the same trait. `PiperTts` speaks when the clone is absent. `FakeTts` is the in-process stand-in for tests. This crate does not open `GALION_SAMANTHA_REF` and does not load a reference clip.

Piper is the same out-of-process path as `galion-ml-voice` `PiperWorker`: `piper --json-input --output_raw`, one JSON line in, raw s16le out, then `Real-time factor:` on stderr. The voice is the pinned pack `en_US-ljspeech-medium` (LJ Speech, public domain, 22050 Hz). Runtime order matches `weights.rs`: `$GALION_A21_RUNTIME`, else `galion-piper-rt` if that binary is installed, else the legacy Piper 2023.11.14-2 CLI. `GALION_PIPER_LEGACY=1` skips the rt binary.

Clauses use the same boundaries as `crates/galion-ml-voice/src/speech/clause.rs`: `.` `!` `?` `,` `;` or a dash, or 8 closed words. Decimals are not cuts. `[laugh]` and `[chuckle]` are removed before synthesis (`samantha.toml` keeps tags off Piper).

The next clause is written to the warm process before `on_pcm` runs for the current one, so playback can start while the next clause renders. Piper VITS still emits a whole clause in one shot. There is no audio until that clause is finished.

Mood is a live `Pad` (`arousal`, `valence`, `dominance`). Resting is `{0.3, 0.25, 0.5}` and is a sample copy. Other values move pitch, gain, and a leading pause on the PCM. Piper reads length and noise once at spawn, so a mood change does not restart the process. `GALION_A21_NOISE=natural` is the only spawn-time noise switch, same as `PiperProsody::from_env`.

`frames_80ms` resamples to 24 kHz and slices 1920-sample frames (80 ms), the same sizes as `galion-heartbeat`. A short tail has `filled < 1920` and the rest zero. That tail is a pause, not a stretched frame. This crate does not open a sound device and does not install a systemd unit. The one-dot page stays `crates/galion-console/assets/v1-screen.html`.

This directory is its own Cargo workspace. It is not a member of the repo root workspace.

## Linux

From this directory:

```bash
bash scripts/fetch-voice.sh
cargo test
cargo run --release --bin piper-ttfa
```

`fetch-voice.sh` checks sha256 and will not keep a bad file. `--check` verifies without the network. The voice and the `piper` binary land in `var/`, which is gitignored. Do not commit `var/`, onnx files, or wavs.

Override paths if you already have the pack:

```bash
export GALION_A21_RUNTIME=/path/to/piper
export GALION_PIPER_MODEL=/path/to/en_US-ljspeech-medium.onnx
export GALION_PIPER_CONFIG=/path/to/en_US-ljspeech-medium.onnx.json
cargo run --release --bin piper-ttfa
```

Optional: `GALION_PIPER_CPUS=0-3` pins the child on Linux. Unset leaves the mask alone. The child inherits `GALION_TTS_DEVICE`. `galion-piper-rt` reads it. CPU is the default. `cuda` uses the CUDA execution provider only when that provider loads, and otherwise logs `falling back to CPU`. The legacy Piper CLI ignores the variable and stays on CPU. See [`voice-stack/mouth/README.md`](../mouth/README.md). `piper-ttfa --wav /tmp/out.s16` writes raw s16le of the two-clause line for listening. Do not commit that file.

Linux reads the pipes with `poll` (no extra drain wait). `GALION_PIPER_IO` is not a switch; tests also run the reader-thread path (`IoMode::Thread`).

## Windows

From this directory, in PowerShell:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\fetch-voice.ps1
cargo test
cargo run --release --bin piper-ttfa
```

The script downloads the same voice pins and `piper_windows_amd64.zip` from the Piper 2023.11.14-2 release (sha256 `f3c58906402b24f3a96d92145f58acba6d86c9b5db896d207f78dc80811efcea`). The binary is `var\piper\piper.exe`. Windows uses a reader thread and a 2 ms drain confirm after each clause so a flush that is still in the other thread is not cut off. That 2 ms is not part of `submit_to_first_pcm`. The Windows path is compiled in this tree and was not executed on the Linux host that produced `MEASURED.json`.

The same `GALION_A21_RUNTIME`, `GALION_PIPER_MODEL`, and `GALION_PIPER_CONFIG` variables work.

## What the bench prints

`piper-ttfa` prints one JSON object and exits 0 only when a real child produced audio.

- `cold_spawn_to_first_pcm_ms`: process spawn through model load and the first PCM byte of `Hello.`
- `cold_submit_to_first_pcm_ms`: the first JSON line through the first PCM byte (still includes model load if the child has not finished starting)
- `warm_submit_to_first_pcm_ms`: five more `Hello.` lines on that same process, plus the median
- `warm_under_80ms`: whether that median is at most 80 on this host
- `piper_rtf`: the child's `Real-time factor` (infer time / audio time)
- `wall_rtf`: wall time of `speak` divided by the shaped audio duration. It includes the pipe handoff
- `stream_second_submitted_before_callback`: the second clause was written before `on_pcm` for the first

Exit 2 means the voice or runtime is not on disk. The JSON then has `"measured": false` and no timings. A missing pack is not a number.

## Measured on the build host

`MEASURED.json` is the stdout of `cargo run --release --bin piper-ttfa` on 2026-11-21. Intel Xeon, 4 CPUs, load `0.06 0.04 0.01`, Linux `poll` path, Piper 2023.11.14-2, `en_US-ljspeech-medium`. The owner's Windows laptop was not measured.

- Cold spawn to first PCM of `Hello.`: **328 ms**, including model load. The child's own RTF on that line was **0.0354**. `cold_wall_rtf` **0.6284** is that load plus synthesis over 522 ms of audio, not the warm factor.
- Warm submit to first PCM, five reps on the same process: **18, 17, 18, 18, 17 ms**. Median **18 ms**, which is under 80 ms on this host. Warm child RTF median **0.0336**. `Hello.` was 11520 samples, 522 ms.
- `I am here. I can hear you.` (two clauses): first PCM **24 ms**, second clause submitted **24 ms**, first callback **24 ms**. Those three stamps share one millisecond. The session writes clause 2 before the callback; `tests/warm_session.rs` locks that order. Wall for both clauses **58 ms**. 22 full 80 ms frames and a 1785-sample tail.
- A throwaway s16 dump of that stream (deleted, not committed) peaked at 32767 with RMS about 3581, so the buffer was not silence.

Windows adds up to 2 ms of drain confirm after each clause. That wait is not part of `submit_to_first_pcm`.

## Library

```rust
use galion_tts_piper::{frames_80ms, Pad, PiperTts, Tts};

let mut mouth = PiperTts::open_resolved()?;
let mut played = Vec::new();
let report = mouth.speak("I am here. I can hear you.", Pad::RESTING, &mut |chunk| {
    played.extend_from_slice(chunk.samples); // copy; the slice ends with the callback
})?;
let frames = frames_80ms(&played, report.sample_rate);
```

`on_pcm` should enqueue and return. Blocking inside it holds the next handoff. S06 depends on this crate and implements `Tts`; it does not have to call Piper.

```toml
galion-tts-piper = { path = "../tts-piper-stream" }
```

## Tests

`cargo test` uses `fake-piper`, a tiny process that speaks the same pipe protocol. It does not load the onnx pack. `cargo test` is green without a download. The neural timings come only from `piper-ttfa`.

## Not in this sector

The phone lane is not rewired to this crate. Nothing here plays the speaker, serves the one-dot page, or installs a unit. `galion-piper-rt` is used when its binary is already installed; this sector does not build onnxruntime. A dead child is dropped and the next `speak` starts a new one, which is cold again. Pitch on a non-resting Pad shortens the buffer by the resample ratio, because length scale stays at the warm process's spawn value.
