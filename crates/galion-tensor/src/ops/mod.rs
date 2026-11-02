// Galion Voice — Perpetual Open MIT License
//! Operator set. Every op has a scalar reference implementation; some have a rayon path.
//! All ops compute in f32 and accept any storage dtype (bf16/i8 are upcast on read).

pub mod conv;
pub mod elementwise;
pub mod embedding;
pub mod matmul;
pub mod norm;
pub mod rope;
pub mod topk;

pub use conv::{conv1d, conv1d_causal_streaming, conv_transpose1d, Conv1dParams, ConvTranspose1dParams};
pub use elementwise::{add, gelu, mul, silu};
pub use embedding::embedding;
pub use matmul::matmul;
pub use norm::{rms_norm, softmax};
pub use rope::{rope_interleaved, rope_neox};
pub use topk::top_k;

#[cfg(test)]
pub(crate) fn assert_close(a: &[f32], b: &[f32], tol: f32) {
    assert_eq!(a.len(), b.len(), "len mismatch");
    for (i, (x, y)) in a.iter().zip(b).enumerate() {
        assert!((x - y).abs() <= tol * (1.0 + y.abs()), "idx {i}: {x} vs {y}");
    }
}
