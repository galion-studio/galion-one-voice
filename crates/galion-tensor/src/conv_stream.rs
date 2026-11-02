// Galion Voice — Perpetual Open MIT License
// SPDX-License-Identifier: MIT
//! Stateful causal conv1d for frame-by-frame streaming (codec encoders/decoders, 80 ms frames).
//!
//! * Weights `[c_out, c_in/groups, k]`, optional bias `[c_out]`, any dilation/groups, stride 1.
//! * Causal: output `t` sees inputs `t - (k-1)*dilation ..= t` (left padding only).
//! * All buffers are allocated once in [`CausalConv1dStream::new`]; [`CausalConv1dStream::step`]
//!   performs **zero heap allocations** (verified by `tests/zero_alloc.rs`).
//! * Bit-exact vs. the scalar reference: streaming any chunking of a signal yields exactly the same
//!   f32 bits as [`crate::ops::conv1d`] with `padding = (k-1)*dilation` on the whole signal
//!   (first `len` outputs). Same accumulation order (bias, then `ic`, then `kk`), and positions
//!   before stream start are *skipped* (not multiplied by zero) exactly like the reference.

use crate::{Result, Tensor, TensorError};

#[derive(Debug, Clone)]
pub struct CausalConv1dStream {
    c_in: usize,
    c_out: usize,
    cpg: usize,
    opg: usize,
    k: usize,
    dilation: usize,
    /// Receptive history length = (k-1)*dilation.
    hist: usize,
    max_frame: usize,
    w: Vec<f32>,
    bias: Option<Vec<f32>>,
    /// `[c_in, hist + max_frame]`: history samples followed by the current frame.
    buf: Vec<f32>,
    /// Number of valid history samples (saturates at `hist`); emulates zero-padding-by-skip.
    filled: usize,
}

impl CausalConv1dStream {
    /// `max_frame`: largest frame (samples per channel) `step` will ever be called with.
    pub fn new(w: &Tensor, bias: Option<&Tensor>, dilation: usize, groups: usize, max_frame: usize) -> Result<Self> {
        if w.shape().rank() != 3 || groups == 0 || dilation == 0 || max_frame == 0 {
            return Err(TensorError::InvalidShape("conv_stream: w [c_out,c_in/g,k], groups/dilation/max_frame > 0".into()));
        }
        let (c_out, cpg, k) = (w.dims()[0], w.dims()[1], w.dims()[2]);
        if k == 0 || cpg == 0 || c_out % groups != 0 {
            return Err(TensorError::InvalidShape(format!("conv_stream: bad weight dims {:?} for groups {groups}", w.dims())));
        }
        let bias = match bias {
            Some(b) if b.dims() != [c_out] => {
                return Err(TensorError::ShapeMismatch { op: "conv_stream bias", lhs: vec![c_out], rhs: b.dims().to_vec() })
            }
            Some(b) => Some(b.to_vec_f32()),
            None => None,
        };
        let c_in = cpg * groups;
        let hist = (k - 1) * dilation;
        Ok(Self {
            c_in, c_out, cpg, opg: c_out / groups, k, dilation, hist, max_frame,
            w: w.to_vec_f32(), bias,
            buf: vec![0.0; c_in * (hist + max_frame)],
            filled: 0,
        })
    }

    pub fn c_in(&self) -> usize { self.c_in }
    pub fn c_out(&self) -> usize { self.c_out }
    pub fn history_len(&self) -> usize { self.hist }
    pub fn max_frame(&self) -> usize { self.max_frame }

    /// Forget all history (start of a new utterance). No allocation.
    pub fn reset(&mut self) {
        self.buf.iter_mut().for_each(|e| *e = 0.0);
        self.filled = 0;
    }

    /// Process one frame. `x`: `[c_in, n]` row-major, `out`: `[c_out, n]` row-major, `n <= max_frame`.
    /// Zero heap allocation.
    pub fn step(&mut self, x: &[f32], n: usize, out: &mut [f32]) -> Result<()> {
        if n > self.max_frame || x.len() != self.c_in * n || out.len() != self.c_out * n {
            return Err(TensorError::InvalidShape("conv_stream step: x [c_in,n], out [c_out,n], n <= max_frame".into()));
        }
        if n == 0 { return Ok(()); }
        let (hist, stride) = (self.hist, self.hist + self.max_frame);
        for c in 0..self.c_in {
            self.buf[c * stride + hist..c * stride + hist + n].copy_from_slice(&x[c * n..(c + 1) * n]);
        }
        // Buffer index `hist - filled` is the first valid sample; earlier ones are "before t=0".
        let first_valid = hist - self.filled;
        for oc in 0..self.c_out {
            let g = oc / self.opg;
            for t in 0..n {
                let mut acc = self.bias.as_ref().map_or(0.0, |b| b[oc]);
                for ic in 0..self.cpg {
                    let row = &self.buf[(g * self.cpg + ic) * stride..];
                    let wr = &self.w[(oc * self.cpg + ic) * self.k..];
                    for kk in 0..self.k {
                        let pos = t + kk * self.dilation; // buffer index; current sample at t+hist
                        if pos >= first_valid {
                            acc += row[pos] * wr[kk];
                        }
                    }
                }
                out[oc * n + t] = acc;
            }
        }
        // Carry the last `hist` samples (history ++ frame) to the front.
        if hist > 0 {
            for c in 0..self.c_in {
                let base = c * stride;
                self.buf.copy_within(base + n..base + n + hist, base);
            }
        }
        self.filled = (self.filled + n).min(hist);
        Ok(())
    }

