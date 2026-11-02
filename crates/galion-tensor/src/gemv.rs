// Galion Voice — Perpetual Open MIT License
// SPDX-License-Identifier: MIT
//! GEMV fast path: `y = A·x (+ b)` with `A` row-major `[m, k]`, `x` `[k]`, `b` `[m]`.
//!
//! Batch-1 decoding (one frame / one token at a time) is GEMV-bound, so this path skips the
//! general matmul machinery: one dot product per output row, no output-row allocation when the
//! caller uses [`gemv_f32_into`] with a reused buffer.
//!
//! Numerics: each row is `dot(A[i,:], x)` then `+ b[i]`. With the scalar oracle the dot is
//! accumulated sequentially. The rayon path splits by row blocks only, so for a fixed SIMD level
//! scalar-exec and parallel-exec are **bit-exact** (G-T2). SIMD dot kernels (AVX2 / AVX-512 /
//! NEON, see `simd.rs`) reassociate + fuse, so SIMD-vs-scalar is held to
//! [`crate::simd::dot_tolerance`] per row. Level comes from [`crate::features::selected`]
//! (opt-in cargo features + runtime CPU check; default build = scalar).
//!
//! [`gemv_t_f32_into`] is the transposed form `y[n] = x[k]·B[k,n]` (B row-major), used by
//! `matmul` when `rows == 1` and `B` is stored `[k, n]`: it is an axpy sweep that is bit-exact
//! vs the scalar matmul row kernel at every SIMD level.
//!
//! int8 GEMV (per-row scales) lives in `gemv_i8.rs`.

use crate::features::{self, SimdLevel};
use crate::simd::{axpy_for, dot_for, AxpyFn, DotFn};
use crate::{Exec, Result, Shape, Tensor, TensorError};

/// Rows per rayon task (amortizes scheduling; ~64×k f32 of `A` per task).
pub const GEMV_ROW_BLOCK: usize = 64;
/// `Exec::Auto` goes parallel when `m*k` is at least this (e.g. 512×512).
pub const GEMV_PAR_THRESHOLD: usize = 512 * 512;

/// Slice-level GEMV into a caller-provided buffer (no allocation). Uses the selected SIMD level.
pub fn gemv_f32_into(a: &[f32], m: usize, k: usize, x: &[f32], bias: Option<&[f32]>, y: &mut [f32], exec: Exec) -> Result<()> {
    gemv_f32_into_with(features::selected(), a, m, k, x, bias, y, exec)
}

/// Same as [`gemv_f32_into`] but with an explicit kernel level (tests / benchmarks).
#[allow(clippy::too_many_arguments)]
pub fn gemv_f32_into_with(level: SimdLevel, a: &[f32], m: usize, k: usize, x: &[f32], bias: Option<&[f32]>, y: &mut [f32], exec: Exec) -> Result<()> {
    let bad = |what: &str| TensorError::InvalidShape(format!("gemv: {what} (m={m}, k={k})"));
    if a.len() != m * k { return Err(bad(&format!("A has {} elems", a.len()))); }
    if x.len() != k { return Err(bad(&format!("x has {} elems", x.len()))); }
    if y.len() != m { return Err(bad(&format!("y has {} elems", y.len()))); }
    if let Some(b) = bias { if b.len() != m { return Err(bad(&format!("bias has {} elems", b.len()))); } }
    let dot = dot_for(level);
    let par = match exec {
        Exec::Auto => cfg!(feature = "parallel") && m * k >= GEMV_PAR_THRESHOLD && m > GEMV_ROW_BLOCK,
        e => e.use_parallel(m * k),
    };
    #[cfg(feature = "parallel")]
    if par {
        use rayon::prelude::*;
        y.par_chunks_mut(GEMV_ROW_BLOCK).enumerate().for_each(|(blk, ys)| {
            rows_kernel(dot, a, k, x, bias, blk * GEMV_ROW_BLOCK, ys)
        });
        return Ok(());
    }
    let _ = par;
    rows_kernel(dot, a, k, x, bias, 0, y);
    Ok(())
}

