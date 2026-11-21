<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright (c) 2026 Galion Labs -->

# eval-golden

Samantha voice quality gates for swarm 16, sector S13. This is a standalone crate. It is not a member of the repository workspace, and it does not install a service.

It checks three things:

- The rollout gate already locked in `tools/emotion-v2/thresholds.toml` and `tools/emotion-v2/harness/metrics.py`, and recorded by `tools/symphony/listen-back`.
- Golden conversation scripts for Samantha: greeting, follow-up, interruption, an unknown question, and an emotional check-in.
- A persona-tone lint for lines that are too long, robotic, help-desk phrased, whispered, or scripted comfort.

HORBM is not in this tree. `ReplyEngine` is the trait `horbm::Brain::turn` will satisfy. `open_horbm` returns an error and does not open a database. Tests use `FakeReplyEngine`. The fake's `arm` is `"fake"`, and it returns no evidence.

## Gates

| Check | Comparison | Source |
|---|---|---|
| WER | `<= 0.018` (1.8%), the ceiling itself passes | `wer_passes` |
| ECAPA cosine | `> 0.7057`, exactly 0.7057 fails | `ecapa_passes` |
| emotion_match | `>= 0.83` | `meets_min`, formula `0.70*category_blend+0.30*emb_sim` |

The swarm brief wrote ECAPA `>= 0.7057`. The locked harness is a strict greater-than, so a cosine sitting on the merge-v2 baseline is not a pass. This crate follows that comparison.

A missing scorer is `UNMEASURED`. A tool that ran and failed is `ERROR`. A measured miss is `FAIL`. None of those become a pass. `claimed` stays false.

WER tokens match `wer_t648.py` and `galion-wer`: lowercase, drop everything that is not `[a-z']` or a space, then Levenshtein over words divided by the reference length.

The 80 ms figure is `galion_heartbeat::FRAME_MS`. Piper's first raw byte and Vosk's first partial are compared with `<= 80`. Whole-utterance time is reported beside that and is not substituted for it.

## What a measured run did

One release run on this Linux machine, after `scripts/a21/fetch-a21.sh` and `scripts/asr/fetch-vosk.sh`. Those files stay under `weights/`, which git ignores. No wav was kept. `GALION_SAMANTHA_REF` was unset. The CPU line in `/proc/cpuinfo` was `Intel(R) Xeon(R) Processor`, 4 cores, Linux 6.12.113+ x86_64.

Command:

```bash
cargo run --release --manifest-path voice-stack/eval-golden/Cargo.toml -- report --out voice-stack/eval-golden/results/report.json
```

Voice: Piper `2023.11.14-2` at `weights/runtime/piper/piper`, pack `weights/tts/en_US-ljspeech-medium.onnx`, 22050 Hz, `noise_scale=0`, `noise_w=0`, `length_scale=1`, `sentence_silence=0`. That pack is the A21 LJSpeech voice. It is not a Samantha clone. ASR: `vosk-model-small-en-us-0.15` and `libvosk.so` 0.3.45, fed 16 kHz s16le from ffmpeg, 8000-byte chunks, same loop as `wer_t648.py`.

| Check | Result | Value |
|---|---|---|
| WER mean of 3 sentences | FAIL | 0.020833 (ceiling 0.018) |
| ECAPA | UNMEASURED | galion-ecapa is not installed, and `GALION_SAMANTHA_REF` is unset |
| emotion_match | UNMEASURED | the 0.70/0.30 recogniser was not run, and the CPU stub was not substituted |
| Rollout, all three together | UNMEASURED | pass is null. WER's miss is real, and the other two were not scored |
| Slowest Piper first byte, model already loaded | FAIL | 136.848 ms on the long sentence (bar 80) |
| Fastest Vosk partial | FAIL | 174.044 ms (bar 80) |
| Piper model load, spawn until `Initialized piper` | measured, not a gate | 305.591 ms |

Per sentence, from `results/report.json`:

| Reference | Heard | WER | First audio | TTS wall | Audio | ASR wall | First partial |
|---|---|---:|---:|---:|---:|---:|---:|
| Hi. I'm here. | hi i'm here | 0 | 28.462 ms | 99.265 ms | 1.265 s | 338.867 ms | 268.570 ms |
| I have moved the meeting to Thursday at four, and the notes are in your inbox. | i have moved the meeting to thursday at four and the notes or in your inbox | 0.0625 | 136.848 ms | 180.696 ms | 4.191 s | 742.883 ms | 174.391 ms |
| The left side sticks on the latch. | the left side sticks on the latch | 0 | 69.831 ms | 115.206 ms | 2.032 s | 515.149 ms | 174.044 ms |

The long line is 16 words. Vosk wrote "or" where the line says "are". That is one substitution, 1/16 = 0.0625. The mean of 0, 0.0625, and 0 is 0.020833, which is over 0.018.

Real-time factor below is TTS wall divided by audio length from that same report. It is not a separate clock.

- "Hi. I'm here.": 99.264941 ms / 1000 / 1.265488 s = 0.0784
- the meeting line: 180.696236 ms / 1000 / 4.191202 s = 0.0431
- the latch line: 115.206307 ms / 1000 / 2.031746 s = 0.0567