    /// Tensor convenience wrapper (allocates the output; use [`Self::step`] on the hot path).
    pub fn step_tensor(&mut self, x: &Tensor) -> Result<Tensor> {
        if x.shape().rank() != 2 || x.dims()[0] != self.c_in {
            return Err(TensorError::InvalidShape(format!("conv_stream: x must be [{}, n], got {:?}", self.c_in, x.dims())));
        }
        let n = x.dims()[1];
        let mut out = vec![0.0; self.c_out * n];
        self.step(&x.to_vec_f32(), n, &mut out)?;
        Tensor::from_vec(out, [self.c_out, n])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::{conv1d, Conv1dParams};

    fn lcg(n: usize, seed: u32) -> Vec<f32> {
        let mut s = seed;
        (0..n).map(|_| { s = s.wrapping_mul(1664525).wrapping_add(1013904223); ((s >> 8) as f32 / (1u32 << 24) as f32) * 2.0 - 1.0 }).collect()
    }

    fn reference(x: &[f32], c_in: usize, len: usize, w: &Tensor, b: Option<&Tensor>, dil: usize, groups: usize) -> Vec<f32> {
        let k = w.dims()[2];
        let pad = (k - 1) * dil;
        let xt = Tensor::from_vec(x.to_vec(), [c_in, len]).unwrap();
        let y = conv1d(&xt, w, b, Conv1dParams { stride: 1, padding: pad, dilation: dil, groups }).unwrap();
        let ol = y.dims()[1];
        let yv = y.to_vec_f32();
        (0..w.dims()[0]).flat_map(|oc| yv[oc * ol..oc * ol + len].to_vec()).collect()
    }

    fn run(c_in: usize, c_out: usize, k: usize, dil: usize, groups: usize, chunks: &[usize], bias: bool) {
        let len: usize = chunks.iter().sum();
        let w = Tensor::from_vec(lcg(c_out * (c_in / groups) * k, 7), [c_out, c_in / groups, k]).unwrap();
        let b = bias.then(|| Tensor::from_vec(lcg(c_out, 9), [c_out]).unwrap());
        let x = lcg(c_in * len, 3);
        let want = reference(&x, c_in, len, &w, b.as_ref(), dil, groups);
        let maxf = *chunks.iter().max().unwrap();
        let mut s = CausalConv1dStream::new(&w, b.as_ref(), dil, groups, maxf.max(1)).unwrap();
        let mut got = vec![0f32; c_out * len];
        let mut off = 0;
        for &n in chunks {
            let xf: Vec<f32> = (0..c_in).flat_map(|c| x[c * len + off..c * len + off + n].to_vec()).collect();
            let mut yf = vec![0f32; c_out * n];
            s.step(&xf, n, &mut yf).unwrap();
            for oc in 0..c_out { got[oc * len + off..oc * len + off + n].copy_from_slice(&yf[oc * n..(oc + 1) * n]); }
            off += n;
        }
        let gb: Vec<u32> = got.iter().map(|f| f.to_bits()).collect();
        let wb: Vec<u32> = want.iter().map(|f| f.to_bits()).collect();
        assert_eq!(gb, wb, "bit-exact mismatch c_in={c_in} k={k} dil={dil} g={groups} chunks={chunks:?}");
    }

    #[test]
    fn bit_exact_vs_reference_various_chunkings() {
        run(1, 1, 3, 1, 1, &[5], false);
        run(1, 1, 3, 1, 1, &[1, 1, 1, 1, 1], false);
        run(3, 4, 5, 1, 1, &[2, 7, 1, 4, 6], true);
        run(4, 4, 3, 2, 4, &[1, 3, 8, 2], true); // depthwise + dilation
        run(4, 6, 4, 3, 2, &[16, 1, 1, 9], true); // grouped, frame < history
        run(2, 2, 1, 1, 1, &[3, 3], true); // k=1: no history
        run(2, 3, 7, 1, 1, &[1920 / 64; 4], true); // codec-like frames
    }

    #[test]
    fn reset_restarts_stream() {
        let w = Tensor::from_vec(lcg(2 * 2 * 3, 1), [2, 2, 3]).unwrap();
        let mut s = CausalConv1dStream::new(&w, None, 1, 1, 4).unwrap();
        let x = lcg(8, 2);
        let mut a = vec![0f32; 8];
        let mut b = vec![0f32; 8];
        s.step(&x, 4, &mut a).unwrap();
        s.step(&x, 4, &mut b).unwrap();
        assert_ne!(a, b); // history affects 2nd frame
        s.reset();
        s.step(&x, 4, &mut b).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn rejects_oversize_frame_and_bad_shapes() {
        let w = Tensor::from_vec(vec![1.0; 3], [1, 1, 3]).unwrap();
        let mut s = CausalConv1dStream::new(&w, None, 1, 1, 2).unwrap();
        let mut o = vec![0f32; 3];
        assert!(s.step(&[0.0; 3], 3, &mut o).is_err());
        assert!(CausalConv1dStream::new(&w, None, 0, 1, 2).is_err());
        assert!(CausalConv1dStream::new(&w, Some(&Tensor::zeros([2])), 1, 1, 2).is_err());
        assert_eq!(s.step_tensor(&Tensor::from_vec(vec![1., 2.], [1, 2]).unwrap()).unwrap().to_vec_f32(), vec![1., 3.]);
    }
}
