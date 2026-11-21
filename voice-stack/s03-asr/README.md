<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright (c) 2026 Galion Labs -->

# S03 ASR

Streaming CPU speech-to-text for the Samantha voice stack. Audio in is 16 kHz mono PCM16. The engine returns partial snapshots and a final. The line to keep is the final. Partials are not deltas, and they are not appended to the final.

This crate is its own Cargo workspace. It is not a member of the repository workspace.

## What it reuses

`galion-asr-vosk` (PR #26, `docs/asr/VOSK.md`) is the phone-path worker. It dlopens libvosk, feeds 20 ms frames (320 samples), drops `[unk]`, and does not call `vosk_gpu_init`. This crate uses that same library, the same `vosk-model-small-en-us-0.15` directory, and the same environment variables (`GALION_VOSK_MODEL`, `GALION_VOSK_LIB`, `GALION_VOSK_THREADS`, default 2 threads).

The #26 protocol accepts one whole utterance per line. `VoskWorkerAsr` speaks that protocol (`galion-asr-vosk/1`) when you point it at the `galion-asr-vosk` binary. The live path is `VoskAsr`: one recognizer, audio pushed as it arrives, a partial when the hypothesis changes, a final when libvosk endpoints or when `finish` is called. Push 80 ms at a time (`HEARTBEAT_SAMPLES` = 1280). That is the same heartbeat as `galion-heartbeat`.

A missing model is an error. The engines do not fill in a sentence.

## Backends

| Name | When |
|---|---|
| `vosk` | Default. In-process streaming libvosk. |
| `vosk-worker` | Spawns `galion-asr-vosk`. Partials arrive at `finish`, because that protocol is one utterance per line. |
| `whisper` | whisper.cpp through whisper-rs 0.16. Cargo feature `whisper`. Off by default. CPU only (`use_gpu = false`). No CUDA, Metal, Vulkan, or HIP features. |
| `fake` | Test double. Reveals a script you already passed in. Not a measurement. |

`LOCKS.toml` still forbids GGML on the phone path. This crate does not edit that file, does not vendor weights, and does not join the main workspace. The whisper backend exists only when you pass `--features whisper`.

whisper.cpp is not a 20 ms decoder. `push_pcm` only buffers unless `GALION_WHISPER_PARTIAL_MS` is set, in which case each window runs a full decode. `finish` runs one decode and returns the final. The runs below left partials off.

## Linux

From this directory (`voice-stack/s03-asr`):

```bash
./scripts/fetch-models.sh
./scripts/fetch-cc0.sh
cargo test
cargo run --release --bin s03-asr-eval -- --threads 2 --out eval/runs/vosk.json
```

One file, NDJSON partials and finals on stdout:

```bash
cargo run --release --bin s03-asr -- --wav weights/cc0/LJ001-0002.wav
```

Whisper (downloads `ggml-tiny.en.bin` and `ggml-base.en.bin`, then builds whisper.cpp):

```bash
./scripts/fetch-models.sh --whisper
# If /usr/bin/c++ is clang and CMake fails to find libstdc++, point at gcc:
CC=/usr/bin/gcc CXX=/usr/bin/g++ cargo run --release --features whisper --bin s03-asr-eval -- \
  --backend whisper --model weights/whisper/ggml-tiny.en.bin --threads 4 \
  --out eval/runs/whisper-tiny.en.json
```

`whisper-rs-sys` 0.15 asks for Rust 1.88 or newer. The default crate, without that feature, was tested with Rust 1.83.

`vosk-worker` needs a built worker from the repo root:

```bash
cargo build -p galion-asr-vosk --bin galion-asr-vosk
```

That package sets `rust-version` 1.85. Then:

```bash
cargo run --release --bin s03-asr-eval -- --backend vosk-worker --bin ../../target/release/galion-asr-vosk
```

## Windows

PowerShell, from `voice-stack\s03-asr`. These commands were not run on the machine that produced `eval/MEASURED.md` (that machine is Linux).

```powershell
powershell -ExecutionPolicy Bypass -File scripts\fetch-models.ps1
powershell -ExecutionPolicy Bypass -File scripts\fetch-cc0.ps1
cargo test
cargo run --release --bin s03-asr-eval -- --threads 2 --out eval\runs\vosk.json
```

The Windows libvosk zip is `vosk-win64-0.3.45` (sha256 `f1dcc9cca460630f81ea8f71794f69c80bed6556d2a4e6237b5785e1d2dff34b`). The loader calls `SetDllDirectoryW` on the directory that contains `libvosk.dll` plus the bundled `libstdc++-6.dll`, `libgcc_s_seh-1.dll`, and `libwinpthread-1.dll`.

Whisper on Windows needs CMake and a C++ compiler (Visual Studio Build Tools or MinGW), then:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\fetch-models.ps1 -Whisper
cargo run --release --features whisper --bin s03-asr-eval -- --backend whisper --model weights\whisper\ggml-tiny.en.bin --threads 4
```

Git Bash can run the `.sh` scripts instead of the PowerShell ones. They pick the Windows zip when `uname` is MINGW, MSYS, or CYGWIN.

## Models and clips

Nothing under `weights/` is committed. `scripts/fetch-models.sh` checks sha256 and unpacks:

| Piece | sha256 |
|---|---|
| `vosk-model-small-en-us-0.15.zip` | `30f26242c4eb449f948e42cb302dd7a686cb29a3423a8367f99ff41780942498` |
| `vosk-linux-x86_64-0.3.45.zip` | `bbdc8ed85c43979f6443142889770ea95cbfbc56cffb5c5dcd73afa875c5fbb2` |
| `vosk-win64-0.3.45.zip` | `f1dcc9cca460630f81ea8f71794f69c80bed6556d2a4e6237b5785e1d2dff34b` |
| `ggml-tiny.en.bin` | `921e4cf8686fdd993dcd081a5da5b6c365bfde1162e72b08d75ac75289920b1f` |
| `ggml-base.en.bin` | `a03779c86df3323075f5e796cb2ce5029f00ec8869eee3fdfb897afe36c6d002` |

The score set is 12 LJ Speech 1.1 clips, LJ001-0001 through LJ001-0012, dataset order. Public domain (Keith Ito / LibriVox, reader Linda Johnson). Mozilla Data Collective catalogs that dataset as CC0-1.0. Transcripts are in `eval/cc0-ljspeech.tsv`. Wav checksums are in `eval/cc0-ljspeech.sha256`. The wavs are not in git.

Vosk is Apache-2.0 (Alpha Cephei). whisper.cpp is MIT. whisper-rs is Unlicense. Those sources are downloaded at build or fetch time. They are not copied into this tree.

## Tests

`cargo test` does not need a model. It covers the word-error counter, the wav reader, the 80 ms frame size, fake streaming, and the `galion-asr-vosk/1` handshake against `s03-asr-protocol-double` (a fixed reply, not a recognizer).

```bash
cargo test -- --ignored
```

runs the silence check when the small-en model and libvosk are on disk. One second of zeros must come back empty.

## Measured

Release builds on this Linux VM. Details and the unmodified stdout are in `eval/MEASURED.md`. JSON with every chunk time is in `eval/runs/`.

The normalizer lowercases, splits on punctuation and hyphens, and keeps in-word apostrophes. It does not expand numerals, so whisper's `15th` against the normalized transcript `fifteenth` counts as an error.

| Backend | Words | Errors | micro WER | Audio | Wall | RTF | 80 ms chunks |
|---|---:|---:|---:|---:|---:|---:|---|
| Vosk small-en, 2 threads | 200 | 10 (8 sub, 0 del, 2 ins) | 0.050000 | 79.451 s | 4.728 s | 0.059510 | p50 0.307 ms, p95 18.116 ms, 1 of 999 over 80 ms |
| whisper tiny.en, 4 threads, CPU | 200 | 12 (8 sub, 3 del, 1 ins) | 0.060000 | 79.451 s | 5.605 s | 0.070543 | push p50 0.000 ms; finish p50 466.048 ms |
| whisper base.en, 4 threads, CPU | 200 | 12 (7 sub, 3 del, 2 ins) | 0.060000 | 79.451 s | 10.484 s | 0.131955 | push p50 0.000 ms; finish p50 869.036 ms |

Vosk model load was 280.806 ms and is not inside the RTF. The chunk over 80 ms was LJ001-0001, chunk index 4, 148.067 ms. First non-empty Vosk partial: wall-clock p50 24.262 ms from the start of feeding, at audio cursor p50 640 ms. The file is pushed as fast as the decoder runs, so that wall time is not a live-mic wait. The chunk times are the live-mic cost of each 80 ms push.

Whisper load was 110.751 ms (tiny) and 198.113 ms (base), also outside RTF. `push_pcm` does not decode, so those chunk times are the buffer copy. The decode is `finish`. Both finish medians are over 80 ms. No whisper partials were emitted. Each clip builds a new whisper state. `whisper_init_with_params_no_state` printed `use gpu = 0`.

The ignored silence test passed: one second of zeros, empty final.

## Missing

- Not wired to the microphone, the VAD, or the HORBM turn.
- Windows was not run here.
- `vosk-worker` was checked against the protocol double, not timed against a built `galion-asr-vosk`.
- Whisper has no 20 ms partials unless you set `GALION_WHISPER_PARTIAL_MS`, which re-decodes the whole buffer.
- Twelve read-speech clips. Not a conversational WER, and not the full LJ Speech set.
