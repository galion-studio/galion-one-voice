# galion-tensor

Galion Voice CPU/VPS-first tensor runtime skeleton. MIT (Perpetual Open).

- `Tensor` = shared `Storage` (f32 / bf16 / i8 placeholders) + `Shape` + strides + offset; zero-copy `reshape`/`transpose` views.
- Ops (scalar reference path; rayon via `Exec::Parallel|Auto` with feature `parallel`, default on):
  matmul, add/mul (broadcast), softmax, rms_norm, rope_neox, gelu, silu, conv1d, embedding, top_k.
- GEMV fast path (`gemv.rs`): `gemv(a[m,k], x[k], bias?, exec)` and allocation-free `gemv_f32_into(..)`; scalar == rayon bit-exact (row-block split, 64 rows/task, Auto ≥ 512×512).
- SIMD (`features.rs`, `simd.rs`, A4): opt-in cargo features `simd-avx2`, `simd-avx512`, `simd-neon` + runtime CPU detection. **Real kernels**: AVX2+FMA (intrinsics), AVX-512 (stable `asm!`, x86_64; int8 needs AVX-512BW), NEON (aarch64 intrinsics). f32 dot vs scalar oracle = tolerance (`simd::dot_tolerance`); int8 dot and f32 axpy = bit-exact. Default build stays scalar. `GALION_SIMD=scalar|avx2|neon` caps the level (read once). `features::report()` prints a diagnostic line.
- int8 GEMV (`gemv_i8.rs`, A4): W8A8 symmetric, per-row weight scales (`QuantRowsI8`), activations quantized into a caller scratch buffer; bit-exact vs scalar at every level/exec; allocation-free.
- `matmul` with `rows == 1` routes to GEMV (zero-copy row-major GEMV when `b` is a `W^T` view; else `gemv_t` axpy sweep, bit-exact).
- Bench: `cargo bench -p galion-tensor --bench gemv --features simd-avx2,simd-avx512` (std-only, no criterion).
- Stubs (return `TensorError::Unimplemented`): `rope_interleaved`, `conv1d_causal_streaming`.
- Only dependency: `rayon` (optional). No third-party inference runtimes. Weights format: Galion `.gq` (future).

```
cargo test -p galion-tensor                         # scalar + rayon
cargo test -p galion-tensor --no-default-features   # pure scalar
cargo test -p galion-tensor --features simd-avx2,simd-avx512,simd-neon  # SIMD vs scalar oracle
./scripts/check-no-forbidden.sh                     # format/runtime-dep gate
```
