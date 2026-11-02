// Galion Voice — Perpetual Open MIT License
// SPDX-License-Identifier: MIT
//! Streaming resampling convolutions for the Mimi codec path (A5 follow-up).
//!
//! * [`StridedConv1dStream`]: causal conv1d with **stride ≥ 1** (encoder downsampling). Left padding
//!   `pad` (default Mimi value `(k-1)*dilation + 1 - stride`), zero/"constant" padding that is
//!   *skipped*, not multiplied. Emits every output whose receptive window is complete; partial
//!   windows wait for the next frame. Feeding any chunking of a signal gives exactly the same f32
//!   bits as [`crate::ops::conv1d`] with `padding = pad` on the whole signal (the outputs that do
//!   not touch right padding).
//! * [`ConvTranspose1dStream`]: causal transposed conv1d (decoder upsampling). Each input frame of
//!   `n` samples yields exactly `n*stride` outputs — Mimi's convention of trimming all
//!   `k - stride` padding from the right. Bit-exact vs. the first `len*stride` outputs of
//!   [`crate::ops::conv_transpose1d`] (`padding = 0`) on the whole signal, for any chunking.
//!
//! Both allocate everything in `new`; `step` performs **zero heap allocations**
//! (`tests/zero_alloc.rs`). Accumulation order everywhere: bias, then `ic`, then `kk`.
//!
//! Signed: Maciej Grajczyk.

use crate::{Result, Tensor, TensorError};

fn take_bias(bias: Option<&Tensor>, c_out: usize, op: &'static str) -> Result<Option<Vec<f32>>> {
    match bias {
        Some(b) if b.dims() != [c_out] => Err(TensorError::ShapeMismatch { op, lhs: vec![c_out], rhs: b.dims().to_vec() }),
        Some(b) => Ok(Some(b.to_vec_f32())),
        None => Ok(None),
    }
}

/// Causal conv1d with arbitrary stride, frame-by-frame. Weights `[c_out, c_in/groups, k]`.
#[derive(Debug, Clone)]
pub struct StridedConv1dStream {
    c_in: usize,
    c_out: usize,
    cpg: usize,
    opg: usize,
    k: usize,
    dilation: usize,
    stride: usize,
    pad: usize,
    /// Effective kernel span `(k-1)*dilation + 1`.
    span: usize,
    max_frame: usize,
    /// Per-channel row length `span - 1 + max_frame`.
    cap: usize,
    w: Vec<f32>,
    bias: Option<Vec<f32>>,
    /// `[c_in, cap]`: real samples at logical positions `base .. base + buf_len`.
    buf: Vec<f32>,
    buf_len: usize,
    base: usize,
    /// Samples received since start/reset.
    total: usize,
    /// Index of the next output to emit.
    t_next: usize,
}

impl StridedConv1dStream {
    /// Mimi-style causal conv: left padding `(k-1)*dilation + 1 - stride` (saturating at 0).
    pub fn new(w: &Tensor, bias: Option<&Tensor>, stride: usize, dilation: usize, groups: usize, max_frame: usize) -> Result<Self> {
        if w.shape().rank() != 3 || w.dims()[2] == 0 || dilation == 0 {
            return Err(TensorError::InvalidShape("strided_conv_stream: w [c_out,c_in/g,k], dilation > 0".into()));
        }
        let pad = ((w.dims()[2] - 1) * dilation + 1).saturating_sub(stride);
        Self::with_padding(w, bias, stride, dilation, groups, pad, max_frame)
    }

    /// Explicit left padding `pad <= (k-1)*dilation` (beyond that, outputs would precede all input).
    pub fn with_padding(w: &Tensor, bias: Option<&Tensor>, stride: usize, dilation: usize, groups: usize, pad: usize, max_frame: usize) -> Result<Self> {
        if w.shape().rank() != 3 || groups == 0 || dilation == 0 || stride == 0 || max_frame == 0 {
            return Err(TensorError::InvalidShape("strided_conv_stream: w [c_out,c_in/g,k], groups/stride/dilation/max_frame > 0".into()));
        }
        let (c_out, cpg, k) = (w.dims()[0], w.dims()[1], w.dims()[2]);
        if k == 0 || cpg == 0 || c_out % groups != 0 {
            return Err(TensorError::InvalidShape(format!("strided_conv_stream: bad weight dims {:?} for groups {groups}", w.dims())));
        }
        let span = (k - 1) * dilation + 1;
        if pad > span - 1 {
            return Err(TensorError::InvalidShape(format!("strided_conv_stream: pad {pad} > (k-1)*dilation {}", span - 1)));
        }
        let bias = take_bias(bias, c_out, "strided_conv_stream bias")?;
        let c_in = cpg * groups;
        let cap = span - 1 + max_frame;
        Ok(Self {
            c_in, c_out, cpg, opg: c_out / groups, k, dilation, stride, pad, span, max_frame, cap,
            w: w.to_vec_f32(), bias,
            buf: vec![0.0; c_in * cap], buf_len: 0, base: 0, total: 0, t_next: 0,
        })
    }

