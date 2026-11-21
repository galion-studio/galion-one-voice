<!--
// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
-->

# galion-emotion-in

S08 of the Samantha voice stack, at `voice-stack/emotion-in/`. This crate reads prosody from mic frames on the CPU and turns it into a PAD mood target for HORBM.

It is its own Cargo workspace. It is not a member of the Galion Voice workspace, and the root `Cargo.toml` does not name it.

The real `horbm` crate is not linked. `FakeBrain::open(memory_db, knowledge_dir, cases)` has the same argument list as `Brain::open` and does not read those paths. `ReplyEngine::turn` returns the same fields as `brain.turn`. When `horbm` is available, implement `ReplyEngine` for `horbm::Brain` and keep feeding it `mood_target`. This crate does not reimplement retrieval. The memory crate it would sit on is `horbm-memory` from `galion-studio/galion-knowledge` at `761a7f225304fc010379219e5b88c6669dd48f40`.

`GALION_SAMANTHA_REF` is not read. No reference clip, no Samantha audio, no Jarvis audio, and no user recording is written.

## What it reuses

- The voice heartbeat: 80 ms, 12.5 Hz, 24 kHz, 1920 samples (`LOCKS.toml` `[clock]`, `galion-emotion` `frame::live_frame`).
- Sense consent, default deny, checked before PCM is read (`LOCKS.toml` `[consent]`, `galion-emotion`).
- RMS and zero-crossing rate, the same left-to-right sum as `galion-emotion` `loudness::rms_zcr`. Valence there is unknown. Here pitch, rate, and pauses are added, and valence is still only a named rule, not a trained label.
- The one-dot mark: the dot stays `#F5F5F7` scaled by brightness. Amber and violet are background only (`galion-render` `palette`).
- The stage unit's sandbox shape (`deploy/systemd/templates/galion-voice-stage.service`). `deploy/galion-emotion-in.service` is a template. Nothing installs it. `ExecStart` is `/usr/bin/false`.

## What a frame becomes

1. The gate runs. On deny, the sample loader is not called.
2. Input that is not already 24 kHz is one 80 ms hop, resampled by position onto 1920 samples. PCM s16le and float32, mono or stereo, 1..=48000 Hz, and only when 80 ms lands on a whole number of samples.
3. Pitch is YIN on a pair-averaged 12 kHz copy of the frame (70–420 Hz). Energy is RMS. Brightness is zero-crossing rate.
4. The last 2.56 s of scalars are kept: 10 ms envelope hops and one F0 per frame. Samples are not in that state. `EmotionIn` is `Copy`.
5. Syllable peaks are local maxima of a 70 ms smoothed envelope, at least 100 ms apart, and at least 40% above the minimum of the previous 300 ms. A flat tone does not count. A pause is a quiet run of at least 200 ms.
6. Six normalised features go through a fixed linear map (`src/map.rs`). `anger_press` then runs only when the window is loud, high, and flat. Confidence is capped at 0.70. Below 0.20 the mood target is `Pad::RESTING` (`arousal` 0.3, `valence` 0.25, `dominance` 0.5), the same cutoff `galion-emotion` uses before it will name an affect.
7. `FakeBrain::tick` moves mood 18% of the remaining gap on each 80 ms step. That gain is this stand-in, not a measured HORBM constant.

Anchors for the 0..1 normalisation, from `src/map.rs`:

| feature | low | high |
|---|---:|---:|
| voiced RMS | 0.015 | 0.280 |
| median F0, Hz | 85 | 280 |
| articulation, syllables/s | 1.5 | 6.5 |
| F0 p90−p10, Hz | 8 | 90 |
| pause ratio | 0 | 0.60 |
| zero-crossing rate | 0.02 | 0.20 |

A framing buffer holds at most one input frame and is wiped after the frame is pushed, on finish, and on drop. Reports set `audio_retained_samples` to 0.

`Pad::RESTING` keeps the brain's triple (arousal 0.3, valence 0.25, dominance 0.5). This crate clamps valence to −1..1. That span is an assumption of this crate. `horbm` does not document a range here, and `EmotionFrame::neutral` uses valence 0.0.

