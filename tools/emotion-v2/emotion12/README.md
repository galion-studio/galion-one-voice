# galion-emotion12 — emotion2vec+ gate scorer (glue)

Galion Voice — Perpetual Open MIT License. Galion Labs.

Gate decision (Mate, card t1359s6830): **per-emotion emotion2vec+ scoring, gate 0.83 unchanged**,
14 emotions = the 12 Emotion V2 emotions + laughter + seductive. Universal: the voice is an input
(per-emotion reference clips), there is no voice-specific code.

This crate is glue only. It is its own cargo workspace and is **not** a member of the repo
workspace, so `ort` stays out of `cargo tree --workspace` (LOCKS.toml unchanged).

## Bricks reused (pinned, licence recorded — licences on hold, t1360u)

| Brick | Pin | Licence | Role |
|---|---|---|---|
| emotion2vec+ base, ONNX export `ziyu12345/emotion2vec_plus_base_onnx` | rev `041e7567f80554b846645a87b6d4d032ee949573` (`emotion2vec_plus_base.onnx` 373,159,295 B + `emotion2vec_head.json`) | export packaging MIT; weights FunASR Model Licence v1.1 (attribute, keep model name) | default scorer: 9-class probs + 768-d embedding. Already the repo's universal-scorer brick (`tools/cloning-v2/scorers/universal`) |
| emotion2vec+ large, ONNX export `Voldorn/emotion2vec-plus-large-onnx` (from `emotion2vec/emotion2vec_plus_large` rev `6c303ba9…`) | rev `cdcbc0964e2fa3092f79937287d4222056bc12fc` (648,984,830 B) | FunASR Model Licence v1.1 | optional `--model large`: probs from large, embedding from base |
| ONNX Runtime GPU build `onnxruntime-win-x64-gpu-1.22.0` | GitHub release v1.22.0 | MIT | runtime (`ORT_DYLIB_PATH`); CUDA EP when a GPU is present, CPU otherwise |
| `ort` crate | `=2.0.0-rc.10` (same as `tools/cloning-v2/scorers/emotion`) | MIT OR Apache-2.0 | Rust binding |
| `hound`, `serde`, `serde_json`, `toml`, `anyhow` | Cargo.toml | MIT / Apache-2.0 | WAV, JSON, config |
| `src/audio.rs` | copied unchanged from `galion-emotion` (`tools/cloning-v2/scorers/emotion`) | Apache-2.0 OR MIT | WAV load + Kaiser resampler |
| `sector_refs.py` (per-sector manifest, sha256 pins) | `tools/cloning-v2/emotion-match` | repo MIT | per-emotion references for `emotion2vec_gate` |

Inference recipe is FunASR's `granularity="utterance"`: waveform layer-norm (`normalize: true` in the
emotion2vec+ config), encoder frames, mean, `proj`, softmax. The large export bakes that in; for the
base export the binary applies the layer-norm (`--raw` skips it).

## Emotion set

`gate-emotions.toml`: `[[emotions]] id, emotion2vec, emotion2vec_in_set`. emotion2vec+ has 7 usable
classes (angry, disgusted, fearful, happy, neutral, sad, surprised; `other`/`unknown` dropped). Seven
of the 14 emotions are native. The other seven read a shared class, split evenly: calm←neutral;
amused, excited, tender, laughter, seductive←happy; disappointed←sad. Any file with the same keys
(e.g. A3's 14-emotion config) works via `--emotions`.

**What that means for the gate:** per-emotion scoring compares a render with *that emotion's own
reference*, so a shared-class emotion still scores high when the render sounds like its reference.
But emotion2vec+ cannot tell calm from neutral, or tender/amused/excited/laughter/seductive from
happy: a render that drifts between emotions inside one family is not caught. That is a model
limit, not hidden by this glue.

## Use

```
set ORT_DYLIB_PATH=G:\galion\toolchains\onnxruntime\onnxruntime-win-x64-gpu-1.22.0\lib\onnxruntime.dll
set PATH=<that lib dir>;<CUDA 12 + cuDNN 9 DLL dir>;%PATH%
cargo build --release
galion-emotion12 score --device auto --out scores.json clip1.wav clip2.wav
galion-emotion12 bench --corpus crema-d --root <CREMA-D> --out crema.json
galion-emotion12 coverage
```

`--device cpu` is the production default path; `auto` uses CUDA only when the CUDA EP is available.

Wired in as the default gate scorer (falls back when the binary or models are absent):
- `tools/emotion-v2/harness` (`score`, `sweep`): backend `auto` → emotion2vec+ when built, else SUPERB, else stub/unmeasured. `--backend emotion2vec+` forces it.
- `tools/cloning-v2/emotion-match --manifest` (per-emotion mode): default backend `emotion2vec+`, one batch run for all refs + renders. `laughter` added next to `seductive` as optional sectors.
- `tools/emotion-v2/emotion2vec_gate.py`: `--scorer-module emotion2vec_gate` for the dataset pipeline (#182). Returns `{emotion: 0.70*category_blend+0.30*emb_sim}` against each emotion's reference (`EMOTION_V2_SECTOR_REFS` manifest); the pipeline checks the clip's own emotion at 0.83.

Gate values (0.83, WER 0.018, ECAPA 0.7057, 0.70/0.30) are unchanged and asserted in
`harness/tests/test_scorer12.py`.

---

G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network
Developed and distributed by Galion Labs
Perpetual Open MIT License
Copyright © 2026 Galion Labs. All rights reserved.
