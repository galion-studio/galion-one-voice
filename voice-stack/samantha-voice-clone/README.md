<!--
// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
-->

# Samantha s711 voice clone

This crate is the Samantha voice backend for the local voice stack. It implements the `Tts` trait: text and a live mood in, raw 24 kHz 16-bit mono PCM out.

The approved recipe is fixed:

| Knob | Value |
|---|---|
| Mode | `expr` |
| Exaggeration | 0.5 |
| CFG | 0.5 |
| Seed | 711, then 712, 713, … one seed per sentence |
| Conditioning | the original reference on every sentence |
| Output | 24 kHz, 16-bit, mono, little-endian |
| Post | none: no mastering, no slowdown, no effect pass |

Mood is accepted on `synthesize` and stored on the utterance. It does not change the words or the knobs. Consent is local zero-shot conditioning only. The crate does not fine-tune, upload, or download a model.

The reference wav is not in git. Set `GALION_SAMANTHA_REF` to a local file. The file is hashed at open. The sha256 must start with `10ac44dc`. Anything else is refused.

## What is reused

`python/s711_worker.py` imports `scripts/clone/chatterbox_worker.py` and uses its sha256, PCM write, and stdout/stderr split. A protocol check also starts that script directly with `GALION_CLONE_STUB_PCM=1` and sends exaggeration 0.5 and cfg 0.5.

The repo worker cannot speak this clip by itself. Its allowlist is `scripts/clone/REFS.sha256` (a different Samantha pin), and `pin_cpu()` exits if the device is not `cpu`. Seed is not a field that worker reads. This crate therefore keeps the expressive generate path in the sector worker: same library call (`ChatterboxTTS.from_local` plus `generate` with the reference path, exaggeration, and cfg), with `torch.manual_seed(711 + index)` before every sentence. CUDA is used only when torch reports it. CPU is the path when it does not.

`GALION_SAMANTHA_WEIGHTS` must already be a local checkpoint directory. There is no `from_pretrained` call in this tree.

Repeated phrases are stored as raw `.pcm` plus a json sidecar. The cache directory is refused if it sits inside the git repo. The default is the system temp directory, `galion-samantha-s711`.

`FakeTts` implements `Tts` with one silent 80 ms frame and no process. Tests use it, and `RecordingSpeaker`, so the recipe can be checked without a checkpoint.

## Linux

