// Galion Voice — Perpetual Open MIT License
use crate::gemv::{gemv_f32_into_with, gemv_t_f32_into};
use crate::simd::{axpy_for, AxpyFn};
use crate::{features, threading::for_each_row, Exec, Result, Shape, Storage, Tensor, TensorError};

/// `a[..., m, k] @ b[k, n] -> [..., m, n]` (linear-layer form; leading dims of `a` are flattened).
/// Scalar path uses i-k-j loop order (cache-friendly on row-major `b`); the inner axpy uses the
/// selected SIMD level, which is bit-exact vs scalar (non-fused mul+add per lane).
///
/// `rows == 1` (batch-1 decode) routes to GEMV:
/// * `b` is a transposed view of a contiguous `[n, k]` f32 weight (`W^T`, strides `[1, k]`) →
///   row-major [`crate::gemv`] over `W` directly (zero-copy; SIMD dot ⇒ tolerance vs scalar).
/// * otherwise → [`gemv_t_f32_into`] (axpy sweep), bit-exact vs the general path.
pub fn matmul(a: &Tensor, b: &Tensor, exec: Exec) -> Result<Tensor> {
    if a.shape().rank() < 1 || b.shape().rank() != 2 {
        return Err(TensorError::InvalidShape("matmul: need a rank>=1, b rank 2".into()));
    }
    let k = a.shape().last();
    let (kb, n) = (b.dims()[0], b.dims()[1]);
    if k != kb {
        return Err(TensorError::ShapeMismatch { op: "matmul", lhs: a.dims().to_vec(), rhs: b.dims().to_vec() });
    }
    let rows = a.numel() / k.max(1);
    let mut dims = a.dims().to_vec();
    *dims.last_mut().unwrap() = n;
    if rows == 1 && k > 0 && n > 0 {
        let level = features::selected();
        let av = a.contiguous();
        let x = av.as_f32_slice().unwrap();
        let mut out = vec![0f32; n];
        if let Storage::F32(v) = b.storage() {
            let off = b.offset();
            if b.strides() == [1, k] && off + n * k <= v.len() {
                gemv_f32_into_with(level, &v[off..off + n * k], n, k, x, None, &mut out, exec)?;
                return Tensor::from_vec(out, Shape(dims));
            }
        }
        let bv = b.contiguous();
        gemv_t_f32_into(level, x, bv.as_f32_slice().unwrap(), k, n, None, &mut out, exec)?;
        return Tensor::from_vec(out, Shape(dims));
    }
    let av = a.contiguous();
    let bv = b.contiguous();
    let (ad, bd) = (av.as_f32_slice().unwrap(), bv.as_f32_slice().unwrap());
    let mut out = vec![0f32; rows * n];
    // Auto: decide on FLOPs, not output size.
    let e = match exec {
        Exec::Auto if rows * n * k >= crate::threading::PAR_THRESHOLD * 64 => Exec::Parallel,
        Exec::Auto => Exec::Scalar,
        other => other,
    };
    let axpy = axpy_for(features::selected());
    for_each_row(&mut out, n, e, |i, orow| row_kernel(axpy, &ad[i * k..(i + 1) * k], bd, n, orow));
    Tensor::from_vec(out, Shape(dims))
}

