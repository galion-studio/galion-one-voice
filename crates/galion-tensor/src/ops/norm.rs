// Galion Voice — Perpetual Open MIT License
use crate::{threading::for_each_row, Exec, Result, Tensor, TensorError};

/// Numerically-stable softmax over the last dimension.
pub fn softmax(x: &Tensor, exec: Exec) -> Result<Tensor> {
    let d = x.shape().last();
    let mut v = x.to_vec_f32();
    for_each_row(&mut v, d, exec, |_, r| {
        let m = r.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let mut s = 0.0;
        for e in r.iter_mut() { *e = (*e - m).exp(); s += *e; }
        let inv = 1.0 / s;
        r.iter_mut().for_each(|e| *e *= inv);
    });
    Tensor::from_vec(v, x.shape().clone())
}

/// RMSNorm over last dim: y = x / sqrt(mean(x^2) + eps) * weight.
pub fn rms_norm(x: &Tensor, weight: &Tensor, eps: f32, exec: Exec) -> Result<Tensor> {
    let d = x.shape().last();
    if weight.dims() != [d] {
        return Err(TensorError::ShapeMismatch { op: "rms_norm", lhs: x.dims().to_vec(), rhs: weight.dims().to_vec() });
    }
    let w = weight.to_vec_f32();
    let mut v = x.to_vec_f32();
    for_each_row(&mut v, d, exec, |_, r| {
        let ms = r.iter().map(|e| e * e).sum::<f32>() / d as f32;
        let inv = 1.0 / (ms + eps).sqrt();
        r.iter_mut().zip(&w).for_each(|(e, wi)| *e = *e * inv * wi);
    });
    Tensor::from_vec(v, x.shape().clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::assert_close;
    #[test]
    fn softmax_rows() {
        let x = Tensor::from_vec(vec![1., 2., 3., 1000., 1000., 1000.], [2, 3]).unwrap();
        let y = softmax(&x, Exec::Scalar).unwrap().to_vec_f32();
        assert_close(&y[..3], &[0.090_030_57, 0.244_728_47, 0.665_240_96], 1e-5);
        assert_close(&y[3..], &[1. / 3., 1. / 3., 1. / 3.], 1e-6); // stability
    }
    #[test]
    fn rmsnorm() {
        let x = Tensor::from_vec(vec![1., 2., 3., 4.], [1, 4]).unwrap();
        let w = Tensor::from_vec(vec![1., 1., 2., 0.5], [4]).unwrap();
        let y = rms_norm(&x, &w, 0.0, Exec::Scalar).unwrap().to_vec_f32();
        let r = (7.5f32).sqrt();
        assert_close(&y, &[1. / r, 2. / r, 6. / r, 2. / r], 1e-6);
        assert!(rms_norm(&x, &Tensor::zeros([3]), 1e-6, Exec::Scalar).is_err());
    }
}
