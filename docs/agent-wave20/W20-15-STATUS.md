# W20-15 · Soft blocker A6 Mimi — real kernels on synthetic tensors

Date: 2026-11-02 (Europe/Zagreb) · Scope: LOCAL ONLY (no git push/gh/VPS, no weights, no he-IL)

## Result
The Mimi streaming **transformer bottleneck is now a real kernel**. It was the last identity
placeholder in `crates/galion-mimi`. With a `WeightSource`, every stage of the codec now runs on real
kernels: SEANet enc/dec, 25⇄12.5 Hz resample, the enc/dec transformers, and split RVQ.
**`IS_STUB` stays `true`.** No real weights exist, no golden frames have been checked, and the
hyper-parameters and conventions marked `VERIFY` have not been checked against upstream.

## What changed (crates/galion-mimi only; galion-tensor/A5 untouched)
- `src/transformer.rs`
  - `StreamingTransformer::{new, step, step_token, reset, position}`: pre-norm layers, run once per 25 Hz step:
    LayerNorm → in_proj `[q;k;v]` (A4 `gemv_f32_into`, `Exec::Scalar`) → A5 `StreamingAttention`
    (RoPE-NeoX + `KvRing`, window = `transformer_context`) → out_proj → LayerScale residual;
    LayerNorm → fc1 → exact-erf GELU → fc2 → LayerScale residual. No biases on linears (VERIFY).
    All scratch is allocated in `new`, and `step` does no allocation (covered by the existing `tests/zero_alloc.rs` codec loop).
  - Local primitives that galion-tensor lacks: `layer_norm_into` (eps `LN_EPS`=1e-5), `erf`
    (A&S 7.1.26, ≤1.5e-7) and `gelu_erf_inplace`. Constant `ROPE_THETA`=10 000.
  - `layer_tensors()` is now the single source for transformer weight names/shapes, and `weights::expected_tensors` uses it.
    The names and shapes are unchanged.
  - `permute_qk_interleaved_to_neox()` is a **convert-time** helper for A7. Upstream Moshi/Mimi uses interleaved RoPE pairs,
    while galion-tensor uses NeoX halves. Permuting the q/k rows of `in_proj` per head makes the two exactly equivalent.
  - `reference::forward`: a whole-sequence batch reference built on `attention_reference`. It works layer by layer.
  - `TransformerStub` is kept only for the no-weights path, where it is an identity on all-zero data.
- `src/codec.rs`: `Kernels` now holds `enc_tf`/`dec_tf: StreamingTransformer`, which are used in the
  encode/decode latent paths and in `reset`.
- `src/lib.rs`, `src/weights.rs`: docs updated, and the contract now uses `layer_tensors`.
- `tests/stream_vs_batch.rs`: the codec-level chain test now includes the transformer reference
  (`codec_conv_chain_matches_batch_reference`). The codec stream is still bit-identical to the batch result.
- **New** `tests/transformer.rs` (6 tests, synthetic in-memory tensors only):
  1. Streaming is bit-identical to the batch reference, including past the KV window (ctx 4/8/32, 1–2 layers, chunk 1–3).
  2. Matches an independent **f64 oracle** with its own LayerNorm, MHA, RoPE and series erf GELU; rel err < 1e-5.
  3. Weights written for interleaved RoPE, after the NeoX permutation, match the interleaved oracle within 1e-5. The unpermuted run differs by more than 1e-3, so the test is not vacuous.
  4. LayerScale = 0 gives the identity. `reset` restarts exactly, a bad shape is rejected, and `IS_STUB` is still true.
  5. LayerNorm and erf/GELU primitives are checked against known values.
  6. The full default config (512-d, 8 heads, ffn 2048, ctx 250) builds and runs on `ZeroWeights`.

## Verification (local)
- `cargo test -p galion-mimi --offline`: all pass (6 + 8 + 6 + 3, plus 2 ignored).
- `cargo test --workspace --offline`: **288 passed, 0 failed**. `cargo build --workspace --tests` has 0 warnings.
- `cargo test --release -p galion-mimi -- --ignored full_size`: the full-width codec, now with real
  transformers on zero weights, takes 2.6 s for 2 frames enc+dec on the scalar path. That is **not real-time**; perf work (SIMD/parallel
  GEMV, int8) is a later task. clippy is not installed on the box, so it was not run.
- Backup of the pre-change crate: `/workspace/w20-15-backup/galion-mimi/`.

## Still blocking `IS_STUB = false` (unchanged owners)
1. **A21**: licence clearance for the CC-BY Mimi weights. `weights::load_codec` still refuses (`NotImplementedStub`).
2. **A7 / W20-14**: a `.gq` → `WeightSource` adapter, plus an upstream→Galion name map. It must apply weight-norm folding,
   EMA-codebook folding and the **new `permute_qk_interleaved_to_neox` step** at convert time.
3. **Golden frames** against the upstream reference within tolerance (contract item 6).
4. Upstream checks still marked `VERIFY`: LN eps, RoPE base/convention, erf versus tanh GELU, the no-bias linears, context
   semantics (250 steps including self), the 25 Hz transformer placement, and replicate padding on the downsample (a known delta on the first frame).

No weights were invented. All tensors used in tests are deterministic pseudo-random values generated in memory from the tensor name (`tests/common`); they are never saved.

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