    pub fn c_in(&self) -> usize { self.c_in }
    pub fn c_out(&self) -> usize { self.c_out }
    pub fn stride(&self) -> usize { self.stride }
    pub fn left_padding(&self) -> usize { self.pad }
    pub fn max_frame(&self) -> usize { self.max_frame }
    /// Upper bound on outputs per channel from any single `step` (size `out` with `c_out *` this).
    pub fn max_outputs_per_step(&self) -> usize { (self.max_frame + self.pad) / self.stride + 1 }

    /// Number of outputs per channel the next `step` with `n` samples will emit.
    pub fn outputs_for(&self, n: usize) -> usize {
        let avail = self.total + n + self.pad;
        if avail < self.span { return 0; }
        ((avail - self.span) / self.stride + 1).saturating_sub(self.t_next)
    }

    /// Forget all history. No allocation.
    pub fn reset(&mut self) {
        self.buf.iter_mut().for_each(|e| *e = 0.0);
        self.buf_len = 0;
        self.base = 0;
        self.total = 0;
        self.t_next = 0;
    }

    /// Process one frame. `x`: `[c_in, n]`, `n <= max_frame`. Writes `[c_out, m]` row-major into the
    /// front of `out` (`out.len() >= c_out * m`) and returns `m` (= [`Self::outputs_for`]`(n)`).
    /// Zero heap allocation.
    pub fn step(&mut self, x: &[f32], n: usize, out: &mut [f32]) -> Result<usize> {
        if n > self.max_frame || x.len() != self.c_in * n {
            return Err(TensorError::InvalidShape("strided_conv_stream step: x [c_in,n], n <= max_frame".into()));
        }
        let m = self.outputs_for(n);
        if out.len() < self.c_out * m {
            return Err(TensorError::InvalidShape(format!("strided_conv_stream step: out needs {} floats", self.c_out * m)));
        }
        // Append samples with logical position >= base (earlier ones were skipped by a stride jump).
        let skip = self.base.saturating_sub(self.total).min(n);
        let keep = n - skip;
        let cap = self.cap;
        for c in 0..self.c_in {
            let dst = c * cap + self.buf_len;
            self.buf[dst..dst + keep].copy_from_slice(&x[c * n + skip..(c + 1) * n]);
        }
        self.buf_len += keep;
        self.total += n;

        for j in 0..m {
            let ws = ((self.t_next + j) * self.stride) as isize - self.pad as isize; // window start
            for oc in 0..self.c_out {
                let g = oc / self.opg;
                let mut acc = self.bias.as_ref().map_or(0.0, |b| b[oc]);
                for ic in 0..self.cpg {
                    let row = &self.buf[(g * self.cpg + ic) * cap..];
                    let wr = &self.w[(oc * self.cpg + ic) * self.k..];
                    for kk in 0..self.k {
                        let pos = ws + (kk * self.dilation) as isize;
                        if pos >= 0 {
                            acc += row[pos as usize - self.base] * wr[kk];
                        }
                    }
                }
                out[oc * m + j] = acc;
            }
        }
        self.t_next += m;

        // Drop samples before the next window start.
        let next_ws = (self.t_next * self.stride).saturating_sub(self.pad);
        if next_ws > self.base {
            let drop = (next_ws - self.base).min(self.buf_len);
            if drop > 0 && drop < self.buf_len {
                for c in 0..self.c_in {
                    let b = c * cap;
                    self.buf.copy_within(b + drop..b + self.buf_len, b);
                }
            }
            self.buf_len -= drop;
            self.base = next_ws;
        }
        Ok(m)
    }