The optional `onnx` feature builds a one-node Gemm of the same coefficients in memory and parses those bytes. It is not a speech-emotion model, it does not download anything, and it does not add a `.onnx` file to the tree. The feature is off unless you pass `--features onnx`.

## Run

Rust 1.85 is what the voice repo asks for. This crate was built and tested with rustc 1.83.0.

### Linux

```bash
cd voice-stack/emotion-in
cargo test --offline
cargo test --offline --release
cargo test --offline --features onnx
cargo run --offline --release -- bench 200
cargo run --offline --release -- wav /path/to/clip.wav --sense
arecord -f S16_LE -r 24000 -c 1 -t raw | cargo run --offline --release -- pcm --rate 24000 --sense --stream
cargo run --offline --release -- serve --bind 127.0.0.1:8768 --wav /path/to/clip.wav --sense
```

Open `http://127.0.0.1:8768/`. The page is one dot. `?debug=1` shows the JSON. `--pace` sleeps 80 ms between frames so the dot moves with the file. That sleep is the heartbeat quantum, not a latency measurement.

Sense stays denied until `--sense` or `GALION_EMOTION_SENSE=1`. Any other value of that variable stays deny. The grant is this process only.

### Windows

From the clone. On the owner's machine the home directory is `G:\galion`; the repo path under that home can differ.

```bat
cd /d G:\galion\galion-one-voice\voice-stack\emotion-in
cargo test --offline
cargo test --offline --release
cargo run --offline --release -- bench 200
cargo run --offline --release -- wav C:\path\to\clip.wav --sense
ffmpeg -f dshow -i audio=Microphone -ac 1 -ar 24000 -f s16le - | target\release\emotion-in.exe pcm --rate 24000 --sense --stream
cargo run --offline --release -- serve --bind 127.0.0.1:8768 --wav C:\path\to\clip.wav --sense
```

The pcm path sets stdin to binary mode on Windows so s16le is not translated. Wav files do not need that. There is no WASAPI capture inside this crate. The microphone is whatever writes the pipe.

CC0 clips are not in git. `*.wav` is ignored. Fetch them yourself. The three used for the measurements below are listed with the numbers.

## Measurements

Run on this cloud VM (Linux, rustc 1.83.0) on 21 Nov 2026. These are the commands that were actually run. Times are microseconds. The swarm streaming bar used for the release assertion is 80 ms = 80000 µs. The architecture ceiling for this stage is 2.4 ms (3% of the frame). The push p95 below is under both.

`cargo test --offline` passed: 26 library tests, 5 pipeline tests.

`cargo test --offline --release` passed: the same 26 and 5. In release the pipeline test also asserts pitch, energy, push, and the mood tick each have p95 under 80000 µs.

`cargo test --offline --features onnx` and `cargo test --offline --release --features onnx` passed: 27 library tests (the extra one checks the Gemm against the Rust coefficients, absolute error under 1e-5 on three vectors) and the same 5 pipeline tests. The ONNX path was not given its own timing run.

### Release bench, 200 frames of a 180 Hz sine at amplitude 0.2

`cargo run --offline --release -- bench 200`

| stage | median µs | p95 µs | sum of per-call µs | one clock around the loop, µs |
|---|---:|---:|---:|---:|
| pitch (YIN) | 65 | 75 | 13213 | 13287 |
| energy (RMS and ZCR) | 2 | 2 | 348 | 412 |
| push (pitch + energy + map) | 72 | 72 | 14442 | 14528 |
| mood tick | 0 | 0 | 0 | 13 |

Per-call medians under 1 µs are stored as 0 because the timer reports whole microseconds. The mood-tick batch of 200 steps was 13 µs.

### Debug bench, 40 frames, same sine

`cargo run --offline -- bench 40` (debug, not release)

| stage | median µs | p95 µs | batch µs |
|---|---:|---:|---:|
| pitch | 326 | 358 | 12878 |
| energy | 5 | 5 | 208 |
| push | 341 | 408 | 14341 |
| mood tick | 0 | 0 | 4 |

### Pitch on one synthetic frame

