// Galion Voice — Perpetual Open MIT License
// SPDX-License-Identifier: MIT
//! int8 GEMV with **per-row** weight scales (W8A8, symmetric).
//!
//! `y[i] = (Σₚ Wq[i,p]·xq[p]) as f32 · (w_scale[i] · x_scale) (+ b[i])`
//!
//! * Weights: row-major `[m, k]` i8, one f32 scale per output row (`real = q · scale`),
//!   produced by [`QuantRowsI8::quantize`] (absmax / 127, round-to-nearest, clamp ±127).
//! * Activations: quantized once per call with one symmetric scale ([`quantize_row_i8`]),
//!   into a caller-provided scratch buffer — no allocation in the frame loop.
//! * The i8·i8 dot accumulates exactly in i32 (guarded by [`DOT_I8_MAX_K`]), and the f32
//!   epilogue is the same expression at every level, so **every SIMD level and exec mode is
//!   bit-exact vs the scalar oracle** ([`gemv_i8_scalar_ref`]).
//! * Not the `.gq` block layout yet (A7 owns that); per-row scales are the simplest layout that a
//!   `.gq` tensor can map onto. Per-block scales can be added as a second kernel later.

use crate::features::{self, SimdLevel};
use crate::simd::{dot_i8_for, dot_i8_scalar, DotI8Fn, DOT_I8_MAX_K};
use crate::{Exec, Result, TensorError};

/// Rows per rayon task.
pub const GEMV_I8_ROW_BLOCK: usize = 64;
/// `Exec::Auto` goes parallel at this many weights (i8 is ~4× cheaper per element than f32).
pub const GEMV_I8_PAR_THRESHOLD: usize = 1024 * 1024;

/// Symmetric absmax quantization of `src` into `dst` (same length). Returns the scale
/// (`real ≈ q · scale`); an all-zero (or empty) input yields scale `1.0` and zeros.
/// No allocation.
pub fn quantize_row_i8(src: &[f32], dst: &mut [i8]) -> f32 {
    debug_assert_eq!(src.len(), dst.len());
    let amax = src.iter().fold(0f32, |m, v| m.max(v.abs()));
    if amax == 0.0 || !amax.is_finite() {
        dst.iter_mut().for_each(|q| *q = 0);
        return 1.0;
    }
    let scale = amax / 127.0;
    let inv = 127.0 / amax;
    for (q, &v) in dst.iter_mut().zip(src) {
        *q = (v * inv).round().clamp(-127.0, 127.0) as i8;
    }
    scale
}

/// Row-major i8 weight matrix with one f32 scale per row.
#[derive(Debug, Clone, PartialEq)]
pub struct QuantRowsI8 {
    pub data: Vec<i8>,
    pub scales: Vec<f32>,
    pub m: usize,
    pub k: usize,
}

impl QuantRowsI8 {
    /// Quantize a row-major f32 `[m, k]` matrix, per row.
    pub fn quantize(a: &[f32], m: usize, k: usize) -> Result<Self> {
        if a.len() != m * k {
            return Err(TensorError::InvalidShape(format!("quantize: A has {} elems (m={m}, k={k})", a.len())));
        }
        let mut data = vec![0i8; m * k];
        let mut scales = vec![0f32; m];
        for i in 0..m {
            scales[i] = quantize_row_i8(&a[i * k..(i + 1) * k], &mut data[i * k..(i + 1) * k]);
        }
        Ok(QuantRowsI8 { data, scales, m, k })
    }
    /// Wrap pre-quantized parts (e.g. from a future `.gq` tensor).
    pub fn from_parts(data: Vec<i8>, scales: Vec<f32>, m: usize, k: usize) -> Result<Self> {
        if data.len() != m * k || scales.len() != m {
            return Err(TensorError::InvalidShape(format!("QuantRowsI8: data {} / scales {} for m={m}, k={k}", data.len(), scales.len())));
        }
        Ok(QuantRowsI8 { data, scales, m, k })
    }
    /// Dequantize row `i` into `out` (len `k`).
    pub fn dequantize_row(&self, i: usize, out: &mut [f32]) {
        let s = self.scales[i];
        for (o, &q) in out.iter_mut().zip(&self.data[i * self.k..(i + 1) * self.k]) { *o = q as f32 * s; }
    }
    /// Bytes of weight payload (data + scales).
    pub fn nbytes(&self) -> usize { self.data.len() + 4 * self.scales.len() }
}