    /// Tensor convenience wrapper (allocates; use [`Self::step`] on the hot path).
    pub fn step_tensor(&mut self, x: &Tensor) -> Result<Tensor> {
        if x.shape().rank() != 2 || x.dims()[0] != self.c_in {
            return Err(TensorError::InvalidShape(format!("strided_conv_stream: x must be [{}, n], got {:?}", self.c_in, x.dims())));
        }
        let n = x.dims()[1];
        let mut out = vec![0.0; self.c_out * self.outputs_for(n)];
        let m = self.step(&x.to_vec_f32(), n, &mut out)?;
        Tensor::from_vec(out, [self.c_out, m])
    }
}

/// Causal transposed conv1d (upsampling), frame-by-frame. Weights `[c_in, c_out/groups, k]`
/// (PyTorch `ConvTranspose1d` layout). `n` input samples -> `n * stride` output samples.
#[derive(Debug, Clone)]
pub struct ConvTranspose1dStream {
    c_in: usize,
    c_out: usize,
    cpg: usize,
    opg: usize,
    k: usize,
    dilation: usize,
    stride: usize,
    /// Input history `ceil((k-1)*dilation / stride)`.
    hist: usize,
    max_frame: usize,
    w: Vec<f32>,
    bias: Option<Vec<f32>>,
    /// `[c_in, hist + max_frame]`.
    buf: Vec<f32>,
    filled: usize,
}

impl ConvTranspose1dStream {
    /// `max_frame`: largest input frame `step` will see (output is `max_frame * stride`).
    pub fn new(w: &Tensor, bias: Option<&Tensor>, stride: usize, dilation: usize, groups: usize, max_frame: usize) -> Result<Self> {
        if w.shape().rank() != 3 || groups == 0 || dilation == 0 || stride == 0 || max_frame == 0 {
            return Err(TensorError::InvalidShape("conv_transpose_stream: w [c_in,c_out/g,k], groups/stride/dilation/max_frame > 0".into()));
        }
        let (c_in, opg, k) = (w.dims()[0], w.dims()[1], w.dims()[2]);
        if k == 0 || opg == 0 || c_in % groups != 0 {
            return Err(TensorError::InvalidShape(format!("conv_transpose_stream: bad weight dims {:?} for groups {groups}", w.dims())));
        }
        let c_out = opg * groups;
        let bias = take_bias(bias, c_out, "conv_transpose_stream bias")?;
        let hist = ((k - 1) * dilation).div_ceil(stride);
        Ok(Self {
            c_in, c_out, cpg: c_in / groups, opg, k, dilation, stride, hist, max_frame,
            w: w.to_vec_f32(), bias,
            buf: vec![0.0; c_in * (hist + max_frame)], filled: 0,
        })
    }

    pub fn c_in(&self) -> usize { self.c_in }
    pub fn c_out(&self) -> usize { self.c_out }
    pub fn stride(&self) -> usize { self.stride }
    pub fn history_len(&self) -> usize { self.hist }
    pub fn max_frame(&self) -> usize { self.max_frame }

    /// Forget all history. No allocation.
    pub fn reset(&mut self) {
        self.buf.iter_mut().for_each(|e| *e = 0.0);
        self.filled = 0;
    }

