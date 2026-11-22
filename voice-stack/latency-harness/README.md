<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright (c) 2026 Galion Labs -->

# S12 latency harness

Per-stage clocks for the local Samantha voice path. The package is `galion-sam-latency`. The directory is `voice-stack/latency-harness`, which is the S12 sector path. This crate is its own Cargo workspace. It is not a member of the repository root workspace.

The release numbers below were measured before this directory rename, with rustc 1.83.0, and were not run again. The JSON still names that compiler and the commit `f0b39b38f42f016fe620fe3ac1a4a096745f10eb`.

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
cd voice-stack/latency-harness
cargo test
cargo run --release -- run --backend all --reps 100 --out-dir results
```

`cargo test` passes without the pack. The Piper integration test skips when the pack is absent. When the pack is present, that test speaks two warm reps and checks that the first PCM byte came from the child.

## Run on Windows

The owner's tree lives under `G:\galion`. From the repository root:

```bat
cd /d G:\galion\voice-stack\latency-harness
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
galion-sam-latency run [--backend fake|piper|all] [--reps N] [--warmup N] [--out-dir DIR]
```

The default is `--backend all --reps 100 --warmup 1 --out-dir results`.

Exit 0 means every requested backend measured. Exit 2 means a requested backend was absent or failed. That path writes `measured: false` and no percentile fields. Exit 1 is usage.

Publish the release profile. The results file records `profile`, so a debug run cannot be mistaken for the release run.

## Results file

`results/<backend>.json` and `results/<backend>.md` are two views of one report. Schema `galion-s12-latency/v1`.

The machine block has the CPU model, CPU MHz, nproc, OS, kernel, hostname (an IP-shaped name is stored as `redacted`), load average, UTC date, git commit, whether the worktree was dirty, and `rustc --version`.

Raw per-rep microseconds are in the JSON under `reps`. The Markdown table is the summary. `clock` is `measured` or `derived`.

Each rep records `first_slice_grain` as `clause` or `sentence`. The report counts those in `first_slice_clause_reps` and `first_slice_sentence_reps`. Reply ready to first slice is that first piece, the same cut `galion-sam-horbm` gives the Piper mouth. A clause ends on a comma, semicolon, colon, dash, or conjunction once it has at least four words. A sentence is the first sentence when no earlier clause was ready. The published phrase `I am right here.` is one sentence. `I don't know.` stays one sentence and is not rewritten.

## Another implementation of the traits

`measure` is generic over `SpeechEnd`, `AsrStage`, `ReplyEngine`, `TtsStage`, and `AudioOut`. A later crate implements `ReplyEngine` for `horbm::Brain` and passes it in. The CLI ships `fake` and `piper` only.

## Measured on this VM

Release run, 2026-11-21. `cargo run --release -- run --backend all --reps 100 --warmup 1 --out-dir results`. Wall clock for that command, compile included, was 02:49:42Z to 02:49:55Z.

Host: `cursor`, Linux 6.12.113+ x86_64, 4 CPUs. CPU: Intel(R) Xeon(R) Processor at 2400.000 MHz. rustc 1.83.0. Source commit `f0b39b38f42f016fe620fe3ac1a4a096745f10eb`. Profile `release`. n = 100, so p99 is not the maximum. The tables are the files in `results/`.

`results/fake.json` recorded `commit_dirty: false`. `results/piper.json` recorded `commit_dirty: true` because `results/fake.json` and `results/fake.md` were already written and not committed. No source file changed between the two files.

Piper runtime: `weights/runtime/piper/piper`. Model sha256 matches the A21 pin. Sample rate 22050 Hz. Phrase `I am right here.` Prosody fixed at rest. Cores were not pinned. VAD, ASR, and the reply are the fake stages in both files.

The fake reply stage measured 0 µs on every rep. The work finished below the 1 µs truncation, so the clock is 0.000 ms. It did run: the report's last mood is resting, and the sink peak is non-zero.

On the Piper run the first stdout read already held a full 80 ms frame (100 of 100 reps, heartbeat play within 2 µs of the first-PCM chain). The derived grid therefore did not add a tick. First audio out is the resample into the sink after that read.