From the repo root, with Rust 1.83 or newer (this repo's CI uses 1.85):

```bash
bash voice-stack/samantha-voice-clone/ci/check-no-voice-files.sh
cargo test --manifest-path voice-stack/samantha-voice-clone/Cargo.toml
cargo run --manifest-path voice-stack/samantha-voice-clone/Cargo.toml --example speak -- --self-check
```

`--self-check` prints the interpreter probe, the in-process stub pack time, and one round trip through the repo Chatterbox stub. It does not need the reference wav and it does not load a model.

To speak, point at the local reference and a local checkpoint. CPU is the honest default when CUDA is not visible:

```bash
export GALION_SAMANTHA_REF="$HOME/samantha-ref.wav"
export GALION_SAMANTHA_WEIGHTS="$HOME/chatterbox-expr"
export GALION_SAMANTHA_DEVICE=auto
export GALION_SAMANTHA_CACHE="${TMPDIR:-/tmp}/galion-samantha-s711"
cargo run --manifest-path voice-stack/samantha-voice-clone/Cargo.toml --example speak -- "Hello."
```

`GALION_SAMANTHA_DEVICE=cpu` forces CPU. `cuda` is refused when the probe does not see CUDA. `GALION_SAMANTHA_STUB=1` is protocol PCM for a pipe test. It is not a voice. `GALION_SAMANTHA_THREADS` caps CPU thread pools only when the device is `cpu`. Unset, CPU mode uses the interpreter default.

Python is `python3` unless `GALION_SAMANTHA_PYTHON` is set. A real utterance needs `torch` and `chatterbox.tts` installed locally. This crate does not install them.

## Windows

The owner's machine keeps Galion on `G:\galion`. The reference stays there, or anywhere outside this repo. Do not copy it into the tree.

PowerShell, from the repo:

```powershell
$env:GALION_SAMANTHA_PYTHON = "python"
$env:GALION_SAMANTHA_REF = "G:\galion\samantha-ref.wav"
$env:GALION_SAMANTHA_WEIGHTS = "G:\galion\chatterbox-expr"
$env:GALION_SAMANTHA_DEVICE = "auto"
$env:GALION_SAMANTHA_CACHE = "$env:TEMP\galion-samantha-s711"
cargo test --manifest-path voice-stack\samantha-voice-clone\Cargo.toml
cargo run --manifest-path voice-stack\samantha-voice-clone\Cargo.toml --example speak -- --self-check
cargo run --manifest-path voice-stack\samantha-voice-clone\Cargo.toml --example speak -- "Hello."
```

`speak` prints sample counts and microseconds. It does not write a wav. The guard script is bash; `cargo test` is the check that runs on Windows without bash. CI runs the script on Ubuntu.

## Library

```rust
use samantha_voice_clone::{Pad, SamanthaTts, Tts};

let mut voice = SamanthaTts::open_from_env()?;
let speech = voice.synthesize("Hello.", &Pad::RESTING)?;
assert_eq!(speech.sample_rate_hz, 24_000);
assert_eq!(speech.channels, 1);
assert_eq!(speech.bits, 16);
```

`speech.full_frames()` yields 1920-sample slices, which is 80 ms at 24 kHz. This crate does not call the heartbeat. A short tail is left as a tail.

`voice.pre_render(&["Hello.", "I'm here."], &Pad::RESTING)?` fills the phrase cache. The next `synthesize` of the same sentence, same reference, same device, does not call the worker. Mood is not part of the cache key.

## Audio guard

`voice-stack/samantha-voice-clone/.gitignore` ignores wav, mp3, flac, onnx, and the raw cache suffixes. `.github/workflows/voice-stack-s06-guard.yml` fails if a `.wav`, `.mp3`, `.flac`, or `.onnx` is tracked anywhere under `voice-stack/`.

## Measured on this build VM

These numbers are from one Linux x86_64 VM (kernel 6.12.113+, 4 CPUs), rustc 1.83.0, Python 3.12.3, on 2026-11-21. `torch` is not installed. `nvidia-smi` is not installed. This is not the owner's laptop and not a Chatterbox `generate`.

Debug `speak --self-check`:

- Compute probe: `cpu`, note `torch-missing:ModuleNotFoundError`, 9875 µs. A second `--probe` on the same binary was 8805 µs.
- Sector self-check: reuse `chatterbox_worker`, seeds 711 and 712, stub pack of 1920 samples in 7 µs, process wall 21090 µs.
- Repo `chatterbox_worker.py` stub, one frame, exaggeration 0.5, cfg 0.5, rate 1.0: exit 0, 3840 bytes, first sample 1756, wall 30928 µs. The controls line was `exaggeration=0.5000 cfg_weight=0.5000 temperature=0.8000 pitch_st=0.0000 energy=1.0000 pre_pause_ms=0 rate=1.0000`.

Release `speak --self-check` on the same VM:

- Probe 9677 µs, still `cpu` / `torch-missing:ModuleNotFoundError`.
- Stub pack 8 µs, self-check wall 20592 µs.
- Repo stub wall 26045 µs, same 3840 bytes and first sample 1756.

`cargo test --manifest-path voice-stack/samantha-voice-clone/Cargo.toml`: 14 unit tests, 6 integration tests, 0 doc tests, all passed. The unit binary reported 0.01 s and the integration binary 0.06 s, after the dependencies were already compiled.

CPU and CUDA model latency were not measured. There is no checkpoint and no reference wav whose sha256 starts with `10ac44dc` on this VM. The 80 ms stage bar is not claimed.

## What is not measured here

A Chatterbox generate on CPU or CUDA is not measured unless you run it on a machine that already has the checkpoint, torch, and the reference wav. The stub times printed by `--self-check` are protocol times. They are not a model latency and they are not an 80 ms claim.

Upstream `generate()` may still apply its own watermark or loudness. This process does not add a mastering pass, does not time-stretch, and does not strip that library step. If the checkpoint returns a rate other than 24 kHz, the worker resamples to 24 kHz and logs `galion-s711-resample`. That is a rate match, not a slowdown.

## Licence

The `LICENSE` file in this directory is the repo licence, copied byte for byte. Perpetual Open MIT License. Copyright (c) 2026 Galion Labs.
