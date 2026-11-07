# PerTh watermark investigation (Rust cbx / cbx-v2)

## Findings
1. **Python `perth` is installed** in `/workspace/swarm/voice-v2-sprint/venv` (Resemble AI perceptual watermark toolkit).
2. **Rust cbx sources (upstream bakeoff-t648 and this cbx-v2 fork) contain zero references** to `perth`, `watermark`, or Resemble watermark APIs (`rg` over `src/`).
3. **Nothing was removed** in this fork; watermark support was never present on the Rust path.
4. Turbo ONNX `conditional_decoder.onnx` inputs are only `speech_tokens`, `speaker_embeddings`, `speaker_features`. It contains `RandomNormalLike` / `RandomUniformLike` nodes — these are **decoder stochasticity**, not a watermark injector.
5. Implication: WAVs written by `cbx` / `cbx-v2` are **not PerTh-watermarked** unless a downstream Python step calls `perth`.

## Recommendation (compliance, not quality)
If product requires PerTh, post-process WAVs with the Python `perth` API after `cbx speak`. Do not bake into ONNX. Out of scope for this pipeline quality sprint unless explicitly requested.
