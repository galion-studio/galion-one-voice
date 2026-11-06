# Adopted scorers (Cloning A2)

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. Symphony section. Index: [README.md](README.md).

`tools/cloning-v2/score-wrap` (`galion-score-wrap` 0.1.0, MIT) is the CPU
wrapper built in this landing. It shells out to an existing scorer when that
binary and its model are both on disk. The JSON field is the string
`unmeasured` when either is missing, when no clip was given, or when the
child returns no finite number. This page records names, URLs, versions, and
licences. It publishes no WER, ECAPA cosine, MOS, SQUIM figure, or
`emotion_match`.

Samantha stays a demo persona. Jarvis voice files and `clip-store-own` are
untouched. No audio, weights, or secrets are added to git.

## Built and wrapped

| Piece | Role |
|---|---|
| `galion-score-wrap` | **Built.** Discovery, JSON, and process spawn. It does no inference of its own. Dependency: `serde_json` 1.0.145. Release binary on this host: 751472 bytes (x86_64, outside git). |
| `galion-wer` | **Wrapped.** Spawned with `--model`, `--lib`, `--jobs`, and `--ref` or `--text-dir`. |
| `galion-mos` | **Wrapped.** Spawned with `--models` and `--threads` when a SQUIM ONNX file is present. Subjective MOS also needs `--nmr`. |
| `galion-ecapa` | **Wrapped.** Spawned with `--ref`, `--model`, and `--threads` when `ecapa.onnx` is present. |
| SpeechBrain ECAPA in the HF cache | **Presence.** The snapshot is reported. The CUDA Python scorers are left alone. |
| `emotion_match` | **Left unmeasured.** The formula name is a lookup key. This crate does not evaluate it. |

## Scorers

### galion-wer

| | |
|---|---|
| Name | `galion-wer` (crate `galion-quality`) |
| Lookup | `cloning-v2/rollout-blendfix/scorers/quality/bin/galion-wer` |
| URL | No public URL for this binary is recorded in this git checkout. The recogniser it calls is Vosk, below. |
| Version | `0.1.0` in the on-disk `cloning-v2/rollout-blendfix/scorers/quality/galion-quality/Cargo.toml`. That crate is outside this git checkout. |
| Licence | MIT, same `Cargo.toml`. |

On this host the binary is present (717712 bytes). A probe with no clip left `wer` as `unmeasured`.

### Vosk (model for galion-wer)

| | |
|---|---|
| Name | `vosk-model-small-en-us-0.15` and libvosk `0.3.45` |
| Lookup | `/home/galion/workspace/galion/weights/asr` (`vosk-model-small-en-us-0.15`, `libvosk/libvosk.so`). Ready when `am/final.mdl` and `conf/model.conf` are non-empty files. |
| URL | Model zip `https://alphacephei.com/vosk/models/vosk-model-small-en-us-0.15.zip`. Library zip `https://github.com/alphacep/vosk-api/releases/download/v0.3.45/vosk-linux-x86_64-0.3.45.zip`. Attribution in `docs/asr/VOSK.md`: `https://github.com/alphacep/vosk-api` and `https://alphacephei.com/vosk/models`. |
| Version | Model `vosk-model-small-en-us-0.15`. Library `0.3.45` (`scripts/asr/fetch-vosk.sh`, `docs/asr/VOSK.md`). |
| Licence | Apache-2.0 (Alpha Cephei / Kaldi), stated in `docs/asr/VOSK.md`. |

On this host the model directory is 70898967 bytes and `libvosk.so` is 25986496 bytes. Those are file sizes. They are not a word-error rate.

### SpeechBrain ECAPA and galion-ecapa

| | |
|---|---|
| Name | SpeechBrain ECAPA-TDNN, cache id `speechbrain/spkrec-ecapa-voxceleb`. CPU shell-out is `galion-ecapa`. |
| Lookup | HF cache under `$HF_HOME` (default `<root>/cache/hf`): `hub/models--speechbrain--spkrec-ecapa-voxceleb/snapshots/*/embedding_model.ckpt` plus `hyperparams.yaml`, or `speechbrain/spkrec-ecapa-voxceleb/`. CPU binary: `cloning-v2/rollout-blendfix/scorers/ecapa/bin/galion-ecapa`. ONNX: `$GALION_ECAPA_MODEL` or `ecapa/models/ecapa.onnx`. |
| URL | This git checkout records no URL. `scripts/clone/lat240/sim.py` names speechbrain ECAPA and Apache-2.0. This host's installed package metadata (speechbrain 1.1.2, not a repo pin) records Homepage `https://speechbrain.github.io/`. The cache `hyperparams.yaml` records `pretrained_path: speechbrain/spkrec-ecapa-voxceleb`. |
| Version | Repo pin: none. Host package: speechbrain 1.1.2. `galion-ecapa` crate version `0.1.0` (`scorers/ecapa/Cargo.toml`, outside this checkout). |
| Licence | SpeechBrain Apache-2.0 (`scripts/clone/lat240/sim.py`, and the installed package metadata). `galion-ecapa`'s `Cargo.toml` declares no licence field. |

