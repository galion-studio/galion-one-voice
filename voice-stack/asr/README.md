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
| `fake` | Test double. Reveals a script you already passed in. Not a measurement. |

Vosk is the only recognizer. This crate does not depend on whisper.cpp or any other GGML runtime, and it does not edit `LOCKS.toml`.

## Linux

From this directory (`voice-stack/asr`):

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

`rust-version` is 1.85. `vosk-worker` needs a built worker from the repo root:

```bash
cargo build -p galion-asr-vosk --bin galion-asr-vosk
```

That package sets `rust-version` 1.85. Then:

```bash
cargo run --release --bin s03-asr-eval -- --backend vosk-worker --bin ../../target/release/galion-asr-vosk
```

## Windows

PowerShell, from `voice-stack\asr`. These commands were not run on the machine that produced `eval/MEASURED.md` (that machine is Linux).

```powershell
powershell -ExecutionPolicy Bypass -File scripts\fetch-models.ps1
powershell -ExecutionPolicy Bypass -File scripts\fetch-cc0.ps1
cargo test
cargo run --release --bin s03-asr-eval -- --threads 2 --out eval\runs\vosk.json
```

The Windows libvosk zip is `vosk-win64-0.3.45` (sha256 `f1dcc9cca460630f81ea8f71794f69c80bed6556d2a4e6237b5785e1d2dff34b`). The loader calls `SetDllDirectoryW` on the directory that contains `libvosk.dll` plus the bundled `libstdc++-6.dll`, `libgcc_s_seh-1.dll`, and `libwinpthread-1.dll`.

Git Bash can run the `.sh` scripts instead of the PowerShell ones. They pick the Windows zip when `uname` is MINGW, MSYS, or CYGWIN.

## Models and clips

Nothing under `weights/` is committed. `scripts/fetch-models.sh` checks sha256 and unpacks:

| Piece | sha256 |
|---|---|
| `vosk-model-small-en-us-0.15.zip` | `30f26242c4eb449f948e42cb302dd7a686cb29a3423a8367f99ff41780942498` |
| `vosk-linux-x86_64-0.3.45.zip` | `bbdc8ed85c43979f6443142889770ea95cbfbc56cffb5c5dcd73afa875c5fbb2` |
| `vosk-win64-0.3.45.zip` | `f1dcc9cca460630f81ea8f71794f69c80bed6556d2a4e6237b5785e1d2dff34b` |

The score set is 12 LJ Speech 1.1 clips, LJ001-0001 through LJ001-0012, dataset order. Public domain (Keith Ito / LibriVox, reader Linda Johnson). Mozilla Data Collective catalogs that dataset as CC0-1.0. Transcripts are in `eval/cc0-ljspeech.tsv`. Wav checksums are in `eval/cc0-ljspeech.sha256`. The wavs are not in git.

Vosk is Apache-2.0 (Alpha Cephei). The model and libvosk are downloaded by the fetch script. They are not copied into this tree.

## Tests

`cargo test` does not need a model. It covers the word-error counter, the wav reader, the 80 ms frame size, fake streaming, and the `galion-asr-vosk/1` handshake against `s03-asr-protocol-double` (a fixed reply, not a recognizer).

```bash
cargo test -- --ignored
```

runs the silence check when the small-en model and libvosk are on disk. One second of zeros must come back empty.

## Measured

Release builds on this Linux VM. Details and the unmodified stdout are in `eval/MEASURED.md`. JSON with every chunk time is in `eval/runs/`.

The normalizer lowercases, splits on punctuation and hyphens, and keeps in-word apostrophes. It does not expand numerals.

| Backend | Words | Errors | micro WER | Audio | Wall | RTF | 80 ms chunks |
|---|---:|---:|---:|---:|---:|---:|---|
| Vosk small-en, 2 threads | 200 | 10 (8 sub, 0 del, 2 ins) | 0.050000 | 79.451 s | 4.728 s | 0.059510 | p50 0.307 ms, p95 18.116 ms, 1 of 999 over 80 ms |

Vosk model load was 280.806 ms and is not inside the RTF. The chunk over 80 ms was LJ001-0001, chunk index 4, 148.067 ms. First non-empty Vosk partial: wall-clock p50 24.262 ms from the start of feeding, at audio cursor p50 640 ms. The file is pushed as fast as the decoder runs, so that wall time is not a live-mic wait. The chunk times are the live-mic cost of each 80 ms push.

The ignored silence test passed: one second of zeros, empty final.

## Missing

- Not wired to the microphone, the VAD, or the HORBM turn.
- Windows was not run here.
- `vosk-worker` was checked against the protocol double, not timed against a built `galion-asr-vosk`.
- Twelve read-speech clips. Not a conversational WER, and not the full LJ Speech set.
