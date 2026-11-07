# Compiler

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-03. Synthesis note 2026-11-06. Symphony section. Index: [README.md](README.md).

The spelling is **Galion**. The product is Galion Voice.

Master slogan: "The seamless connection with the human being, on the voice only."

Synthesis is the Rust stack that reads the Galion voice file. The compiler writes that file from clone PCM. `galion-ml-voice` opens it. `galion-heartbeat` frames it at 24 kHz, 80 ms, 1920 samples. The file is a `.gq` clip: magic `GALIONGQ`, the persona, the rate, and a sha256 sidecar. Chatterbox stays the out-of-process CPU child that can emit PCM. This compiler is the next step. It starts no PyTorch process and it downloads no weights.

## Piper and flite

Piper is the sunset out-of-process pin. `LOCKS.toml` `[formats.exceptions.a21_piper]` names the pack `weights/tts/en_US-ljspeech-medium.onnx`, the runtime `galion-piper-rt` (separate process, its own cargo workspace), and `sunset = "2027-02-01"` (decision t123u, widened once as t613s2992 for onnxruntime 1.31.1 inside that process). The legacy piper CLI (2023.11.14-2) is the pin's own fallback under `GALION_PIPER_LEGACY=1`. ONNX Runtime stays inside the Piper process. This page leaves the lock as it stands. It does not train Piper and it does not widen the pin to ASR or to a second TTS.

Flite is the fallback when the pack is absent. `SpeechStack::detect` (`crates/galion-ml-voice/src/speech/mod.rs`) reads `GALION_SPEECH`:

| `GALION_SPEECH` | Who speaks |
|---|---|
| `auto` (default) | Clone slot when that slot can speak. Otherwise Piper when the A21 pack is on disk. Otherwise flite. |
| `clone` | Clone slot when it can speak. Otherwise Piper, then flite, when the pack is absent. |
| `a21` | Piper when the pack is on disk. With the pack absent the slot is `SpeechBackend::None`. |
| `flite` | `flite::FliteTts` (`system-flite`). libflite is opened with `dlopen`. Default voice `slt`, or `GALION_FLITE_VOICE`. |
| `off` | `SpeechBackend::None`. |

Flite resamples its 16 kHz wave to 24 kHz. The label is `system-flite`. A flite utterance is a system voice. The compiled `.gq` clip is the Galion voice file.

`GALION_NATIVE_DECODER_WIRED` stays false. A future A21 `.gq` decoder pack stays inactive. `GALION_TTS_WEIGHTS` points at a Piper pack directory, which is a different path from `voices/compiled`.

## Gate

**PASS** when a fixture compiles and a round-trip reads the same PCM and the same persona id back. The file that round-trip opens is the Galion voice file. A run that already happened is a row in [SELF-EVAL.md](SELF-EVAL.md). This page adds no row.

**FAIL** when the round-trip drops samples, changes the persona id, or the Rust path opens a Chatterbox container, a Piper ONNX pack, or a WAV as the Galion voice file.

**BLOCKED** until a measured run is written in [SELF-EVAL.md](SELF-EVAL.md). Unmeasured rows stay **BLOCKED**. This landing does not close them:

| Gate | State |
|---|---|
| A persona `.gq` committed in the tree | **BLOCKED**. `voices/compiled/` is gitignored. `galion-student/` is the distillation plan. |
| Live Chatterbox `generate` for Samantha or Jarvis | **BLOCKED**. Weights are not in git. The fixture uses synthetic PCM in the worker's s16le 24 kHz shape. |
| Tick on an EPYC 9655P-class host, a CX23, or a CX43 | **BLOCKED**. Those machines are not in this landing. |
| MOS | **BLOCKED**. No MOS number is published here. |
| TTFA | **BLOCKED**. No TTFA number is published here. |
| Speaker similarity | **BLOCKED**. No similarity number is published here. |
| Film audio | **BLOCKED**. No film audio is fetched. |

The non-inference tick budget is 1% of the 80 ms frame: 800 µs. `fill_frame` is the tick this compiler contributes. A clock from a run belongs in [SELF-EVAL.md](SELF-EVAL.md). This page prints none.

## Where it sits

