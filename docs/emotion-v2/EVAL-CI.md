# Emotion eval CI (laptop) — Samantha × 14

Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
Samantha · Galion Voice V2. Tuning standard: **432 Hz**.

One command scores any voice build (universal: a folder per emotion) across the emotions in
`tools/emotion-v2/eval/emotions.toml` (default **14** = the 12 V2 emotions + laughter + seductive, card t1359s6830)
and prints a pass/fail matrix against the gate:

| measure | tool (existing brick) | gate |
|---|---|---|
| emotion_match per emotion = 0.70·cos(mean posterior) + 0.30·cos(mean embedding) vs that emotion's reference clips | `galion-emotion matrix` (Rust, ONNX Runtime) on **emotion2vec+ base** | ≥ 0.83 |
| WER (mean over that emotion's clips, text from `<clip>.txt`) | `galion-wer` (Rust, vosk) | ≤ 0.018 |
| ECAPA (mean cosine vs speaker reference) | `galion-ecapa` (Rust, ONNX Runtime) | > 0.7057 |

PASS only when all three are measured and inside the gate. Anything missing is **UNMEASURED**, never PASS.
Pinned model files (`tools/emotion-v2/eval/MODELS.lock`) are sha256-checked first; a missing or changed file stops the
run with exit 2 ("NOT MEASURED"), so the 14 Nov "unmeasured" sweep can't recur silently.

```bash
# voice build: BUILD/<emotion>/*.wav (+ same-stem .txt), REF/<emotion>/*.wav, one speaker reference wav
scripts/eval/emotion-eval.sh --matrix --build-dir BUILD --reference-dir REF --speaker-ref SPK.wav --out OUT
# public benchmark fixture (CREMA-D, one actor as the "build", the other actors as emotion references)
scripts/eval/crema-d-fixture.sh /tmp/fx 1001
scripts/eval/emotion-eval.sh --matrix --build-dir /tmp/fx/build --reference-dir /tmp/fx/ref --speaker-ref /tmp/fx/speaker-ref.wav
# classifier benchmark (accuracy / UAR / confusion) for every row in scorers.tsv
scripts/eval/emotion-eval.sh --bench crema-d
```

Device: `EMOTION_EVAL_DEVICE=auto` (default for the eval) uses the CUDA build of ONNX Runtime when `nvidia-smi`
answers (laptop RTX 4060), else the CPU build with all logical CPUs. Every report logs the device actually used
(`cuda:0 (onnxruntime CUDAExecutionProvider)` or `cpu (N threads; CUDA fallback: <reason>)`). The `galion-emotion`
binary itself still defaults to `--device cpu`; production stays CPU-only.

Models live on the laptop under `G:\galion\models` (`/mnt/g/galion/models`), override with `GALION_MODELS`.

## Bricks reused (pinned) and licences

Licences are recorded for review before any public release (RULES: LICENCES ON HOLD, t1360u).

| brick | version / pin | licence | role |
|---|---|---|---|
| ONNX Runtime (Microsoft) | 1.31.1 `onnxruntime-linux-x64-gpu_cuda13` + CPU build | MIT | inference runtime |
| `ort` crate (pykeio) | 2.0.0-rc.10 (`load-dynamic`, `cuda`) | MIT OR Apache-2.0 | Rust bindings |
| emotion2vec+ base, ONNX export `ziyu12345/emotion2vec_plus_base_onnx` | rev `041e7567` | export MIT; weights FunASR Model Licence v1.1 (attribute "emotion2vec+") | gate emotion scorer |
| SUPERB `superb/wav2vec2-base-superb-er` (ONNX export, existing `py/export_superb.py`) | rev `441a7599` | Apache-2.0 | 4-class reference scorer (bench rows) |
| SpeechBrain `spkrec-ecapa-voxceleb` (ONNX export, existing `galion-ecapa`) | existing `ecapa.onnx` | Apache-2.0 | ECAPA speaker similarity |
| Vosk API (alphacep) | libvosk 0.3.45 | Apache-2.0 | ASR for WER |
| `vosk-model-small-en-us-0.15` | 0.15 | Apache-2.0 | ASR model |
| CREMA-D (CheyneyComputerScience) | commit `1658cd34`, sentences DFA+ITS, level XX, 91 actors (1091 clips) | ODbL 1.0 / DbCL 1.0 | public eval benchmark |
| NVIDIA CUDA 13 / cuDNN 9 runtime libs | from the existing laptop torch venv (`nvidia-*` wheels) | NVIDIA EULA | CUDA EP dependencies |
| `galion-emotion`, `galion-ecapa`, `galion-wer` | this repo | MIT / Apache-2.0 OR MIT | existing scorers; this change adds glue only |

New code (glue): `--device` + `--ser-head` + `bench` + `matrix` in `galion-emotion` (Rust), `galion-wer --jobs` up to
all CPU threads, `galion-ecapa` switched to `load-dynamic` so it uses the same pinned ONNX Runtime, two bash scripts.

## Same scorers in the dataset HQ gate (#182)

`tools/emotion-v2/eval/emotion_v2_ci.py` is the `emotion_v2_ci` scorer plugin that #182's
`--scorer-module` / `build_scorers(config)` interface asks for. It writes each rendered clip to a temp
tree and calls the same three Rust tools as `--matrix` (galion-wer, galion-ecapa, galion-emotion matrix
with a one-emotion config), so the dataset gate and the clone eval report identical numbers.
A tool that fails or a value it cannot measure raises; it is never a pass.

```bash
PYTHONPATH=tools/emotion-v2/eval:tools/emotion-v2/dataset \
GALION_EVAL_BIN=$CARGO_TARGET_DIR/release GALION_EVAL_SPEAKER_REF=SPK.wav GALION_EVAL_REFERENCE_DIR=REF \
ORT_DYLIB_PATH=.../libonnxruntime.so python -m emotion_v2_dataset run --scorer-module emotion_v2_ci ...
```

It is slow per clip (it re-embeds that emotion's references for every clip); use `--matrix` for whole builds.

## GitHub CI (no weights)

`.github/workflows/emotion-v2-ci.yml` job `gate-matrix-logic`: `bash -n` on the eval scripts, the
galion-emotion gate verdict unit tests (`cargo test`), and the plugin tests with fake tools and a
synthetic 432 Hz tone. The scored run needs the pinned models and stays on the laptop.

## Plug in another scorer

Change `[scorer]` in `emotions.toml` (any ONNX frame-feature model + linear head JSON in the emotion2vec export
format), or add a row to `scorers.tsv` for the benchmark. Pin the files in `MODELS.lock`.

## Consent

Test inputs are public (CREMA-D) or CC0 donor renders only. Never own-voice audio. The Samantha run uses the CC0
donor Samantha (Amy Koenig, LibriVox, public domain; `docs/galion-voice-v2/CLONE-CC0-DONORS.md`) until Slavik's
training-data-generation consent is signed.
