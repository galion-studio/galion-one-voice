# Local studio

Perpetual Open MIT License. (c) 2026 Galion Labs.

Offline renderer for a script on the owner's machine. The reference clip is a path you pass when you run it. The clip stays on that machine. This tool does not ship a voice, does not name a third-party speaker, and does not download weights.

CPU is the default. `device = "auto"` uses CUDA when an NVIDIA GPU is visible (`nvidia-smi`, a Linux NVIDIA device node, or a usable `CUDA_VISIBLE_DEVICES` value). An explicit `--device cpu` stays on CPU. A CUDA start that fails is tried once more on CPU. The same binary runs on Linux and on Windows.

The locked voice workspace does not build this crate. Inference stays in programs that are already installed:

| Step | Program | What this crate does |
|---|---|---|
| Zero-shot takes | `cbx speak` (Chatterbox), or `render.command` | Passes the reference wav, the line, the device, and a seed. Sets offline environment variables. |
| WER | OpenAI `whisper` CLI | Reads the transcript and computes word error rate here. whisper.cpp is not vendored. |
| Speaker similarity | `galion-ecapa` | Reads the cosine score. The gate is greater than 0.7057. |
| Emotion | `galion-emotion`, or a command that prints a directed score | Reads `emotion_match`, `emotion`, `score`, or the classifier posterior for the line's label. |
| Opus and mp3 | `ffmpeg` | Encodes the mastered wav. |
| Loudness | in-process BS.1770-4, or `ffmpeg loudnorm` when ffmpeg is installed | -16 LUFS, true peak at or below -1 dBTP. `loudnorm` uses `linear=true`. |

`HF_HUB_OFFLINE=1` is set on every child process. A tool that ignores it and tries to fetch weights is outside this crate. Install models before the run.

## Gates

A take is kept only when all three scores were measured and all three pass:

- WER ≤ 0.018 (1.8%)
- ECAPA > 0.7057
- emotion ≥ 0.83

The config can make a gate stricter. It cannot loosen one. A missing score fails the line. The best passing take is the one with the higher emotion score, then the higher ECAPA score, then the lower WER. Lines are stitched with the lead, the per-line pause, and the tail from the config.

## Bed

The optional bed is generated in `src/bed.rs`. Pad, chime, and low pulse are integer ratios of 432 Hz. Breath is filtered noise from a fixed generator. There is no sample file and no donor clip. Under the voice, the bed's root-mean-square level is 18 dB down. The mix is then mastered, so that ratio stays put.

## Captions

`captions.json` has one sentence per script line: start, end, script, emotion, take, scores, and the sha256 of that take's wav. The top-level sha256 is the mastered wav. `report.json` lists every take, including the ones the gates rejected.

## Run

From the repository root, on GALION-STUDIO or another Linux machine:

```bash
cargo build --manifest-path tools/local-studio/Cargo.toml --release
./tools/local-studio/target/release/galion-local-studio doctor --config tools/local-studio/examples/studio.toml
./tools/local-studio/target/release/galion-local-studio render \
  --config tools/local-studio/examples/studio.toml \
  --reference "$HOME/galion-voices/owner.wav" \
  --script tools/local-studio/examples/script.txt \
  --out "$HOME/galion-studio-out" \
  --owner-reference
```

On Windows, from the same kind of checkout:

```powershell
cargo build --manifest-path tools/local-studio/Cargo.toml --release
.\tools\local-studio\target\release\galion-local-studio.exe render `
  --config tools/local-studio/examples/studio.toml `
  --reference "$env:USERPROFILE\galion-voices\owner.wav" `
  --script tools/local-studio/examples/script.txt `
  --out "$env:USERPROFILE\galion-studio-out" `
  --owner-reference
```

`--owner-reference` is required on every render, and `consent.owner_reference` in the config must be true. The reference path has to be a file outside this repository. Put the output directory outside the repository as well. `studio-out/` and `tools/local-studio/out/` are gitignored if a run is pointed at the checkout by mistake.

Chatterbox Turbo's `--exaggeration` flag does not steer the ONNX model. The emotion direction is scored after the take. Point `render.command` at a runtime that accepts `{emotion}` when the render itself must follow the direction. Render placeholders are `{text}`, `{emotion}`, `{reference}`, `{out}`, `{device}`, `{seed}`, `{temperature}`, `{take}`, and `{line}`. Scorer commands also accept `{wav}`, `{json}`, `{dir}`, and `{model}`.

The emotion command should print one of `emotion_match`, `emotion`, or `score`, or a posterior whose label matches the line. `galion-emotion` writes `files[].ser.probs` with the labels `neu`, `hap`, `ang`, and `sad`. A script word such as `calm` does not match those labels until `emotion.label_map` names the translation. Similarity against the reference clip is not that score.

`--synthetic` renders a 432 Hz tone instead of a clone. The gates still run. A tone will not pass whisper. It exists so the stitch, the bed, and the master can be tested with no model.

## Script

```text
# comment
[calm] The meeting is on Thursday at four.
[warm pause_ms=360] I left the notes in your inbox.
```

## Tests

```bash
cargo test --manifest-path tools/local-studio/Cargo.toml
```

Tests generate tones in a temporary directory. They do not read a voice and they do not download weights. Rust 1.85 matches the rest of this repository.

## Audio in git

Wav, flac, mp3, opus, and ogg files are gitignored. `scripts/check-audio-fixtures.sh` fails when a tracked file of those kinds is larger than 4096 bytes unless that exact blob is already in `scripts/audio-legacy.sha256`. That list is the listening-set audio that was tracked before this guard. The emotion-sweep harness pins those hashes. Do not add lines to the list. New renders and new reference clips stay out of the commit.
