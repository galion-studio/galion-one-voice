# Compiler

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-03. Symphony section. Index: [README.md](README.md).

The spelling is **Galion**. The product is Galion Voice.

The compiler takes the clone the Chatterbox child produced and writes the Galion voice file the Rust symphony runs. That file is the `.gq` student path and the Galion clip ABI: PCM the heartbeat can frame, plus the small header the stack needs to find the persona, the rate, and the hash.

Chatterbox stays the out-of-process CPU child that knows its own weights. The file that lands in the clip store, and the file `galion-heartbeat` and `galion-ml-voice` read, is Galion's.

## Gate

**PASS** when a fixture compiles and a round-trip reads the same PCM and the same persona id back. The primary path recorded in the fixture is the Galion voice file.

**FAIL** when the round-trip drops samples, changes the persona id, or leaves the Chatterbox container as the file the Rust path opens.

**BLOCKED** when there is no compiled artifact yet. That is the state of this landing: `galion-student/` is the distillation plan, and a `.gq` voice file is not in the tree.

## Where it sits

| Piece | Path |
|---|---|
| Heartbeat frames | `crates/galion-heartbeat` (24 kHz, 80 ms, 1920 samples) |
| Speech slot | `crates/galion-ml-voice` |
| Student plan | `galion-student/` |
| Clone child | [CLONE.md](CLONE.md), [CLONE-WORKER.md](../CLONE-WORKER.md) |
| Store | [CLIP-STORE.md](CLIP-STORE.md) |

A compiled file names its persona (`samantha` or `jarvis` for the test voices) and carries the sha256 of the reference it was built from. The Jarvis file keeps the hub texture. See [JARVIS-TEXTURE.md](JARVIS-TEXTURE.md).

Record the gate in [SELF-EVAL.md](SELF-EVAL.md).
# Symphony section A — voice compiler

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. The runtime voice is a Galion clip. Chatterbox files are not.

Master slogan: "The seamless connection with the human being, on the voice only."

## Cut

Question: why would the symphony play a Chatterbox weight, a WAV, or the worker's raw stdout?
Delete: those are not the runtime path.
Simplify: one container the tree already reads (`.gq` v1), four tensors, 24 kHz, 80 ms frames.
Accelerate: load once (mmap, prefault). The tick copies one frame. No alloc, no I/O, no decode.
Automate: `scripts/symphony/self-eval-a.sh`. A miss fails the section.

Chatterbox stays the out-of-process generator (PyTorch, CPU, not linked). Its stdout is s16le at 24 kHz. This compiler is the next step. It does not start PyTorch and it does not download weights.

## Runtime path

```text
voices/compiled/<persona>.gq
voices/compiled/<persona>.gq.sha256
```

`<persona>` is `samantha`, `jarvis`, or another id of 1..=15 characters from `[a-z0-9_-]`, starting with a letter. `$GALION_VOICE_DIR` replaces `voices/compiled`. The file stem must match the persona stored in the clip.

`galion-tensor` stays reader-only. The writer lives in `crates/galion-ml-voice` (`artifact`). `GqFile::open` with `Prefault::Populate` is the load. `VoiceClip::fill_frame` is the tick.

Do not point `GALION_TTS_WEIGHTS` at this directory. An A21 `.gq` is a future decoder pack and stays inactive. A voice clip is PCM frames, not that pack.

## File

Magic `GALIONGQ`, version 1, alignment 64. Same header the reader already checks. Four f32 tensors, lexicographic:

| Tensor | Shape | Meaning |
|---|---|---|
| `voice.clock` | `[4]` | `u32` bits: rate, `frame_ms`, `n_samples`, channels |
| `voice.flags` | `[4]` | `u32` bits: `cpu_only=1`, `gpu=0`, `chatterbox_runtime=0`, `galion_clip=1` |
| `voice.id` | `[16]` | persona, one byte per `u32`, then zeros |
| `voice.pcm` | `[n_frames, 1920]` | mono f32 in [-1, 1]. Tail after `n_samples` is silence |

Integers are `f32::from_bits`, not decimal floats. `.gq` v1 has no u32 dtype, and this clip does not add one. Rate is 24000. Frame is 80 ms. 1920 samples. That is `galion_heartbeat::{SAMPLE_RATE, FRAME_MS, FRAME_SAMPLES}` and `LOCKS.toml` `frame_ms`.