#[inline]
fn row_kernel(axpy: AxpyFn, arow: &[f32], b: &[f32], n: usize, out: &mut [f32]) {
    for (p, &av) in arow.iter().enumerate() {
        if av == 0.0 { continue; }
        axpy(av, &b[p * n..(p + 1) * n], out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::assert_close;
    #[test]
    fn matmul_small() {
        let a = Tensor::from_vec(vec![1., 2., 3., 4., 5., 6.], [2, 3]).unwrap();
        let b = Tensor::from_vec(vec![7., 8., 9., 10., 11., 12.], [3, 2]).unwrap();
        let c = matmul(&a, &b, Exec::Scalar).unwrap();
        assert_eq!(c.dims(), &[2, 2]);
        assert_eq!(c.to_vec_f32(), vec![58., 64., 139., 154.]);
        assert!(matmul(&a, &a, Exec::Scalar).is_err());
    }
    #[test]
    fn matmul_transposed_view_and_parallel_match() {
        let (m, k, n) = (37, 64, 29);
        let a = Tensor::from_vec((0..m * k).map(|i| ((i * 7) % 13) as f32 - 6.0).collect(), [m, k]).unwrap();
        let bt = Tensor::from_vec((0..n * k).map(|i| ((i * 5) % 11) as f32 * 0.1).collect(), [n, k]).unwrap();
        let b = bt.transpose(0, 1).unwrap(); // strided view
        let s = matmul(&a, &b, Exec::Scalar).unwrap();
        let p = matmul(&a, &b, Exec::Parallel).unwrap();
        assert_eq!(s.to_vec_f32(), p.to_vec_f32()); // same per-row order => bit-exact
        // naive reference
        let (av, bv) = (a.to_vec_f32(), b.to_vec_f32());
        let mut r = vec![0f32; m * n];
        for i in 0..m { for j in 0..n { for p in 0..k { r[i * n + j] += av[i * k + p] * bv[p * n + j]; } } }
        assert_close(&s.to_vec_f32(), &r, 1e-5);
    }
    #[test]
    fn matmul_batched_lhs() {
        let a = Tensor::full([2, 3, 4], 1.0);
        let b = Tensor::full([4, 5], 0.5);
        let c = matmul(&a, &b, Exec::Auto).unwrap();
        assert_eq!(c.dims(), &[2, 3, 5]);
        assert!(c.to_vec_f32().iter().all(|&x| x == 2.0));
    }
    #[test]
    fn matmul_rows1_routes_to_gemv() {
        let (k, n) = (77, 45);
        let x = Tensor::from_vec((0..k).map(|i| ((i * 5) % 9) as f32 * 0.3 - 1.2).collect(), [1, k]).unwrap();
        let w = Tensor::from_vec((0..n * k).map(|i| ((i * 7) % 23) as f32 * 0.05 - 0.5).collect(), [n, k]).unwrap();
        let wt = w.transpose(0, 1).unwrap(); // [k, n] view of row-major W -> zero-copy GEMV
        let y = matmul(&x, &wt, Exec::Auto).unwrap();
        assert_eq!(y.dims(), &[1, n]);
        let g = crate::gemv::gemv(&w, &x, None, Exec::Scalar).unwrap().to_vec_f32();
        assert_eq!(y.to_vec_f32(), g); // same kernel, same level
        // General (rows=2) path vs rows==1 route: tolerance (dot vs axpy order / fusion).
        let x2 = Tensor::from_vec([x.to_vec_f32(), x.to_vec_f32()].concat(), [2, k]).unwrap();
        let y2 = matmul(&x2, &wt, Exec::Scalar).unwrap().to_vec_f32();
        assert_close(&y.to_vec_f32(), &y2[..n], 1e-5);
        assert_close(&y.to_vec_f32(), &y2[n..], 1e-5);
        // Contiguous [k, n] b -> gemv_t, bit-exact vs general path.
        let bc = wt.contiguous();
        let y3 = matmul(&x, &bc, Exec::Parallel).unwrap().to_vec_f32();
        assert_eq!(y3, matmul(&x2, &bc, Exec::Scalar).unwrap().to_vec_f32()[..n].to_vec());
        // Rank-1 lhs + 3-D [1,1,k] lhs keep their shapes.
        assert_eq!(matmul(&x.reshape([k]).unwrap(), &bc, Exec::Auto).unwrap().dims(), &[n]);
        assert_eq!(matmul(&x.reshape([1, 1, k]).unwrap(), &wt, Exec::Auto).unwrap().dims(), &[1, 1, n]);
    }
}