| Piece | Path |
|---|---|
| Heartbeat frames | `crates/galion-heartbeat` (24 kHz, 80 ms, 1920 samples) |
| Compiler writer | `crates/galion-ml-voice` (`artifact`, bin `galion-voice-compile`) |
| Reader | `crates/galion-tensor/src/gq.rs` (`GqFile::open`, reader-only) |
| Speech selection | `crates/galion-ml-voice/src/speech/mod.rs` |
| Piper pin | `LOCKS.toml` `[formats.exceptions.a21_piper]`, `crates/galion-ml-voice/src/speech/piper_worker.rs` |
| Flite fallback | `crates/galion-ml-voice/src/speech/flite.rs` |
| Student plan | `galion-student/` |
| Clone child | [CLONE.md](CLONE.md), [CLONE-WORKER.md](../CLONE-WORKER.md) |
| Store | [CLIP-STORE.md](CLIP-STORE.md) |

A compiled file names its persona (`samantha` or `jarvis` for the test voices) and carries the sha256 of the reference it was built from. The Jarvis file keeps the hub texture. The compiler does not denoise. See [JARVIS-TEXTURE.md](JARVIS-TEXTURE.md). This page does not change Jarvis.

A clone utterance can also land as `voices/gq/<persona>.gqvoice` (magic `GQVOICE`) or a `.gvc` (magic `GALIONVC`). Those are Galion packets the speech stack can read. They are a different container from this `.gq` clip, and they are a different file from the Piper ONNX pack.

Record a measured gate in [SELF-EVAL.md](SELF-EVAL.md).

## Cut

The symphony plays the Galion voice file. The compiler's container is `.gq` v1: four tensors, 24 kHz, 80 ms frames. Load once (mmap, prefault). The tick copies one frame. The fixture is `scripts/symphony/self-eval-a.sh`. A miss fails the section. Chatterbox stdout, when the child runs, is s16le at 24 kHz and is compiler input.

## Runtime path

```text
voices/compiled/<persona>.gq
voices/compiled/<persona>.gq.sha256
```

`<persona>` is `samantha`, `jarvis`, or another id of 1..=15 characters from `[a-z0-9_-]`, starting with a letter. `$GALION_VOICE_DIR` replaces `voices/compiled`. The file stem matches the persona stored in the clip.

`galion-tensor` stays reader-only. The writer lives in `crates/galion-ml-voice` (`artifact`). `GqFile::open` with `Prefault::Populate` is the load. `VoiceClip::fill_frame` is the tick.

## File

Magic `GALIONGQ`, version 1, alignment 64. Same header the reader already checks. Four f32 tensors, lexicographic:

| Tensor | Shape | Meaning |
|---|---|---|
| `voice.clock` | `[4]` | `u32` bits: rate, `frame_ms`, `n_samples`, channels |
| `voice.flags` | `[4]` | `u32` bits: `cpu_only=1`, `gpu=0`, `chatterbox_runtime=0`, `galion_clip=1` |
| `voice.id` | `[16]` | persona, one byte per `u32`, then zeros |
| `voice.pcm` | `[n_frames, 1920]` | mono f32 in [-1, 1]. Tail after `n_samples` is silence |

Integers are `f32::from_bits`. `.gq` v1 has no u32 dtype, and this clip adds none. Rate is 24000. Frame is 80 ms. 1920 samples. That is `galion_heartbeat::{SAMPLE_RATE, FRAME_MS, FRAME_SAMPLES}` and `LOCKS.toml` `frame_ms`.

Each tensor carries its own SHA-256 in the table. The sidecar is the whole-file digest, coreutils shape (`<hex>  <file>`). `open_runtime` requires a sidecar, a matching digest, a stem that matches the persona, `gpu = 0`, and `chatterbox_runtime = 0`.

Write is `<path>.tmp`, fsync, rename. Same for the sidecar.

## Inputs

| Input | What it is |
|---|---|
| `.wav` | 16-bit PCM, mono or stereo. Rate is in the header. |
| `.pcm` `.s16le` `.raw` | Raw s16le. `--rate` default 24000, `--channels` default 1. |
| `.gci` | Galion clone intermediate. Magic `GCLONINT`. Compiler input only. |

Stereo is averaged per frame. A click on one channel stays in the mix. Resample to 24 kHz is linear. There is no noise gate and no high-pass. Jarvis hub texture stays in the samples.

`open_runtime` accepts a compiled `.gq` clip. Compiler input suffixes it leaves closed are `.wav`, `.gci`, and raw PCM. Refused as input and as output: `.safetensors`, `.pt`, `.pth`, `.onnx`, `.bin`, `.ckpt`, `.npz`, `.npy`, `.ggml`, `.mp3`, `.flac`, and a GGML-family magic. A safetensors blob renamed to `.gq` is refused. A `.gq` clip is already the runtime file, so the compiler does not take one as clone output.

Cap: 256 MiB in, 15 minutes at 24 kHz out.

## Run

Rust 1.85, from the repo root.

