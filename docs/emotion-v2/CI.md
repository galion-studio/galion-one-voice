# Emotion V2 CI

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.

Samantha, Galion Voice V2. Tuning standard: **432 Hz**. This harness scores her twelve emotions. It does not deploy, it does not merge, and it does not read a caller's voice.

The emotions are Neutral, Calm, Happy, Amused, Excited, Tender, Sad, Disappointed, Angry, Fearful, Surprised, and Disgusted.

## What a run measures

For each emotion the harness takes three inputs:

- fixed scenario texts (`tools/emotion-v2/scenarios/<emotion>/`),
- a render directory of 16-bit PCM WAV files,
- that emotion's reference directory.

It then writes one JSON file per emotion plus `summary.json` and `summary.md`.

| Measure | How | Gate |
|---|---|---|
| Categorical posterior vs that emotion's reference | Cosine of the mean recogniser posterior of the renders and the mean posterior of the reference clips | ≥ **0.83** (`tools/emotion-v2/thresholds.toml`, one row per emotion) |
| Embedding similarity vs that emotion's reference | Cosine of the mean embeddings | Reported. Set `embedding.min` in the thresholds file to gate it |
| WER | `galion-wer --text-dir <scenarios> --jobs 2 --out wer.json <renders…>` when that binary is present | ≤ **0.018**. Absent is not a measurement, and it fails the row only with `--require-guards` |
| ECAPA | `galion-ecapa --ref <speaker wav> --threads 2 --out ecapa.json <renders…>` when that binary is present | **> 0.7057** (the baseline itself fails). Same absence rule as WER |

Reference clips are exemplars of the emotion. They are not lined up with the scenario stems. The speaker reference for ECAPA is `--speaker-ref`, or the first reference WAV in that emotion's directory when the flag is omitted.

The recogniser is pluggable. `auto` uses `superb/wav2vec2-base-superb-er` when a complete weight directory is already on the machine, and the stub otherwise. `superb` fails closed when the weights are missing. Nothing in this tree downloads weights, installs torch, or installs transformers.

## CPU job (pull requests)

Workflow: `.github/workflows/emotion-v2-ci.yml`.

The job is CPU only. It runs the harness unit tests. Those tests synthesize sines in a temporary directory, force the stub backend, and do not call the SUPERB loader. `HF_HUB_OFFLINE`, `TRANSFORMERS_OFFLINE`, and `HF_DATASETS_OFFLINE` are set. The job does not install packages and does not fetch a model.

Run the same tests locally:

```bash
python -m unittest discover -s tools/emotion-v2/harness/tests -v
```

The same job also runs the `emotion_match` per-sector tests. Those score each
synthetic render against its own sector reference through
`0.70*category_blend+0.30*emb_sim`, with the 0.83 gate applied per sector.
A manifest maps each emotion to a reference path and sha256. A missing or
mismatched pin is a hard failure. The tests use the CPU stub and sines in a
temporary directory. They do not load SUPERB weights and they do not commit
audio. Real capture-kit references are still required before a live score.

```bash
python -m unittest discover -s tools/cloning-v2/emotion-match/tests -v
```

Score a local render tree with the stub (no weights):

```bash
PYTHONPATH=tools/emotion-v2 python -m harness score \
  --backend stub \
  --render-dir /path/to/renders \
  --reference-dir /path/to/references \
  --out-dir /path/to/out
```

`--render-dir` and `--reference-dir` each contain one subdirectory per emotion (`neutral` or `Neutral`, and the other eleven). WAV files stay outside git. `tools/emotion-v2/.gitignore` ignores audio and weight files.

Exit status: `0` when every scored emotion passes, `1` when a score fails, `2` when the thresholds file is unusable or a requested recogniser has no local weights.

## Stub backend