Cold clocks are the `Hi.` utterance and are not in the percentile table. Spawn to first PCM was 330.370 ms. Write to first PCM on that same utterance was 329.941 ms. The ~0.4 ms gap is process creation before the write. The ~330 ms is model load plus the first sentence. That cold number is over the 80 ms bar.

Warm Piper, reply ready to first PCM byte: p50 41.180 ms, p95 58.275 ms, p99 59.087 ms, max 65.066 ms. End of speech to first audio out: p50 41.228 ms, p95 58.332 ms, p99 59.147 ms, max 65.134 ms. Every measured percentile in the warm table is under 80 ms. The pipeline product claim stays false: the ASR and the reply are fakes, and there is no microphone or speaker in the clock.

### Fake (`results/fake.md`)

Load average at 2026-11-21T02:49:44Z: 0.08 0.05 0.01.

| stage | clock | min ms | p50 ms | p95 ms | p99 ms | max ms | p50 ≤ 80 ms | p95 ≤ 80 ms | p99 ≤ 80 ms |
|---|---|---:|---:|---:|---:|---:|---|---|---|
| vad (offline buffer) | measured | 0.003 | 0.003 | 0.005 | 0.005 | 0.005 | yes | yes | yes |
| end of speech to ASR final | measured | 0.002 | 0.002 | 0.003 | 0.003 | 0.015 | yes | yes | yes |
| ASR final to reply ready | measured | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | yes | yes | yes |
| reply ready to first TTS audio | measured | 0.009 | 0.009 | 0.012 | 0.013 | 0.019 | yes | yes | yes |
| first TTS audio to first audio out | measured | 0.005 | 0.005 | 0.007 | 0.008 | 0.008 | yes | yes | yes |
| end of speech to first audio out | measured | 0.017 | 0.019 | 0.024 | 0.028 | 0.037 | yes | yes | yes |
| end of speech to heartbeat play | derived | 0.012 | 0.012 | 0.016 | 0.022 | 0.029 | yes | yes | yes |

### Piper (`results/piper.md`)

Load average at 2026-11-21T02:49:55Z: 0.37 0.12 0.03.

| stage | clock | min ms | p50 ms | p95 ms | p99 ms | max ms | p50 ≤ 80 ms | p95 ≤ 80 ms | p99 ≤ 80 ms |
|---|---|---:|---:|---:|---:|---:|---|---|---|
| vad (offline buffer) | measured | 0.005 | 0.006 | 0.007 | 0.007 | 0.007 | yes | yes | yes |
| end of speech to ASR final | measured | 0.002 | 0.002 | 0.003 | 0.003 | 0.003 | yes | yes | yes |
| ASR final to reply ready | measured | 0.000 | 0.001 | 0.001 | 0.001 | 0.001 | yes | yes | yes |
| reply ready to first TTS audio | measured | 33.457 | 41.180 | 58.275 | 59.087 | 65.066 | yes | yes | yes |
| first TTS audio to first audio out | measured | 0.024 | 0.045 | 0.071 | 0.088 | 0.207 | yes | yes | yes |
| end of speech to first audio out | measured | 33.542 | 41.228 | 58.332 | 59.147 | 65.134 | yes | yes | yes |
| end of speech to heartbeat play | derived | 33.461 | 41.185 | 58.280 | 59.091 | 65.071 | yes | yes | yes |

## What is not in this crate

- The reply stage is the fake slot. It does not open a HORBM memory database and does not score cards. The first-slice cut is `galion_sam_horbm::speakable_chunks`, the same splitter the 7881 mouth uses.
- Vosk is not called. The ASR stage is the fake scan.
- There is no microphone and no sound device. End of speech is an offline VAD on a generated tone. First audio out is an in-memory sink.
- The heartbeat play time is computed from measured arrivals. The process does not sleep until the next tick.
- Piper prosody stays at rest for the whole run.
- No Samantha, Jarvis, or s711 audio is read or written.