```text
cargo run -p galion-ml-voice --bin galion-voice-compile -- \
  --persona samantha \
  --input clone.wav \
  --out voices/compiled/samantha.gq
```

`scripts/clone/compile-voice.sh` is that command. Exit 0 written, 2 bad input or persona, 3 refused container, 1 I/O.

Load from Rust:

```text
let clip = galion_ml_voice::artifact::open_runtime(path)?;
let mut frame = [0f32; galion_ml_voice::artifact::FRAME_SAMPLES];
match clip.fill_frame(index, late, &mut frame) {
    FramePull::Sound => { /* play frame */ }
    FramePull::Paused => { /* silence; pass the same index next tick */ }
}
```

`fill_frame` does not allocate and does not read the clock. A late flag, a short buffer, or an index past the end writes silence and returns `Paused`. The index stays put. Call it outside `Heartbeat::step`. The step still does not touch disk.

## CPU

CPU only. No GPU buffer in the clip (`voice.flags` gpu = 0).

Hosts this file is shaped for, from the CPU foundation note (capacity, pending a measurement on those machines):

- Flagship app/voice node: AMD EPYC 9655P-class, 96 cores / 192 threads
- Plus: CX23
- Ultra: CX43

A compiled utterance is f32 PCM. One second is 24 000 samples, 96 000 bytes. The CPU foundation's voice-session RAM figure is about 0.65–1.15 GiB, and it is a plan. This crate does not measure a hosted voice session. The host rows above stay **BLOCKED** as measurements.

## Audio stays off git

Compiled clips stay on the machine that built them.

| Path | Git rule |
|---|---|
| `voices/compiled/` | `*.gq`, `*.gq.sha256`, `*.tmp` ignored |
| `voices/gq/` | Landed `.gqvoice` packets ignored |
| `weights/tts/` | Piper pack fetched by `scripts/a21/fetch-a21.sh`, not vendored |
| `weights/clone/` | Chatterbox pins, not vendored |

This page adds no `.wav`, `.pcm`, `.mp3`, `.flac`, `.ogg`, `.gq`, or `.onnx`.

## Credits

Puzzle pieces, named so the licence stays visible. Nothing below is linked except the in-repo crates.

| Piece | Licence | What this compiler does with it |
|---|---|---|
| `.gq` v1 reader, `crates/galion-tensor/src/gq.rs` | Perpetual Open MIT, (c) 2026 Maciej Grajczyk | The runtime reader. The clip writer in this crate follows that layout. `galion-tensor` grows no writer. |
| SHA-256 in that same reader | FIPS 180-4, in-repo MIT | Tensor digests and the sidecar. No third-party hash crate. |
| WAV PCM walk, same subset as `galion-render/src/wav.rs` | In-repo MIT | Reimplemented here so the voice crate does not depend on the render crate. Public RIFF/WAVE layout. |
| Linear resample, same idea as `galion-render` `resample_linear` | In-repo MIT | Reimplemented. Samples stay as mixed. |
| safetensors header (u64 length + JSON `"dtype"`) | Hugging Face, Apache-2.0 | Recognized only to refuse it. No safetensors crate. |
| Chatterbox | Resemble AI, MIT (read 2026-11-03) | The worker may emit PCM. Weights stay out of process. This file is the Galion clip. |
| Piper `en_US-ljspeech-medium` | rhasspy/piper-voices, MIT; pin in `LOCKS.toml` | Sunset out-of-process pin until 2027-02-01. The compiler does not shell out to it. |
| Flite (CMU `libflite`) | BSD-style, distro `libflite1` | Fallback when the Piper pack is absent. `dlopen` at runtime. Nothing vendored. |
| GGML-family container | Unused here | Magic is refused. Voice does not read or write that container. The datacenter note's LLM sizing is a capacity reference only. |

## What this section does not do

- It does not edit `LOCKS.toml`. `reference_clips` stays as the lock left it. The Piper sunset date stays `2027-02-01`.
- It does not run Chatterbox `generate`. That is the clone worker, and only when the pinned weights are on disk.
- It does not train Piper, fetch the A21 pack, or link ONNX Runtime into a Galion workspace crate.
- It does not turn Nano/Turbo safetensors into a neural decoder. `GALION_NATIVE_DECODER_WIRED` stays false. The symphony plays frames.
- It does not publish a MOS, a TTFA, or a similarity number. Those gates stay **BLOCKED**.
- It does not commit audio.
- It does not claim an EPYC or CX23/CX43 timing capture. See [SELF-EVAL.md](SELF-EVAL.md) for any clock a real run already wrote.

— Maciej Grajczyk

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
