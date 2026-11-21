<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright (c) 2026 Galion Labs -->

# S12 latency harness

Per-stage clocks for the local Samantha voice path. This crate is its own Cargo workspace. It is not a member of the repository root workspace.

The path is mic-shaped, but this program does not open a microphone or a speaker. It times five marks on a buffer that is already in memory:

1. End of user speech (the energy VAD returns).
2. ASR final.
3. Reply ready.
4. First TTS audio (the first PCM sample).
5. First audio out (one 24 kHz heartbeat frame accepted by the sink).

A sixth row, heartbeat play, is derived from the measured arrivals. The grid is the one in `scripts/eval/piper_stream_ttfa.py`: the tick starts at the first PCM byte, a tick with no full frame is a pause, and a late frame does not play a catch-up burst. The derived row is not a sleep and not a second wall clock.

The 80 ms frame is the same contract as `crates/galion-heartbeat`: `FRAME_MS = 80`, `SAMPLE_RATE = 24000`, `FRAME_SAMPLES = 1920`. Percentiles use the same nearest-rank index as `rank` in `crates/galion-heartbeat/tests/frame_budget.rs`: `ceil(n * p / 100) - 1`. For `n < 100`, p99 is the maximum sample. The published runs use `n = 100`, so p99 is not the maximum.

The bar on each stage is 80 ms. A row that misses it says so. Missing a bar is a result, not a failure of the file.

## What is reused

- Heartbeat frame size, the late-tick-is-a-pause rule, and the percentile index, from `crates/galion-heartbeat` and its frame-budget test. This crate does not link that crate and does not remeasure `Heartbeat::step`.
- Piper spawn flags and the first-PCM clock, from `crates/galion-v2-live/src/piper.rs` and `scripts/eval/piper_stream_ttfa.py`. The runtime lookup follows `crates/galion-ml-voice/src/speech/weights.rs`: `$GALION_A21_RUNTIME`, else `galion-piper-rt` when that binary is installed, else `weights/runtime/piper/piper`. `$GALION_PIPER_LEGACY=1` keeps the legacy CLI.
- The A21 pack pin for `en_US-ljspeech-medium`, from the same weights module. A mismatch does not get a latency number.

VAD, ASR, and the reply in both backends are in-process fakes. They do real work (an RMS scan, a string copy) and they do not sleep. They are not Vosk and they are not HORBM. The Piper backend changes only the mouth.

`ReplyEngine` is the slot for `horbm::Brain`. The real call is `Brain::open(memory_db, knowledge_dir, cases)` then `brain.turn(&str) -> TurnOut`. `Pad::RESTING` is arousal 0.3, valence 0.25, dominance 0.5. This crate does not open a memory database and does not score cards. `FakeReply` fills the slot until that crate is linked. Mood steps once per 80 ms toward rest. That rule is the harness stand-in (`toward-rest-0.1`). It is not HORBM's update. The spoken sentence does not change with mood.

The Piper child is started at rest (`length_scale 1.0`, `noise_scale 0`, `noise_w 0`) so the spread is the CPU and not a noise draw. The reply still carries a `Pad`. That `Pad` is not applied to the child. The voice is `en_US-ljspeech-medium` (LJSpeech). It is not Samantha. `GALION_SAMANTHA_REF` is not read. No audio file is written.

## Run on Linux

From the repository root, fetch the pinned Piper pack once (Linux x86_64). The files land in `weights/`, which git ignores.

```bash
scripts/a21/fetch-a21.sh
cd voice-stack/s12-latency-harness
cargo test
cargo run --release -- run --backend all --reps 100 --out-dir results
```

`cargo test` passes without the pack. The Piper integration test skips when the pack is absent. When the pack is present, that test speaks two warm reps and checks that the first PCM byte came from the child.

## Run on Windows

The owner's tree lives under `G:\galion`. From the repository root:

```bat
cd /d G:\galion\voice-stack\s12-latency-harness
cargo test
cargo run --release -- run --backend fake --reps 100 --out-dir results
```

`scripts/a21/fetch-a21.sh` installs the Linux x86_64 Piper build only. On Windows, point the harness at a local Piper and do not expect this program to download:

```bat
set GALION_A21_RUNTIME=C:\path\to\piper.exe
set GALION_TTS_MODEL=C:\path\to\en_US-ljspeech-medium.onnx
cargo run --release -- run --backend piper --reps 100 --out-dir results
```

The `.onnx.json` must sit beside the model (`en_US-ljspeech-medium.onnx.json`).

## CLI

```text
s12-latency-harness run [--backend fake|piper|all] [--reps N] [--warmup N] [--out-dir DIR]
```

The default is `--backend all --reps 100 --warmup 1 --out-dir results`.

Exit 0 means every requested backend measured. Exit 2 means a requested backend was absent or failed. That path writes `measured: false` and no percentile fields. Exit 1 is usage.

Publish the release profile. The results file records `profile`, so a debug run cannot be mistaken for the release run.

## Results file

`results/<backend>.json` and `results/<backend>.md` are two views of one report. Schema `galion-s12-latency/v1`.

The machine block has the CPU model, CPU MHz, nproc, OS, kernel, hostname (an IP-shaped name is stored as `redacted`), load average, UTC date, git commit, whether the worktree was dirty, and `rustc --version`.

Raw per-rep microseconds are in the JSON under `reps`. The Markdown table is the summary. `clock` is `measured` or `derived`.

## Another implementation of the traits

`measure` is generic over `SpeechEnd`, `AsrStage`, `ReplyEngine`, `TtsStage`, and `AudioOut`. A later crate implements `ReplyEngine` for `horbm::Brain` and passes it in. The CLI ships `fake` and `piper` only.

## Measured on this VM

The tables below are copied from the release run committed next to this file. If you re-run, trust the new `results/` files over this section.

See `results/fake.md` and `results/piper.md` after a release run. This section is filled from those files and names the CPU they were measured on.

## What is not in this crate

- HORBM is not linked. The reply stage is the fake slot.
- Vosk is not called. The ASR stage is the fake scan.
- There is no microphone and no sound device. End of speech is an offline VAD on a generated tone. First audio out is an in-memory sink.
- The heartbeat play time is computed from measured arrivals. The process does not sleep until the next tick.
- Piper prosody stays at rest for the whole run.
- No Samantha, Jarvis, or s711 audio is read or written.