Two of the three first-byte times were under 80 ms (28.5 ms and 69.8 ms). The long line was not. The gate uses the slowest, so the stage fails the bar.

Golden scripts on `FakeReplyEngine` were all PASS. That is the text stand-in, not a measured call through HORBM.

## What is not measured

- Speaker similarity. ECAPA runs only when `GALION_ECAPA` (or the release binary under `tools/cloning-v2/scorers/ecapa`) exists and `GALION_SAMANTHA_REF` is a local file. Temp wavs for that compare are deleted. None of that ran here. No reference clip is in git.
- Emotion. `emotion_match` needs the emotion-v2 recogniser against a resting reference. This crate does not download those weights and does not report the CPU stub as a score.
- HORBM retrieval, intent, evidence, and the real mood model. The fake advances a pad one 80 ms lerp step per turn from `Pad::RESTING` `{arousal: 0.3, valence: 0.25, dominance: 0.5}`. That is not Samantha's persona rest in `galion-emotion`.
- Duplex barge-in. The interruption script checks the reply text ("Stop at once"). It does not cancel a playing buffer.
- A Samantha voice. The round-trip is the public LJSpeech Piper pack.
- Windows. The steps below are the same commands. They were not executed on Windows in this run.

## Linux

From the repo root, with Rust 1.83 or newer:

```bash
bash scripts/a21/fetch-a21.sh
bash scripts/asr/fetch-vosk.sh
cargo test --manifest-path voice-stack/eval-golden/Cargo.toml
cargo run --release --manifest-path voice-stack/eval-golden/Cargo.toml -- report --out /tmp/eval-golden.json
```

`ffmpeg` has to be on `PATH`. The fetch scripts are the existing ones. They write under `weights/` and check sha256. This crate does not download.

Useful overrides, all optional:

```bash
export GALION_A21_RUNTIME=/path/to/piper
export GALION_TTS_WEIGHTS=/path/to/en_US-ljspeech-medium.onnx
export GALION_VOSK_MODEL=/path/to/vosk-model-small-en-us-0.15
export GALION_VOSK_LIB=/path/to/libvosk.so
export GALION_FFMPEG=/path/to/ffmpeg
export GALION_ECAPA=/path/to/galion-ecapa
export GALION_SAMANTHA_REF=/path/to/local-reference.wav
```

`GALION_SAMANTHA_REF` is read at runtime. Do not commit that file, a clone, or any wav this tool writes.

Other commands:

```bash
cargo run --manifest-path voice-stack/eval-golden/Cargo.toml -- golden
cargo run --manifest-path voice-stack/eval-golden/Cargo.toml -- lint --text "Hi. I'm here." --user "Hey."
```

`--strict` exits 1 when a measured check fails. The default exit is 0 after an honest report, including when a gate is FAIL or UNMEASURED.

Open `voice-stack/eval-golden/ui/index.html` and choose the JSON. The page has no network and no audio. The one-dot console stays the existing demo.

## Windows

The owner's home is `G:\galion`. From that checkout, in PowerShell:

```powershell
cargo test --manifest-path voice-stack\eval-golden\Cargo.toml
$env:GALION_HOME = "G:\galion"
$env:GALION_A21_RUNTIME = "G:\galion\weights\runtime\piper\piper.exe"
$env:GALION_TTS_WEIGHTS = "G:\galion\weights\tts\en_US-ljspeech-medium.onnx"
$env:GALION_VOSK_MODEL = "G:\galion\weights\asr\vosk-model-small-en-us-0.15"
$env:GALION_VOSK_LIB = "G:\galion\weights\asr\libvosk\libvosk.dll"
cargo run --release --manifest-path voice-stack\eval-golden\Cargo.toml -- report --out "$env:TEMP\eval-golden.json"
```

The Linux fetch script installs `piper`, not `piper.exe`. On Windows, point the variables at the Piper and Vosk build already on that machine. `ffmpeg` still has to be on `PATH`, or set `GALION_FFMPEG`. This agent did not run the Windows path.

Without those files the round-trip stays UNMEASURED and the golden-script tests still run.

## Lint

From `docs/galion-voice-v2/personas/SAMANTHA.md`:

- More than three sentences, or more than 40 words, is too long. A third sentence is allowed only when the user asked her to stay with it.
- Help-desk and robotic openers are flagged (`I'd be happy to`, `certainly`, `as an AI`, `great question`, and the same family).
- Canned empathy is flagged (`I hear you`, `that sounds so hard`, and the lines the persona file forbids). "I'm here." on its own is not that phrase.
- Any "whisper" is flagged. She is not a whisper voice.

## Layout

```
voice-stack/eval-golden/
  src/           crate
  ui/            report page
  results/report.json   the release run above
```

`cargo test` is green without a network. On a machine that already has Piper and Vosk it also runs one round-trip and checks that ECAPA and emotion stay unmeasured unless those tools are actually present.

## Licence

Perpetual Open MIT. The `LICENSE` file in this directory is a byte copy of the repository `LICENSE`.