On this host the HF snapshot is present. `embedding_model.ckpt` is 83316686 bytes. The snapshot tree is 89112092 bytes. `galion-ecapa` is present (28019424 bytes). `ecapa.onnx` is absent, so its size is `unmeasured` and the `ecapa` score is `unmeasured`.

### galion-mos (SQUIM MOS and objective SQUIM)

| | |
|---|---|
| Name | `galion-mos`. Subjective field `mos` (SQUIM subjective / NORESQA-style, child key `summary.mos.mean`). Objective field `squim` (`summary.stoi.mean`, `summary.pesq.mean`, `summary.si_sdr.mean`). |
| Lookup | `cloning-v2/rollout-blendfix/scorers/quality/bin/galion-mos`. Models: `squim_subjective.onnx` and `squim_objective.onnx` under `$GALION_SQUIM_MODELS`, `scorers/quality/models`, or `scorers/quality/galion-quality/models`. |
| URL | No SQUIM URL is recorded in this git checkout. `docs/galion-voice-v2/evals/premium-voices/PREMIUM-VOICES.md` names torchaudio `SQUIM_OBJECTIVE`. |
| Version | `galion-quality` `0.1.0` (same on-disk crate as `galion-wer`). |
| Licence | MIT for that Galion crate. An upstream SQUIM licence is not recorded in this repo. |

On this host `galion-mos` is present (804896 bytes). Both ONNX files are absent, so their sizes are `unmeasured` and `mos` and `squim` are `unmeasured`.

### emotion_match

| | |
|---|---|
| Name | Persona `emotion_match`. Formula name, used only as text: `0.70*category_blend+0.30*emb_sim`. |
| Lookup | An executable named `emotion_match`, `emotion-match`, or `galion-emotion-match` under `cloning-v2/rollout-blendfix/scorers` or `cloning-v2/scorers`, whose own text or a sibling `FORMULA` / `formula.txt` / `formula.name` contains that formula name. |
| URL | None in this repo. |
| Version | None in this repo. |
| Licence | None in this repo. |

No such scorer is on this host. `emotion_match` is the string `unmeasured`. The wrapper does not multiply category blend by 0.70 or embedding similarity by 0.30. `galion-emotion` is a different tool and is not treated as this scorer.

## Threads, SIMD, iGPU

Threads default to 2 and are clamped to 1..=2, matching the existing scorers. The cap is passed as `--jobs` (WER) or `--threads` (MOS, ECAPA, emotion_match) and as `OMP_NUM_THREADS`, `OPENBLAS_NUM_THREADS`, and `MKL_NUM_THREADS`. Each child is started with `CUDA_VISIBLE_DEVICES` empty and, when `/usr/bin/nice` exists, `nice -n 10`.

Runtime SIMD is not required. The JSON field `simd` is `not-required`. The crate uses no `std::arch` path.

The `igpu` Cargo feature is optional and empty. It adds no dependency, so wgpu is not downloaded. With or without the feature the device stays `cpu` and `igpu` is `not-wired`. There is no `cuda` feature. CUDA remains the offline cbx path.

## This host

Probe: `galion-score-wrap --probe --root /home/galion/workspace/galion` (no clip, no GPU).

| Field | Value |
|---|---|
| `wer` | `unmeasured` (binary and Vosk present; no clip) |
| `ecapa` | `unmeasured` (`ecapa.onnx` absent; HF snapshot present) |
| `mos` | `unmeasured` |
| `squim` | `unmeasured` |
| `emotion_match` | `unmeasured` |
| `device` | `cpu` |
| `cuda` | false |
| `igpu` | `not-wired` |
| `simd` | `not-required` |
| `threads` | 2 |

## Tests and aarch64 check

`cargo test --offline` in `tools/cloning-v2/score-wrap`: 11 passed. The same 11 passed with `--features igpu`. The tests cover the unmeasured path, a stub shell-out that passes a child number through, and a refusal to treat `category_blend` plus `emb_sim` as `emotion_match`.

`cargo check --offline --target aarch64-unknown-linux-gnu` finished successfully, and the same check with `--features igpu` finished successfully. That is a cross check on this x86_64 host. It is not a run on a Pi board, and it is not a timing.

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