/// W8A8 GEMV with pre-quantized activations, explicit level. Allocation-free.
#[allow(clippy::too_many_arguments)]
pub fn gemv_i8_into_with(
    level: SimdLevel,
    wq: &[i8],
    w_scales: &[f32],
    m: usize,
    k: usize,
    xq: &[i8],
    x_scale: f32,
    bias: Option<&[f32]>,
    y: &mut [f32],
    exec: Exec,
) -> Result<()> {
    let bad = |what: &str| TensorError::InvalidShape(format!("gemv_i8: {what} (m={m}, k={k})"));
    if wq.len() != m * k { return Err(bad(&format!("W has {} elems", wq.len()))); }
    if w_scales.len() != m { return Err(bad(&format!("scales has {} elems", w_scales.len()))); }
    if xq.len() != k { return Err(bad(&format!("x has {} elems", xq.len()))); }
    if y.len() != m { return Err(bad(&format!("y has {} elems", y.len()))); }
    if let Some(b) = bias { if b.len() != m { return Err(bad(&format!("bias has {} elems", b.len()))); } }
    if k > DOT_I8_MAX_K { return Err(bad(&format!("k exceeds exact-i32 bound {DOT_I8_MAX_K}"))); }
    let dot = dot_i8_for(level);
    let par = match exec {
        Exec::Auto => cfg!(feature = "parallel") && m * k >= GEMV_I8_PAR_THRESHOLD && m > GEMV_I8_ROW_BLOCK,
        e => e.use_parallel(m * k),
    };
    #[cfg(feature = "parallel")]
    if par {
        use rayon::prelude::*;
        y.par_chunks_mut(GEMV_I8_ROW_BLOCK).enumerate().for_each(|(blk, ys)| {
            rows_i8(dot, wq, w_scales, k, xq, x_scale, bias, blk * GEMV_I8_ROW_BLOCK, ys)
        });
        return Ok(());
    }
    let _ = par;
    rows_i8(dot, wq, w_scales, k, xq, x_scale, bias, 0, y);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
#[inline]
fn rows_i8(dot: DotI8Fn, wq: &[i8], ws: &[f32], k: usize, xq: &[i8], xs: f32, bias: Option<&[f32]>, row0: usize, ys: &mut [f32]) {
    for (j, yo) in ys.iter_mut().enumerate() {
        let i = row0 + j;
        *yo = epilogue(dot(&wq[i * k..(i + 1) * k], xq), ws[i], xs, bias.map(|b| b[i]));
    }
}

/// Shared f32 epilogue (identical at every level => bit-exact).
#[inline(always)]
fn epilogue(acc: i32, w_scale: f32, x_scale: f32, b: Option<f32>) -> f32 {
    let v = acc as f32 * (w_scale * x_scale);
    match b { Some(b) => v + b, None => v }
}

/// W8A8 GEMV on a [`QuantRowsI8`] with pre-quantized activations, selected SIMD level.
pub fn gemv_i8_into(w: &QuantRowsI8, xq: &[i8], x_scale: f32, bias: Option<&[f32]>, y: &mut [f32], exec: Exec) -> Result<()> {
    gemv_i8_into_with(features::selected(), &w.data, &w.scales, w.m, w.k, xq, x_scale, bias, y, exec)
}

/// Convenience: quantize f32 activations into `x_scratch` (len `k`, reused by the caller), then
/// run W8A8 GEMV. Allocation-free.
pub fn gemv_i8_f32x_into(w: &QuantRowsI8, x: &[f32], x_scratch: &mut [i8], bias: Option<&[f32]>, y: &mut [f32], exec: Exec) -> Result<()> {
    if x.len() != w.k || x_scratch.len() != w.k {
        return Err(TensorError::InvalidShape(format!("gemv_i8_f32x: x {} / scratch {} for k={}", x.len(), x_scratch.len(), w.k)));
    }
    let xs = quantize_row_i8(x, x_scratch);
    gemv_i8_into(w, x_scratch, xs, bias, y, exec)
}

/// Scalar oracle (allocating, test/bench use only).
pub fn gemv_i8_scalar_ref(w: &QuantRowsI8, xq: &[i8], x_scale: f32, bias: Option<&[f32]>) -> Vec<f32> {
    (0..w.m)
        .map(|i| epilogue(dot_i8_scalar(&w.data[i * w.k..(i + 1) * w.k], xq), w.scales[i], x_scale, bias.map(|b| b[i])))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEVELS: [SimdLevel; 4] = [SimdLevel::Scalar, SimdLevel::Avx2, SimdLevel::Avx512, SimdLevel::Neon];

    fn data(n: usize, s: usize) -> Vec<f32> {
        (0..n).map(|i| (((i * s + 3) % 29) as f32 - 14.0) * 0.0371 + ((i * 13) as f32 * 0.01).cos() * 0.2).collect()
    }
    fn bits(v: &[f32]) -> Vec<u32> { v.iter().map(|f| f.to_bits()).collect() }

    #[test]
    fn quantize_row_roundtrip_and_edge_cases() {
        let src = [0.0, 1.0, -2.0, 0.5, 2.0];
        let mut q = [0i8; 5];
        let s = quantize_row_i8(&src, &mut q);
        assert_eq!(s, 2.0 / 127.0);
        assert_eq!(q, [0, 64, -127, 32, 127]); // 63.5 rounds away from zero
        for (v, &qq) in src.iter().zip(&q) { assert!((qq as f32 * s - v).abs() <= s / 2.0 + 1e-7); }
        let mut z = [5i8; 3];
        assert_eq!(quantize_row_i8(&[0.0; 3], &mut z), 1.0);
        assert_eq!(z, [0; 3]);
        assert_eq!(quantize_row_i8(&[], &mut []), 1.0);
        let w = QuantRowsI8::quantize(&[1.0, -1.0, 0.0, 0.0, 4.0, 2.0], 3, 2).unwrap();
        assert_eq!(w.scales, vec![1.0 / 127.0, 1.0, 4.0 / 127.0]);
        assert_eq!(w.data, vec![127, -127, 0, 0, 127, 64]);
        let mut row = [0f32; 2];
        w.dequantize_row(2, &mut row);
        assert!((row[0] - 4.0).abs() < 1e-6 && (row[1] - 2.0).abs() < 0.02);
        assert!(QuantRowsI8::quantize(&[0.0; 5], 2, 3).is_err());
        assert!(QuantRowsI8::from_parts(vec![0; 6], vec![1.0; 2], 3, 2).is_err());
    }

    #[test]
    fn gemv_i8_all_levels_and_exec_bit_exact_vs_scalar_oracle() {
        for &(m, k) in &[(1, 1), (1, 31), (7, 1), (3, 33), (16, 64), (17, 65), (65, 96), (130, 257), (300, 1000)] {
            let w = QuantRowsI8::quantize(&data(m * k, 7), m, k).unwrap();
            let x = data(k, 5);
            let b = data(m, 3);
            let mut xq = vec![0i8; k];
            let xs = quantize_row_i8(&x, &mut xq);
            let r = gemv_i8_scalar_ref(&w, &xq, xs, Some(&b));
            for level in LEVELS {
                for exec in [Exec::Scalar, Exec::Parallel, Exec::Auto] {
                    let mut y = vec![f32::NAN; m];
                    gemv_i8_into_with(level, &w.data, &w.scales, m, k, &xq, xs, Some(&b), &mut y, exec).unwrap();
                    assert_eq!(bits(&y), bits(&r), "m={m} k={k} {level:?} {exec:?}");
                }
            }
            let mut y = vec![0f32; m];
            let mut scratch = vec![0i8; k];
            gemv_i8_f32x_into(&w, &x, &mut scratch, Some(&b), &mut y, Exec::Auto).unwrap();
            assert_eq!(bits(&y), bits(&r));
            assert_eq!(scratch, xq);
        }
    }

    #[test]
    fn gemv_i8_accuracy_vs_f32_within_quantization_bound() {
        let (m, k) = (200, 512);
        let a = data(m * k, 11);
        let x = data(k, 17);
        let w = QuantRowsI8::quantize(&a, m, k).unwrap();
        let mut xq = vec![0i8; k];
        let xs = quantize_row_i8(&x, &mut xq);
        let mut y = vec![0f32; m];
        gemv_i8_into(&w, &xq, xs, None, &mut y, Exec::Auto).unwrap();
        for i in 0..m {
            let row = &a[i * k..(i + 1) * k];
            let exact: f64 = row.iter().zip(&x).map(|(p, q)| *p as f64 * *q as f64).sum();
            // |Σ ax - âx̂| ≤ Σ (|a|·δx + |x|·δa + δa·δx), δa = sa/2, δx = sx/2 (+ f32 epilogue slack).
            let (da, dx) = (w.scales[i] as f64 / 2.0, xs as f64 / 2.0);
            let bound: f64 = row.iter().zip(&x).map(|(p, q)| (*p as f64).abs() * dx + (*q as f64).abs() * da + da * dx).sum::<f64>()
                + 1e-5 * (1.0 + exact.abs());
            assert!((y[i] as f64 - exact).abs() <= bound, "row {i}: {} vs {exact} (bound {bound})", y[i]);
        }
    }

    #[test]
    fn gemv_i8_shape_errors() {
        let w = QuantRowsI8::quantize(&[0.5; 12], 4, 3).unwrap();
        let mut y = vec![0f32; 4];
        assert!(gemv_i8_into(&w, &[0; 2], 1.0, None, &mut y, Exec::Scalar).is_err());
        assert!(gemv_i8_into(&w, &[0; 3], 1.0, Some(&[0.0; 3]), &mut y, Exec::Scalar).is_err());
        assert!(gemv_i8_into(&w, &[0; 3], 1.0, None, &mut [0f32; 3], Exec::Scalar).is_err());
        assert!(gemv_i8_f32x_into(&w, &[0.0; 3], &mut [0; 2], None, &mut y, Exec::Scalar).is_err());
        assert!(gemv_i8_into_with(SimdLevel::Scalar, &[0; 12], &[1.0; 3], 4, 3, &[0; 3], 1.0, None, &mut y, Exec::Scalar).is_err());
        let mut e: Vec<f32> = vec![];
        gemv_i8_into_with(SimdLevel::Scalar, &[], &[], 0, 3, &[0; 3], 1.0, None, &mut e, Exec::Auto).unwrap();
        let mut y2 = vec![9f32; 2];
        gemv_i8_into_with(SimdLevel::Avx2, &[], &[1.0, 1.0], 2, 0, &[], 1.0, Some(&[3.0, 4.0]), &mut y2, Exec::Auto).unwrap();
        assert_eq!(y2, vec![3.0, 4.0]);
    }
}
