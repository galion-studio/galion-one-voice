// Galion Voice — Perpetual Open MIT License
use crate::{Result, Tensor, TensorError};

/// Rotary position embedding, NeoX "rotate-half" layout.
/// `x`: `[seq, heads, head_dim]` (head_dim even). Position of row `t` is `pos0 + t`.
/// Minimal scalar implementation; cos/sin cache + fused QK path is a TODO.
pub fn rope_neox(x: &Tensor, pos0: usize, theta: f32) -> Result<Tensor> {
    if x.shape().rank() != 3 || x.dims()[2] % 2 != 0 {
        return Err(TensorError::InvalidShape(format!("rope: need [seq, heads, even head_dim], got {:?}", x.dims())));
    }
    let (seq, heads, hd) = (x.dims()[0], x.dims()[1], x.dims()[2]);
    let half = hd / 2;
    let mut v = x.to_vec_f32();
    for t in 0..seq {
        let pos = (pos0 + t) as f32;
        for i in 0..half {
            let freq = theta.powf(-2.0 * i as f32 / hd as f32);
            let (s, c) = (pos * freq).sin_cos();
            for h in 0..heads {
                let base = (t * heads + h) * hd;
                let (a, b) = (v[base + i], v[base + i + half]);
                v[base + i] = a * c - b * s;
                v[base + i + half] = a * s + b * c;
            }
        }
    }
    Tensor::from_vec(v, x.shape().clone())
}

/// Interleaved (GPT-J style) RoPE — stub. Will be needed only if a borrowed checkpoint uses it.
pub fn rope_interleaved(_x: &Tensor, _pos0: usize, _theta: f32) -> Result<Tensor> {
    Err(TensorError::Unimplemented("rope_interleaved"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::assert_close;
    #[test]
    fn rope_pos0_identity_and_norm_preserving() {
        let x = Tensor::from_vec((0..2 * 2 * 4).map(|i| i as f32 * 0.25 - 1.0).collect(), [2, 2, 4]).unwrap();
        let y = rope_neox(&x, 0, 10_000.0).unwrap().to_vec_f32();
        let xv = x.to_vec_f32();
        assert_close(&y[..8], &xv[..8], 1e-6); // t=0 => identity
        let n = |s: &[f32]| s.iter().map(|e| e * e).sum::<f32>();
        assert!((n(&y) - n(&xv)).abs() < 1e-3); // rotation preserves norm
        // pair (i=0) at t=1 rotated by exactly 1 rad
        let (a, b) = (xv[8], xv[10]);
        assert_close(&[y[8], y[10]], &[a * 1f32.cos() - b * 1f32.sin(), a * 1f32.sin() + b * 1f32.cos()], 1e-5);
        assert!(rope_interleaved(&x, 0, 1e4).is_err());
    }
}