Same release bench. Harmonicity is `1 - d'` at the chosen lag.

| target Hz | measured Hz | error Hz | error % | harmonicity |
|---:|---:|---:|---:|---:|
| 100 | 100.003 | +0.003 | +0.003 | 1.000 |
| 140 | 140.006 | +0.006 | +0.005 | 1.000 |
| 180 | 180.012 | +0.012 | +0.007 | 1.000 |
| 220 | 220.013 | +0.013 | +0.006 | 0.999 |
| 300 | 300.089 | +0.089 | +0.030 | 1.000 |

### Synthetic scenes

Same release bench. The prosody block is the whole scene when the scene is at most 2.56 s. `estimate` is the linear map. `target` is what would be handed to the brain. Silence has confidence 0, so the target is resting even though the raw map, looking at a window that is entirely pause, sits at valence −0.75.

| scene | F0 Hz | F0 range Hz | RMS | dBFS | articulation /s | nuclei | pause ratio | longest pause ms | press | confidence | estimate a, v, d | target a, v, d | push p95 µs |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|---|---:|
| silence, 2.0 s | 0 | 0 | 0 | −120 | 0 | 0 | 1.000 | 2000 | 0 | 0 | 0.000, −0.750, 0.000 | 0.300, 0.250, 0.500 | 71 |
| steady 120 Hz, amp 0.05, 0.96 s | 120.006 | 0.001 | 0.0353 | −29.04 | 0 | 0 | 0 | 0 | 0 | 0.700 | 0.128, −0.028, 0.290 | same | 70 |
| steady 120 Hz, amp 0.25, 0.96 s | 120.006 | 0.001 | 0.1766 | −15.06 | 0 | 0 | 0 | 0 | 0 | 0.700 | 0.394, −0.002, 0.503 | same | 75 |
| steady 120 Hz, amp 0.15, 0.96 s | 120.006 | 0.001 | 0.1059 | −19.50 | 0 | 0 | 0 | 0 | 0 | 0.700 | 0.261, −0.015, 0.396 | same | 70 |
| steady 240 Hz, amp 0.15, 0.96 s | 240.047 | 0.006 | 0.1061 | −19.49 | 0 | 0 | 0 | 0 | 0 | 0.700 | 0.397, 0.047, 0.427 | same | 70 |
| 180 Hz, AM 2 Hz, 2.0 s | 180.020 | 0.030 | 0.0707 | −23.01 | 1.500 | 3 | 0 | 0 | 0 | 0.700 | 0.262, 0.009, 0.358 | same | 71 |
| 180 Hz, AM 5 Hz, 2.0 s | 180.059 | 0.037 | 0.0707 | −23.01 | 4.500 | 9 | 0 | 0 | 0 | 0.700 | 0.394, 0.081, 0.449 | same | 71 |
| 180 Hz, 400 ms tone, 400 ms gap, 400 ms tone | 180.013 | 0.002 | 0.1414 | −16.99 | 3.750 | 3 | 0.333 | 400 | 0 | 0.700 | 0.339, −0.312, 0.366 | same | 70 |
| steady 240 Hz, amp 0.35, 2.0 s | 240.047 | 0.006 | 0.2475 | −12.13 | 0 | 0 | 0 | 0 | 0.407 | 0.700 | 0.664, −0.130, 0.690 | same | 71 |
| 140/220 Hz every 180 ms, amp 0.12, 2.0 s | 140.007 | 148.806 | 0.0848 | −21.43 | 0 | 0 | 0 | 0 | 0 | 0.700 | 0.324, 0.541, 0.420 | same | 71 |

What those rows actually show:

- A louder steady tone at the same F0 has higher RMS and higher arousal (0.394 vs 0.128).
- A higher steady F0 at the same amplitude has higher arousal (0.397 vs 0.261).
- Amplitude modulation at 5 Hz produced 9 peaks in 2.0 s (4.5/s). Modulation at 2 Hz produced 3 peaks (1.5/s). The written rates were 5 and 2; peaks that sit on the edge of the window are the difference. Arousal followed the rate (0.394 vs 0.262).
- A constructed 400 ms zero gap was measured as 400 ms, and the pause ratio on that 1.2 s scene was 0.333.
- A loud, high, flat sine took `anger_press` 0.407 and valence −0.130. Alternating 140 and 220 Hz did not. Its measured F0 spread was 148.8 Hz, wider than the 80 Hz gap between those two tones, so some frames on the switch are not sitting on either tone. Valence on that scene was +0.541. Articulation was 0: the level did not rise and fall, only the frequency did.
- A steady tone produced 0 syllable peaks.

### CC0 clips

Fetched into `/tmp` for this run. Not committed.

| clip | source | format as analysed |
|---|---|---|
| `pdx-voice.wav` | `https://raw.githubusercontent.com/pdx-cs-sound/wavs/main/voice.wav` | pcm s16le, mono, 48000 Hz, 4.949 s, 61 frames, 3328 tail samples dropped |
| `amy_koenig.wav` | Voice Zero `amy_koenig.flac` (CC0), ffmpeg to pcm s16le, rate left at 44100 Hz | 6.400 s, 80 frames, 0 tail samples |
| `alan_davis_drake.wav` | Voice Zero `alan_davis_drake.flac` (CC0), same conversion | 7.063 s, 88 frames, 994 tail samples dropped |

`cargo run --offline --release -- wav <file> --sense`

The numbers below are the last 2.56 s only (the ring). They are not a label for the whole file, and these clips have no emotion transcript in this run. All three kept `audio_retained_samples` at 0, `anger_press` at 0, and `classifier` at `linear`.

| clip | F0 Hz | range Hz | RMS dBFS | articulation /s | nuclei | pause ratio | longest pause ms | voiced ratio | confidence | target a, v, d | push median µs | push p95 µs |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|---:|---:|
| pdx | 127.425 | 50.724 | −23.92 | 3.125 | 6 | 0.250 | 640 | 0.648 | 0.661 | 0.1863, 0.0539, 0.2842 | 73 | 78 |
| Amy Koenig | 165.288 | 36.331 | −24.38 | 1.493 | 3 | 0.215 | 550 | 0.578 | 0.640 | 0.1536, −0.0203, 0.2490 | 73 | 77 |
| Alan Davis Drake | 108.968 | 43.553 | −26.71 | 1.852 | 4 | 0.156 | 400 | 0.621 | 0.654 | 0.1132, 0.0586, 0.2574 | 73 | 85 |

Amy's articulation is 1.493/s, just under the 1.5 anchor, so her normalised rate is 0. The three readings sit near a quiet rest: low arousal, valence within about ±0.06 of 0, dominance under 0.3. That is what this map does with moderate level, a modest pitch range, and pauses of a few hundred milliseconds. It is not a claim that the speakers were calm, sad, or anything else.

The highest push p95 on these files was 85 µs (Alan Davis Drake), against the 80000 µs bar.

`emotion-in serve --bind 127.0.0.1:8768 --wav` the Amy Koenig file was requested with curl. `GET /` returned the page (one `#dot`, script `/dot.js`). `GET /dot` returned JSON with `audio_retained_samples` 0 and `state` `listening`. `serve --bind 0.0.0.0:8768` exited 4 with `Bind: loopback only` and did not bind. The server was stopped after that check.

## What is missing

- `horbm` itself. The mood target is only applied to `FakeBrain`.
- A microphone device. Capture is a wav file or an s16le pipe (`arecord`, ffmpeg dshow). No cpal, no WASAPI.
- A trained emotion model. The ONNX file is the linear map, not a classifier fit on labelled speech.
- Any claim that prosody alone can tell joy from anger. `anger_press` is one explicit rule for loud + high + flat. Confidence never goes above 0.70, and on the CC0 narrations valence stayed near 0.
- A per-speaker baseline. The anchors are fixed.
- The summary is the last 2.56 s, not a whole conversation.
- Wiring into the 80 ms heartbeat process, the phone lane, or Piper. This sector only produces the target.
- The systemd template is not installed and was not started.
- The one-dot page was checked over HTTP. It was not opened in a browser on this VM.