    /// `x`: `[c_in, n]`, `out`: `[c_out, n*stride]`, `n <= max_frame`. Zero heap allocation.
    pub fn step(&mut self, x: &[f32], n: usize, out: &mut [f32]) -> Result<()> {
        let on = n * self.stride;
        if n > self.max_frame || x.len() != self.c_in * n || out.len() != self.c_out * on {
            return Err(TensorError::InvalidShape("conv_transpose_stream step: x [c_in,n], out [c_out,n*stride], n <= max_frame".into()));
        }
        if n == 0 { return Ok(()); }
        let (hist, row_len, s) = (self.hist, self.hist + self.max_frame, self.stride);
        for c in 0..self.c_in {
            self.buf[c * row_len + hist..c * row_len + hist + n].copy_from_slice(&x[c * n..(c + 1) * n]);
        }
        let first_valid = hist - self.filled; // buffer index of the oldest real input
        for oc in 0..self.c_out {
            let (g, ol) = (oc / self.opg, oc % self.opg);
            for jj in 0..on {
                let mut acc = self.bias.as_ref().map_or(0.0, |b| b[oc]);
                let jh = jj + hist * s; // output index relative to buffer index 0
                for ic in 0..self.cpg {
                    let xc = g * self.cpg + ic;
                    let row = &self.buf[xc * row_len..];
                    let wr = &self.w[(xc * self.opg + ol) * self.k..];
                    for kk in 0..self.k {
                        let kd = kk * self.dilation;
                        if kd <= jh && (jh - kd) % s == 0 {
                            let idx = (jh - kd) / s;
                            if idx >= first_valid {
                                acc += row[idx] * wr[kk];
                            }
                        }
                    }
                }
                out[oc * on + jj] = acc;
            }
        }
        if hist > 0 {
            for c in 0..self.c_in {
                let b = c * row_len;
                self.buf.copy_within(b + n..b + n + hist, b);
            }
        }
        self.filled = (self.filled + n).min(hist);
        Ok(())
    }