Each tensor carries its own SHA-256 in the table. The sidecar is the whole-file digest, coreutils shape (`<hex>  <file>`). `open_runtime` refuses a clip with no sidecar, a bad digest, a wrong stem, a GPU flag, or `chatterbox_runtime != 0`.

Write is `<path>.tmp`, fsync, rename. Same for the sidecar.

## Inputs (not runtime)

| Input | What it is |
| `.wav` | 16-bit PCM, mono or stereo. Rate is in the header. |
| `.pcm` `.s16le` `.raw` | Raw s16le. `--rate` default 24000, `--channels` default 1. |
| `.gci` | Galion clone intermediate. Magic `GCLONINT`. Compiler input only. |

Stereo is averaged per frame. A click on one channel stays in the mix. Resample to 24 kHz is linear. There is no noise gate, no high-pass, and no denoise. Jarvis hub texture is not stripped here.

Refused as input and as output: `.safetensors`, `.pt`, `.pth`, `.onnx`, `.bin`, `.ckpt`, `.npz`, `.npy`, `.ggml`, `.mp3`, `.flac`, and a GGML-family magic. A safetensors blob renamed to `.gq` is refused. A `.gq` clip is not accepted as clone output (it is already the runtime file). `.wav`, `.gci`, and raw PCM are refused by `open_runtime`.

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

`fill_frame` does not allocate and does not read the clock. A late flag, a short buffer, or an index past the end writes silence and returns `Paused`. The index is not skipped. Call it outside `Heartbeat::step`. The step still does not touch disk.

## CPU

CPU only. No GPU buffer in the clip (`voice.flags` gpu = 0).

Hosts this file is shaped for, from the CPU foundation note (capacity, not a measurement on those machines):

- Flagship app/voice node: AMD EPYC 9655P-class, 96 cores / 192 threads
- Plus: CX23
- Ultra: CX43

Non-inference tick budget is 1% of the 80 ms frame: 800 µs. `fill_frame` is that tick for a compiled utterance. The gate asserts p99 and max under 800 µs. Numbers from the run that wrote `SELF-EVAL.md` are in that file.

A compiled utterance is f32 PCM. One second is 96 KiB. That sits under the foundation's ~0.65–1.15 GiB RAM/session figure. This crate does not measure a hosted voice session.

## Credits

Puzzle pieces, named so the licence stays visible. Nothing below is linked except the in-repo crates.

| Piece | Licence | What this compiler does with it |
|---|---|---|
| `.gq` v1 reader, `crates/galion-tensor/src/gq.rs` | Perpetual Open MIT, (c) 2026 Maciej Grajczyk | The runtime reader. The clip writer in this crate follows that layout. `galion-tensor` grows no writer. |
| SHA-256 in that same reader | FIPS 180-4, in-repo MIT | Tensor digests and the sidecar. No third-party hash crate. |
| WAV PCM walk, same subset as `galion-render/src/wav.rs` | In-repo MIT | Reimplemented here so the voice crate does not depend on the render crate. Public RIFF/WAVE layout. |
| Linear resample, same idea as `galion-render` `resample_linear` | In-repo MIT | Reimplemented. No denoise. |
| safetensors header (u64 length + JSON `"dtype"`) | Hugging Face, Apache-2.0 | Recognized only to refuse it. No safetensors crate. |
| Chatterbox | Resemble AI, MIT (read 2026-11-03) | Not copied. The worker may emit PCM. Weights stay out of process and are not this file. |
| GGML-family container | Not used | Magic is refused. Voice does not read or write that container. The datacenter note's LLM sizing is a capacity reference only. |

## What this section does not do

- It does not edit `LOCKS.toml`. `reference_clips` stays as the lock left it. No film audio is fetched.
- It does not run Chatterbox `generate`. That is the clone worker, and only when the pinned weights are on disk.
- It does not turn Nano/Turbo safetensors into a neural decoder. `GALION_NATIVE_DECODER_WIRED` stays false. The symphony plays frames. It does not host the Chatterbox graph.
- It does not claim an EPYC or CX23/CX43 timing capture. The budget gate ran on the build host. See `SELF-EVAL.md`.

— Maciej Grajczyk