#[inline]
fn rows_kernel(dot: DotFn, a: &[f32], k: usize, x: &[f32], bias: Option<&[f32]>, row0: usize, ys: &mut [f32]) {
    for (j, yo) in ys.iter_mut().enumerate() {
        let i = row0 + j;
        let mut acc = dot(&a[i * k..(i + 1) * k], x);
        if let Some(b) = bias { acc += b[i]; }
        *yo = acc;
    }
}

/// Columns per rayon task in [`gemv_t_f32_into`] (multiple of 16 so SIMD lanes stay full).
pub const GEMV_T_COL_BLOCK: usize = 256;
/// `Exec::Auto` parallelizes `gemv_t` only from this many output columns. Bench (A4,
/// benches/gemv.rs, 8-thread x86): column-split is *slower* than 1 thread at n ≤ 1024
/// (every task re-walks all k rows), ~2× faster at n = 4096.
pub const GEMV_T_PAR_MIN_COLS: usize = 2048;

/// Transposed GEMV into a caller buffer: `y[n] = x[k] · B[k, n] (+ bias)`, `B` row-major.
/// Per output element the accumulation order is `p = 0..k` with `x[p] == 0` skipped and a
/// non-fused multiply-add, i.e. **bit-exact** with the scalar `matmul` row kernel at every
/// SIMD level and exec mode. No allocation.
#[allow(clippy::too_many_arguments)]
pub fn gemv_t_f32_into(level: SimdLevel, x: &[f32], b: &[f32], k: usize, n: usize, bias: Option<&[f32]>, y: &mut [f32], exec: Exec) -> Result<()> {
    let bad = |what: &str| TensorError::InvalidShape(format!("gemv_t: {what} (k={k}, n={n})"));
    if b.len() != k * n { return Err(bad(&format!("B has {} elems", b.len()))); }
    if x.len() != k { return Err(bad(&format!("x has {} elems", x.len()))); }
    if y.len() != n { return Err(bad(&format!("y has {} elems", y.len()))); }
    if let Some(bb) = bias { if bb.len() != n { return Err(bad(&format!("bias has {} elems", bb.len()))); } }
    let axpy = axpy_for(level);
    let par = match exec {
        Exec::Auto => cfg!(feature = "parallel") && n * k >= GEMV_PAR_THRESHOLD && n >= GEMV_T_PAR_MIN_COLS,
        e => e.use_parallel(n * k),
    };
    #[cfg(feature = "parallel")]
    if par {
        use rayon::prelude::*;
        y.par_chunks_mut(GEMV_T_COL_BLOCK).enumerate().for_each(|(blk, ys)| {
            cols_kernel(axpy, x, b, n, bias, blk * GEMV_T_COL_BLOCK, ys)
        });
        return Ok(());
    }
    let _ = par;
    cols_kernel(axpy, x, b, n, bias, 0, y);
    Ok(())
}

#[inline]
fn cols_kernel(axpy: AxpyFn, x: &[f32], b: &[f32], n: usize, bias: Option<&[f32]>, col0: usize, ys: &mut [f32]) {
    ys.iter_mut().for_each(|v| *v = 0.0);
    let w = ys.len();
    for (p, &xv) in x.iter().enumerate() {
        if xv == 0.0 { continue; }
        axpy(xv, &b[p * n + col0..p * n + col0 + w], ys);
    }
    if let Some(bb) = bias {
        for (v, &c) in ys.iter_mut().zip(&bb[col0..col0 + w]) { *v += c; }
    }
}