    /// Tensor convenience wrapper (allocates; use [`Self::step`] on the hot path).
    pub fn step_tensor(&mut self, x: &Tensor) -> Result<Tensor> {
        if x.shape().rank() != 2 || x.dims()[0] != self.c_in {
            return Err(TensorError::InvalidShape(format!("conv_transpose_stream: x must be [{}, n], got {:?}", self.c_in, x.dims())));
        }
        let n = x.dims()[1];
        let mut out = vec![0.0; self.c_out * n * self.stride];
        self.step(&x.to_vec_f32(), n, &mut out)?;
        Tensor::from_vec(out, [self.c_out, n * self.stride])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::{conv1d, conv_transpose1d, Conv1dParams, ConvTranspose1dParams};
    use crate::CausalConv1dStream;

    fn lcg(n: usize, seed: u32) -> Vec<f32> {
        let mut s = seed;
        (0..n).map(|_| { s = s.wrapping_mul(1664525).wrapping_add(1013904223); ((s >> 8) as f32 / (1u32 << 24) as f32) * 2.0 - 1.0 }).collect()
    }
    fn bits(v: &[f32]) -> Vec<u32> { v.iter().map(|f| f.to_bits()).collect() }
    fn slice_frame(x: &[f32], c: usize, len: usize, off: usize, n: usize) -> Vec<f32> {
        (0..c).flat_map(|ch| x[ch * len + off..ch * len + off + n].to_vec()).collect()
    }
    /// Concatenate per-step `[c, m_i]` outputs into `[c, sum m_i]`.
    fn concat(parts: &[Vec<f32>], c: usize) -> Vec<f32> {
        let mut out = Vec::new();
        for ch in 0..c {
            for p in parts { let m = p.len() / c; out.extend_from_slice(&p[ch * m..(ch + 1) * m]); }
        }
        out
    }

    // ---------- strided conv ----------
    fn strided_ref(x: &[f32], c_in: usize, len: usize, w: &Tensor, b: Option<&Tensor>, s: usize, d: usize, g: usize, pad: usize) -> Vec<f32> {
        let span = (w.dims()[2] - 1) * d + 1;
        if len + pad < span { return vec![]; }
        let valid = (len + pad - span) / s + 1;
        let xt = Tensor::from_vec(x.to_vec(), [c_in, len]).unwrap();
        let y = conv1d(&xt, w, b, Conv1dParams { stride: s, padding: pad, dilation: d, groups: g }).unwrap();
        let ol = y.dims()[1];
        let yv = y.to_vec_f32();
        (0..w.dims()[0]).flat_map(|oc| yv[oc * ol..oc * ol + valid].to_vec()).collect()
    }

    fn run_strided(c_in: usize, c_out: usize, k: usize, s: usize, d: usize, g: usize, pad: Option<usize>, chunks: &[usize], bias: bool) {
        let len: usize = chunks.iter().sum();
        let w = Tensor::from_vec(lcg(c_out * (c_in / g) * k, 11), [c_out, c_in / g, k]).unwrap();
        let b = bias.then(|| Tensor::from_vec(lcg(c_out, 5), [c_out]).unwrap());
        let x = lcg(c_in * len, 17);
        let maxf = (*chunks.iter().max().unwrap()).max(1);
        let mut st = match pad {
            Some(p) => StridedConv1dStream::with_padding(&w, b.as_ref(), s, d, g, p, maxf).unwrap(),
            None => StridedConv1dStream::new(&w, b.as_ref(), s, d, g, maxf).unwrap(),
        };
        let want = strided_ref(&x, c_in, len, &w, b.as_ref(), s, d, g, st.left_padding());
        let mut out = vec![0f32; c_out * st.max_outputs_per_step()];
        let (mut parts, mut off) = (Vec::new(), 0);
        for &n in chunks {
            let expect_m = st.outputs_for(n);
            let m = st.step(&slice_frame(&x, c_in, len, off, n), n, &mut out).unwrap();
            assert_eq!(m, expect_m);
            assert!(m <= st.max_outputs_per_step());
            parts.push(out[..c_out * m].to_vec());
            off += n;
        }
        let got = concat(&parts, c_out);
        assert_eq!(bits(&got), bits(&want), "strided mismatch k={k} s={s} d={d} g={g} pad={pad:?} chunks={chunks:?}");
    }

    #[test]
    fn strided_bit_exact_various_chunkings() {
        run_strided(1, 1, 4, 2, 1, 1, None, &[8], false);
        run_strided(1, 1, 4, 2, 1, 1, None, &[1; 9], false); // odd total, 1-sample frames
        run_strided(3, 4, 5, 3, 1, 1, None, &[2, 7, 1, 4, 6, 5], true);
        run_strided(4, 4, 3, 2, 2, 4, None, &[1, 3, 8, 2, 1], true); // depthwise + dilation
        run_strided(4, 6, 4, 4, 1, 2, None, &[3, 1, 1, 9, 2], true); // grouped, stride == k (pad 1)
        run_strided(2, 3, 2, 5, 1, 1, None, &[3, 4, 1, 7], true); // stride > k: windows skip samples
        run_strided(2, 2, 1, 3, 1, 1, None, &[2, 2, 5], true); // k=1 pure decimation
        run_strided(2, 3, 5, 2, 1, 1, Some(0), &[3, 1, 6], true); // no padding ("valid")
        run_strided(2, 3, 5, 2, 1, 1, Some(4), &[3, 1, 6], true); // full causal padding
        run_strided(1, 2, 3, 1, 1, 1, None, &[4, 1, 2], true); // stride 1 sanity
    }

    #[test]
    fn strided_stride1_matches_causal_stream_bits() {
        let w = Tensor::from_vec(lcg(3 * 2 * 5, 3), [3, 2, 5]).unwrap();
        let b = Tensor::from_vec(lcg(3, 4), [3]).unwrap();
        let mut a = StridedConv1dStream::new(&w, Some(&b), 1, 2, 1, 16).unwrap();
        let mut c = CausalConv1dStream::new(&w, Some(&b), 2, 1, 16).unwrap();
        for (i, n) in [16usize, 3, 1, 9].into_iter().enumerate() {
            let x = lcg(2 * n, 100 + i as u32);
            let (mut ya, mut yc) = (vec![0f32; 3 * 16], vec![0f32; 3 * n]);
            assert_eq!(a.step(&x, n, &mut ya).unwrap(), n);
            c.step(&x, n, &mut yc).unwrap();
            assert_eq!(bits(&ya[..3 * n]), bits(&yc));
        }
    }

    /// Mimi SEANet encoder downsampling ratios (4, 5, 6, 8; k = 2*ratio) at 24 kHz, 1920-sample frames.
    #[test]
    fn strided_codec_frames_mimi_encoder_ratios() {
        for r in [4usize, 5, 6, 8] {
            run_strided(2, 3, 2 * r, r, 1, 1, None, &[1920, 1920, 1920], true);
            run_strided(1, 2, 2 * r, r, 1, 1, None, &[480, 1440, 7, 1913], true); // jittery frame sizes
        }
        // 25 Hz -> 12.5 Hz downsample: k=4, s=2, one latent per step.
        run_strided(4, 4, 4, 2, 1, 1, None, &[1; 6], false);
    }

    // ---------- transposed conv ----------
    fn run_tr(c_in: usize, opg: usize, k: usize, s: usize, d: usize, g: usize, chunks: &[usize], bias: bool) {
        let len: usize = chunks.iter().sum();
        let c_out = opg * g;
        let w = Tensor::from_vec(lcg(c_in * opg * k, 23), [c_in, opg, k]).unwrap();
        let b = bias.then(|| Tensor::from_vec(lcg(c_out, 29), [c_out]).unwrap());
        let x = lcg(c_in * len, 31);
        // Reference: whole-signal transposed conv, causal trim to the first len*s outputs. If the
        // kernel is shorter than the stride, extend with output_padding so len*s outputs exist.
        let full = (len - 1) * s + d * (k - 1) + 1;
        let op = (len * s).saturating_sub(full);
        let y = conv_transpose1d(&Tensor::from_vec(x.clone(), [c_in, len]).unwrap(), &w, b.as_ref(),
            ConvTranspose1dParams { stride: s, dilation: d, groups: g, output_padding: op, padding: 0 }).unwrap();
        let ol = y.dims()[1];
        let yv = y.to_vec_f32();
        let want: Vec<f32> = (0..c_out).flat_map(|oc| yv[oc * ol..oc * ol + len * s].to_vec()).collect();

        let maxf = (*chunks.iter().max().unwrap()).max(1);
        let mut st = ConvTranspose1dStream::new(&w, b.as_ref(), s, d, g, maxf).unwrap();
        let (mut parts, mut off) = (Vec::new(), 0);
        for &n in chunks {
            let mut yf = vec![0f32; c_out * n * s];
            st.step(&slice_frame(&x, c_in, len, off, n), n, &mut yf).unwrap();
            parts.push(yf);
            off += n;
        }
        let got = concat(&parts, c_out);
        assert_eq!(bits(&got), bits(&want), "transposed mismatch k={k} s={s} d={d} g={g} chunks={chunks:?}");
    }

    #[test]
    fn transposed_bit_exact_various_chunkings() {
        run_tr(1, 1, 4, 2, 1, 1, &[5], false);
        run_tr(1, 1, 4, 2, 1, 1, &[1; 5], false);
        run_tr(3, 2, 5, 3, 1, 1, &[2, 1, 4, 1, 3], true); // k not multiple of stride
        run_tr(4, 1, 4, 2, 1, 4, &[1, 3, 2, 1], false); // depthwise (Mimi 12.5 -> 25 Hz upsample)
        run_tr(4, 3, 3, 2, 2, 2, &[2, 1, 5], true); // grouped + dilation
        run_tr(2, 2, 2, 5, 1, 1, &[1, 2, 3], true); // k < stride (zero-contribution gaps)
        run_tr(2, 3, 1, 1, 1, 1, &[3, 3], true); // k=1, s=1: pointwise
        run_tr(2, 2, 7, 1, 1, 1, &[2, 5, 1], true); // stride 1 = full-causal conv with flipped taps
    }

    /// Mimi SEANet decoder ratios (8, 6, 5, 4; k = 2*ratio) fed one 12.5 Hz latent frame at a time.
    #[test]
    fn transposed_codec_frames_mimi_decoder_ratios() {
        for r in [8usize, 6, 5, 4] {
            run_tr(3, 2, 2 * r, r, 1, 1, &[1; 6], true);
            run_tr(2, 2, 2 * r, r, 1, 1, &[2, 1, 3], true);
        }
    }

    /// Full decode-shaped chain: depthwise 2x upsample, then 8·6·5·4 ConvTranspose stages.
    /// One latent per step -> 2*960 = 1920 samples (80 ms at 24 kHz). Streaming == batch, bit-exact.
    #[test]
    fn mimi_decoder_chain_one_latent_per_step_is_1920_samples() {
        let stages: [(usize, usize, usize, usize, usize); 5] = [(4, 1, 4, 2, 4), (4, 1, 16, 8, 1), (1, 2, 12, 6, 1), (2, 1, 10, 5, 1), (1, 1, 8, 4, 1)];
        // (c_in, opg, k, stride, groups)
        let ws: Vec<Tensor> = stages.iter().enumerate()
            .map(|(i, &(ci, o, k, _, _))| Tensor::from_vec(lcg(ci * o * k, 40 + i as u32).iter().map(|v| v * 0.5).collect(), [ci, o, k]).unwrap())
            .collect();
        let bs: Vec<Tensor> = stages.iter().enumerate()
            .map(|(i, &(_, o, _, _, g))| Tensor::from_vec(lcg(o * g, 60 + i as u32), [o * g]).unwrap()).collect();
        let latents = 5usize;
        let x = lcg(4 * latents, 77);
        // Batch reference per stage (trim to len*s).
        let mut cur = x.clone();
        let mut cur_len = latents;
        for (i, &(ci, o, _, s, g)) in stages.iter().enumerate() {
            let y = conv_transpose1d(&Tensor::from_vec(cur.clone(), [ci, cur_len]).unwrap(), &ws[i], Some(&bs[i]),
                ConvTranspose1dParams { stride: s, groups: g, ..Default::default() }).unwrap();
            let ol = y.dims()[1];
            let yv = y.to_vec_f32();
            let nl = cur_len * s;
            cur = (0..o * g).flat_map(|oc| yv[oc * ol..oc * ol + nl].to_vec()).collect();
            cur_len = nl;
        }
        assert_eq!(cur_len, latents * 1920);
        // Streaming, one latent per step, preallocated per-stage buffers.
        let mut sts: Vec<ConvTranspose1dStream> = stages.iter().enumerate()
            .map(|(i, &(_, _, _, s, g))| ConvTranspose1dStream::new(&ws[i], Some(&bs[i]), s, 1, g, stages[..i].iter().map(|t| t.3).product::<usize>()).unwrap())
            .collect();
        let mut bufs: Vec<Vec<f32>> = Vec::new();
        let mut n = 1usize;
        for &(_, o, _, s, g) in &stages { n *= s; bufs.push(vec![0f32; o * g * n]); }
        let mut got = Vec::new();
        for t in 0..latents {
            let mut inp: Vec<f32> = (0..4).map(|c| x[c * latents + t]).collect();
            let mut n_in = 1usize;
            for (i, st) in sts.iter_mut().enumerate() {
                st.step(&inp, n_in, &mut bufs[i]).unwrap();
                n_in *= st.stride();
                inp = bufs[i].clone();
            }
            assert_eq!(n_in, 1920);
            got.push(inp);
        }
        assert_eq!(bits(&concat(&got, 1)), bits(&cur));
    }

    #[test]
    fn reset_and_errors() {
        let w = Tensor::from_vec(lcg(2 * 2 * 4, 1), [2, 2, 4]).unwrap();
        let mut t = ConvTranspose1dStream::new(&w, None, 2, 1, 1, 3).unwrap();
        let x = lcg(2 * 3, 2);
        let (mut a, mut b) = (vec![0f32; 12], vec![0f32; 12]);
        t.step(&x, 3, &mut a).unwrap();
        t.step(&x, 3, &mut b).unwrap();
        assert_ne!(a, b);
        t.reset();
        t.step(&x, 3, &mut b).unwrap();
        assert_eq!(bits(&a), bits(&b));
        assert!(t.step(&lcg(8, 1), 4, &mut [0f32; 16]).is_err());
        assert!(t.step(&x, 3, &mut [0f32; 6]).is_err());
        assert_eq!(t.step_tensor(&Tensor::from_vec(x.clone(), [2, 3]).unwrap()).unwrap().dims(), &[2, 6]);

        let mut s = StridedConv1dStream::new(&w, None, 2, 1, 1, 3).unwrap();
        let mut o = vec![0f32; 2 * s.max_outputs_per_step()];
        let m1 = s.step(&x, 3, &mut o).unwrap();
        let first = o[..2 * m1].to_vec();
        s.step(&x, 3, &mut o).unwrap();
        s.reset();
        assert_eq!(s.step(&x, 3, &mut o).unwrap(), m1);
        assert_eq!(bits(&o[..2 * m1]), bits(&first));
        assert!(s.step(&lcg(8, 1), 4, &mut o).is_err());
        assert!(StridedConv1dStream::with_padding(&w, None, 2, 1, 1, 4, 3).is_err()); // pad > k-1
        assert!(StridedConv1dStream::new(&w, None, 0, 1, 1, 3).is_err());
        assert!(ConvTranspose1dStream::new(&w, Some(&Tensor::zeros([3])), 2, 1, 1, 3).is_err());
        assert_eq!(s.step_tensor(&Tensor::from_vec(x, [2, 3]).unwrap()).unwrap().dims()[0], 2);
    }
}
