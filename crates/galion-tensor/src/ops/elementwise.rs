// Galion Voice — Perpetual Open MIT License
use crate::{threading::for_each_row, Exec, Result, Tensor};

fn binary(a: &Tensor, b: &Tensor, f: impl Fn(f32, f32) -> f32) -> Result<Tensor> {
    let out_shape = a.shape().broadcast(b.shape())?;
    let r = out_shape.rank();
    let (av, bv) = (a.to_vec_f32(), b.to_vec_f32());
    if a.shape() == b.shape() {
        return Tensor::from_vec(av.iter().zip(&bv).map(|(&x, &y)| f(x, y)).collect(), out_shape);
    }
    // broadcast strides (0 on broadcast dims)
    let bstr = |t: &Tensor| -> Vec<usize> {
        let cs = t.shape().contiguous_strides();
        let pad = r - t.shape().rank();
        (0..r).map(|i| if i < pad || t.dims()[i - pad] == 1 { 0 } else { cs[i - pad] }).collect()
    };
    let (sa, sb) = (bstr(a), bstr(b));
    let n = out_shape.numel();
    let mut out = Vec::with_capacity(n);
    let mut idx = vec![0usize; r];
    for _ in 0..n {
        let (mut oa, mut ob) = (0, 0);
        for d in 0..r { oa += idx[d] * sa[d]; ob += idx[d] * sb[d]; }
        out.push(f(av[oa], bv[ob]));
        for d in (0..r).rev() {
            idx[d] += 1;
            if idx[d] < out_shape.dims()[d] { break; }
            idx[d] = 0;
        }
    }
    Tensor::from_vec(out, out_shape)
}

/// Broadcasting add.
pub fn add(a: &Tensor, b: &Tensor) -> Result<Tensor> { binary(a, b, |x, y| x + y) }
/// Broadcasting elementwise multiply.
pub fn mul(a: &Tensor, b: &Tensor) -> Result<Tensor> { binary(a, b, |x, y| x * y) }

fn unary(x: &Tensor, exec: Exec, f: impl Fn(f32) -> f32 + Sync + Send) -> Result<Tensor> {
    let mut v = x.to_vec_f32();
    let chunk = 4096.min(v.len().max(1));
    for_each_row(&mut v, chunk, exec, |_, r| r.iter_mut().for_each(|e| *e = f(*e)));
    Tensor::from_vec(v, x.shape().clone())
}

/// GELU, tanh approximation (as used by most speech/LM stacks).
pub fn gelu(x: &Tensor, exec: Exec) -> Result<Tensor> {
    const C: f32 = 0.797_884_6; // sqrt(2/pi)
    unary(x, exec, |v| 0.5 * v * (1.0 + (C * (v + 0.044715 * v * v * v)).tanh()))
}

/// SiLU / swish: x * sigmoid(x).
pub fn silu(x: &Tensor, exec: Exec) -> Result<Tensor> { unary(x, exec, |v| v / (1.0 + (-v).exp())) }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::assert_close;
    #[test]
    fn add_mul_broadcast() {
        let a = Tensor::from_vec(vec![1., 2., 3., 4., 5., 6.], [2, 3]).unwrap();
        let b = Tensor::from_vec(vec![10., 20., 30.], [3]).unwrap();
        assert_eq!(add(&a, &b).unwrap().to_vec_f32(), vec![11., 22., 33., 14., 25., 36.]);
        let c = Tensor::from_vec(vec![2., 3.], [2, 1]).unwrap();
        assert_eq!(mul(&a, &c).unwrap().to_vec_f32(), vec![2., 4., 6., 12., 15., 18.]);
        assert_eq!(add(&a, &Tensor::scalar(1.0)).unwrap().to_vec_f32(), vec![2., 3., 4., 5., 6., 7.]);
        assert!(add(&a, &Tensor::zeros([4])).is_err());
    }
    #[test]
    fn activations() {
        let x = Tensor::from_vec(vec![-3., -1., 0., 1., 3.], [5]).unwrap();
        // reference values (tanh-approx GELU, SiLU)
        assert_close(&gelu(&x, Exec::Scalar).unwrap().to_vec_f32(), &[-0.003_637_3, -0.158_808, 0.0, 0.841_192, 2.996_363], 1e-4);
        assert_close(&silu(&x, Exec::Scalar).unwrap().to_vec_f32(), &[-0.142_278, -0.268_941, 0.0, 0.731_059, 2.857_722], 1e-4);
        assert_eq!(gelu(&x, Exec::Parallel).unwrap().to_vec_f32(), gelu(&x, Exec::Scalar).unwrap().to_vec_f32());
    }
}