/// Tensor-level GEMV. `a`: `[m, k]` (any strides/dtype; non-contiguous is materialized),
/// `x`: `[k]` or `[1, k]`, `bias`: optional `[m]`. Returns `[m]` (or `[1, m]` if `x` was `[1, k]`).
pub fn gemv(a: &Tensor, x: &Tensor, bias: Option<&Tensor>, exec: Exec) -> Result<Tensor> {
    if a.shape().rank() != 2 {
        return Err(TensorError::InvalidShape("gemv: A must be rank 2 [m, k]".into()));
    }
    let (m, k) = (a.dims()[0], a.dims()[1]);
    let x_row = match x.dims() {
        [kk] if *kk == k => false,
        [1, kk] if *kk == k => true,
        _ => return Err(TensorError::ShapeMismatch { op: "gemv", lhs: a.dims().to_vec(), rhs: x.dims().to_vec() }),
    };
    if let Some(b) = bias {
        if b.dims() != [m] {
            return Err(TensorError::ShapeMismatch { op: "gemv(bias)", lhs: vec![m], rhs: b.dims().to_vec() });
        }
    }
    let (ac, xc) = (a.contiguous(), x.contiguous());
    let bc = bias.map(|b| b.contiguous());
    let mut y = vec![0f32; m];
    gemv_f32_into(
        ac.as_f32_slice().expect("contiguous f32"),
        m,
        k,
        xc.as_f32_slice().expect("contiguous f32"),
        bc.as_ref().map(|b| b.as_f32_slice().expect("contiguous f32")),
        &mut y,
        exec,
    )?;
    let dims = if x_row { vec![1, m] } else { vec![m] };
    Tensor::from_vec(y, Shape(dims))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::{assert_close, matmul};
    use crate::simd::dot_tolerance;

    fn naive(a: &[f32], m: usize, k: usize, x: &[f32], b: Option<&[f32]>) -> Vec<f32> {
        let mut y = vec![0f32; m];
        for i in 0..m {
            let mut acc = 0f32;
            for p in 0..k { acc += a[i * k + p] * x[p]; }
            if let Some(b) = b { acc += b[i]; }
            y[i] = acc;
        }
        y
    }
    fn data(n: usize, s: usize) -> Vec<f32> {
        (0..n).map(|i| (((i * s + 3) % 29) as f32 - 14.0) * 0.0371).collect()
    }
    fn bits(v: &[f32]) -> Vec<u32> { v.iter().map(|f| f.to_bits()).collect() }

    #[test]
    fn gemv_small_exact() {
        let a = Tensor::from_vec(vec![1., 2., 3., 4., 5., 6.], [2, 3]).unwrap();
        let x = Tensor::from_vec(vec![1., 0., -1.], [3]).unwrap();
        let b = Tensor::from_vec(vec![10., 20.], [2]).unwrap();
        assert_eq!(gemv(&a, &x, None, Exec::Scalar).unwrap().to_vec_f32(), vec![-2., -2.]);
        let y = gemv(&a, &x, Some(&b), Exec::Scalar).unwrap();
        assert_eq!(y.dims(), &[2]);
        assert_eq!(y.to_vec_f32(), vec![8., 18.]);
        let xr = x.reshape([1, 3]).unwrap();
        assert_eq!(gemv(&a, &xr, None, Exec::Auto).unwrap().dims(), &[1, 2]);
    }

    #[test]
    fn gemv_bit_exact_vs_naive_small_shapes() {
        for &(m, k) in &[(1, 1), (1, 7), (7, 1), (3, 5), (16, 16), (17, 33), (65, 9), (130, 31)] {
            let (a, x, b) = (data(m * k, 7), data(k, 5), data(m, 3));
            let r = naive(&a, m, k, &x, Some(&b));
            for exec in [Exec::Scalar, Exec::Parallel, Exec::Auto] {
                let mut y = vec![f32::NAN; m];
                gemv_f32_into_with(SimdLevel::Scalar, &a, m, k, &x, Some(&b), &mut y, exec).unwrap();
                assert_eq!(bits(&y), bits(&r), "m={m} k={k} {exec:?} scalar");
                for level in [SimdLevel::Avx2, SimdLevel::Avx512, SimdLevel::Neon] {
                    let mut y = vec![f32::NAN; m];
                    gemv_f32_into_with(level, &a, m, k, &x, Some(&b), &mut y, exec).unwrap();
                    for i in 0..m {
                        let row = &a[i * k..(i + 1) * k];
                        let tol = dot_tolerance(row, &x) + 2.0 * f32::EPSILON * (r[i].abs() + b[i].abs());
                        assert!((y[i] - r[i]).abs() <= tol, "m={m} k={k} {exec:?} {level:?} row {i}: {} vs {}", y[i], r[i]);
                    }
                }
            }
            let mut y = vec![0f32; m];
            gemv_f32_into_with(SimdLevel::Scalar, &a, m, k, &x, None, &mut y, Exec::Scalar).unwrap();
            assert_eq!(bits(&y), bits(&naive(&a, m, k, &x, None)));
            gemv_f32_into(&a, m, k, &x, None, &mut y, Exec::Scalar).unwrap(); // selected level
            assert_close(&y, &naive(&a, m, k, &x, None), 1e-5);
        }
    }

    #[test]
    fn gemv_strided_view_and_matmul_agree() {
        let (m, k) = (23, 41);
        let at = Tensor::from_vec(data(m * k, 11), [k, m]).unwrap();
        let a = at.transpose(0, 1).unwrap(); // [m, k] non-contiguous
        let x = Tensor::from_vec(data(k, 13), [k]).unwrap();
        let y = gemv(&a, &x, None, Exec::Parallel).unwrap().to_vec_f32();
        let r = naive(&a.to_vec_f32(), m, k, &x.to_vec_f32(), None);
        if features::selected() == SimdLevel::Scalar {
            assert_eq!(bits(&y), bits(&r));
        } else {
            assert_close(&y, &r, 1e-5); // SIMD dot: tolerance vs scalar oracle
        }
        // Cross-check vs matmul (different summation order => tolerance): x[1,k] @ A^T[k,m].
        let mm = matmul(&x.reshape([1, k]).unwrap(), &at, Exec::Scalar).unwrap().to_vec_f32();
        assert_close(&y, &mm, 1e-5);
    }

    #[test]
    fn gemv_shape_errors() {
        let a = Tensor::zeros([4, 3]);
        assert!(gemv(&a, &Tensor::zeros([4]), None, Exec::Scalar).is_err());
        assert!(gemv(&a, &Tensor::zeros([2, 3]), None, Exec::Scalar).is_err());
        assert!(gemv(&a, &Tensor::zeros([3]), Some(&Tensor::zeros([3])), Exec::Scalar).is_err());
        assert!(gemv(&Tensor::zeros([4]), &Tensor::zeros([4]), None, Exec::Scalar).is_err());
        let mut y = vec![0f32; 3];
        assert!(gemv_f32_into(&[0.0; 12], 4, 3, &[0.0; 3], None, &mut y, Exec::Scalar).is_err());
        let mut y0: Vec<f32> = vec![];
        gemv_f32_into(&[], 0, 5, &[0.0; 5], None, &mut y0, Exec::Auto).unwrap();
        let mut y1 = vec![1f32; 2];
        gemv_f32_into(&[], 2, 0, &[], Some(&[3.0, 4.0]), &mut y1, Exec::Auto).unwrap();
        assert_eq!(y1, vec![3.0, 4.0]);
    }

    #[test]
    fn gemv_large_shape_smoke() {
        // Decoder-ish projection: 2048 x 1024 (above Auto parallel threshold).
        let (m, k) = (2048, 1024);
        let (a, x, b) = (data(m * k, 17), data(k, 19), data(m, 23));
        let mut ys = vec![0f32; m];
        let mut yp = vec![0f32; m];
        let mut ya = vec![0f32; m];
        gemv_f32_into(&a, m, k, &x, Some(&b), &mut ys, Exec::Scalar).unwrap();
        gemv_f32_into(&a, m, k, &x, Some(&b), &mut yp, Exec::Parallel).unwrap();
        gemv_f32_into(&a, m, k, &x, Some(&b), &mut ya, Exec::Auto).unwrap();
        assert_eq!(bits(&ys), bits(&yp));
        assert_eq!(bits(&ys), bits(&ya));
        // f64 reference for absolute accuracy.
        for i in (0..m).step_by(97) {
            let r: f64 = (0..k).map(|p| a[i * k + p] as f64 * x[p] as f64).sum::<f64>() + b[i] as f64;
            assert!((ys[i] as f64 - r).abs() <= 1e-3 * (1.0 + r.abs()), "row {i}: {} vs {r}", ys[i]);
        }
        assert!(ys.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn simd_levels_exec_modes_bit_exact_per_level_and_close_to_scalar() {
        let (m, k) = (300, 777);
        let (a, x, b) = (data(m * k, 31), data(k, 37), data(m, 41));
        let mut rs = vec![0f32; m];
        gemv_f32_into_with(SimdLevel::Scalar, &a, m, k, &x, Some(&b), &mut rs, Exec::Scalar).unwrap();
        for level in [SimdLevel::Avx2, SimdLevel::Avx512, SimdLevel::Neon, features::selected()] {
            let mut ys = vec![0f32; m];
            let mut yp = vec![0f32; m];
            gemv_f32_into_with(level, &a, m, k, &x, Some(&b), &mut ys, Exec::Scalar).unwrap();
            gemv_f32_into_with(level, &a, m, k, &x, Some(&b), &mut yp, Exec::Parallel).unwrap();
            assert_eq!(bits(&ys), bits(&yp), "{level:?} exec split must be bit-exact");
            for i in 0..m {
                let tol = dot_tolerance(&a[i * k..(i + 1) * k], &x) + 2.0 * f32::EPSILON * (rs[i].abs() + b[i].abs());
                assert!((ys[i] - rs[i]).abs() <= tol, "{level:?} row {i}");
            }
        }
    }

    #[test]
    fn gemv_t_bit_exact_vs_matmul_all_levels() {
        for &(k, n) in &[(1, 1), (5, 3), (16, 16), (33, 17), (64, 300), (129, 515), (700, 1100)] {
            let (x, bm) = (data(k, 7), data(k * n, 3));
            let mut x = x;
            if k > 2 { x[1] = 0.0; } // exercise the zero-skip path
            let mm = matmul(&Tensor::from_vec(x.clone(), [1, k]).unwrap(), &Tensor::from_vec(bm.clone(), [k, n]).unwrap(), Exec::Scalar)
                .unwrap()
                .to_vec_f32();
            // Reference: the original scalar i-k-j kernel.
            let mut r = vec![0f32; n];
            for p in 0..k { if x[p] == 0.0 { continue; } for j in 0..n { r[j] += x[p] * bm[p * n + j]; } }
            assert_eq!(bits(&mm), bits(&r), "matmul rows==1 route k={k} n={n}");
            for level in [SimdLevel::Scalar, SimdLevel::Avx2, SimdLevel::Avx512, SimdLevel::Neon] {
                for exec in [Exec::Scalar, Exec::Parallel, Exec::Auto] {
                    let mut y = vec![f32::NAN; n];
                    gemv_t_f32_into(level, &x, &bm, k, n, None, &mut y, exec).unwrap();
                    assert_eq!(bits(&y), bits(&r), "k={k} n={n} {level:?} {exec:?}");
                }
            }
        }
        let mut y = vec![0f32; 2];
        assert!(gemv_t_f32_into(SimdLevel::Scalar, &[1.0], &[1.0, 2.0, 3.0], 1, 2, None, &mut y, Exec::Scalar).is_err());
        gemv_t_f32_into(SimdLevel::Scalar, &[2.0], &[1.0, 2.0], 1, 2, Some(&[0.5, 0.5]), &mut y, Exec::Scalar).unwrap();
        assert_eq!(y, vec![2.5, 4.5]);
    }
}