`StubBackend` returns a deterministic posterior over the twelve emotion names. A pure tone at the frequency in `STUB_FREQ_HZ` peaks on that emotion. Amused is **432 Hz**, the tuning standard, so a 432 Hz sine is the fixture for Amused. The same samples always yield the same posterior and the same embedding. The stub is not a model of affect.

## SUPERB weights

The loader calls `from_pretrained` on a local directory with `local_files_only=True`. The hub id is not passed to the loader. There is no `snapshot_download` and no `hf_hub_download`.

Search order:

1. `EMOTION_V2_SUPERB_WEIGHTS` — exclusive. A missing path stays missing. No other cache is tried.
2. `weights/emotion-v2/wav2vec2-base-superb-er/` (gitignored).
3. `tools/emotion-v2/weights/wav2vec2-base-superb-er/` (gitignored).
4. An already complete Hugging Face snapshot under `HF_HUB_CACHE`, `HUGGINGFACE_HUB_CACHE`, `HF_HOME/hub`, or `~/.cache/huggingface/hub`.

A complete directory contains `config.json` and one of `model.safetensors`, `pytorch_model.bin`, or `model.safetensors.index.json`. A partial download is ignored. The harness will not finish it.

The published head is four-way (neu, hap, ang, sad). The twelve-way score is still the cosine against that emotion's reference posterior, so Tender, Amused, and the rest do not need a twelve-way head. The embedding is the mean of the last hidden state.

## Guards

`GALION_WER` and `GALION_ECAPA` point at the existing executables. When those variables are unset, the harness looks for:

- `tools/cloning-v2/scorers/quality/bin/galion-wer`
- `tools/cloning-v2/scorers/ecapa/target/release/galion-ecapa`

Those paths are not in this repository today. CPU CI therefore records the guards as `absent` unless a test supplies a stand-in. `--require-guards` turns absence into a failure. That flag is for a machine that is supposed to have the scorers. The pull-request job does not pass it.

WER JSON is `{ "summary": { "mean": <float> } }`. ECAPA JSON is `{ "mean": <float> }`. That is the shape `galion-voicecheck` already reads.

## GPU nightly

There is no nightly workflow in `.github/workflows`. A GPU run is an operator job on a machine that already holds the weights and the scorers. Do not add a workflow step that downloads them.

Sketch, not checked in as a job:

```bash
export HF_HUB_OFFLINE=1
export TRANSFORMERS_OFFLINE=1
export HF_DATASETS_OFFLINE=1
export EMOTION_V2_SUPERB_WEIGHTS=/opt/galion/weights/wav2vec2-base-superb-er
export GALION_WER=/opt/galion/bin/galion-wer
export GALION_ECAPA=/opt/galion/bin/galion-ecapa

PYTHONPATH=tools/emotion-v2 python -m harness score \
  --backend superb \
  --require-guards \
  --speaker-ref /opt/galion/refs/samantha/reference.wav \
  --render-dir /opt/galion/emotion-v2/renders \
  --reference-dir /opt/galion/emotion-v2/references \
  --out-dir /tmp/emotion-v2-nightly
```

The operator copies `summary.md` and the twelve JSON files out as a build artifact if they want a record. They do not commit WAV files, clips, weights, or secrets. A missing weight directory must fail the nightly (`--backend superb` exits 2). It must not fall back to the stub and it must not reach the hub.

Suggested cadence is once a night on one GPU runner. The pull-request job stays the CPU stub. A red nightly does not deploy and does not merge.

## Nebula live feed

`docs/emotion-v2/NEBULA-LIVE-FEED.md` is the frame schema for `apps/nebula-voice-v2`. The pull-request job runs its unit tests in the same CPU step's environment. Those tests use numeric posteriors. They do not synthesize audio, and they do not download weights.

```bash
python -m unittest discover -s tools/emotion-v2/nebula/tests -v
```

## Out of scope

This cut does not render speech, tune a checkpoint, touch Galion OS, edit `SOUL.md`, `config.yaml`, or `LOCKS.toml`, or change a deployed voice.
