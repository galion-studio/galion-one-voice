// Galion Voice — Perpetual Open MIT License
// SPDX-License-Identifier: MIT
//! SEANet causal conv encoder / decoder — op inventory **and real streaming kernels** (A6 wave 2).
//!
//! Encoder (audio [1, T] → latent [C_lat, T/hop]), VERIFY against real config:
//! ```text
//! Conv1d(1 → 64, k=7)                                  causal pad
//! for ratio r in [4,5,6,8], ch doubling 64→128→256→512→1024:
//!     ResidualUnit(ch): ELU → Conv1d(ch→ch/2,k=3,dil=1) → ELU → Conv1d(ch/2→ch,k=1) + skip
//!     ELU → Conv1d(ch → 2ch, k=2r, stride=r)           causal pad (k-1)+1-r
//! ELU → Conv1d(1024 → 512, k=3)
//! ```
//! Decoder: Conv1d(512 → 1024, k=7) → for r in [8,6,5,4]: ELU → ConvTranspose1d(ch→ch/2, k=2r, s=r)
//! (right-trimmed, causal) → ResidualUnit → … → ELU → Conv1d(64 → 1, k=3). No final tanh (VERIFY).
//!
//! Kernels (galion-tensor, A5): stride-1 convs (incl. k=1 pointwise) → [`CausalConv1dStream`];
//! strided downsampling → [`StridedConv1dStream`]; upsampling → [`ConvTranspose1dStream`].
//! Every buffer is allocated in `new`; `forward` does **zero heap allocation** (tests/zero_alloc.rs).
//!
//! Streaming == batch: any frame sequence through [`SeanetEncoder`]/[`SeanetDecoder`] gives the same
//! f32 bits as [`reference`] run on the whole signal (tests/stream_vs_batch.rs, synthetic weights).
//!
//! Weights: this module never fabricates weights. It reads them from a
//! [`crate::weights::WeightSource`]. Without a real source the codec does not build these kernels
//! at all (see `codec.rs`), and `IS_STUB` stays true either way.

use crate::config::MimiConfig;
use crate::weights::{fetch, LoadError, WeightSource};
use galion_tensor::{CausalConv1dStream, ConvTranspose1dStream, StridedConv1dStream};

/// One conv layer's static description (op inventory, weight contract, kernel construction).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConvSpec {
    pub name: String,
    pub in_ch: usize,
    pub out_ch: usize,
    pub kernel: usize,
    pub stride: usize,
    pub dilation: usize,
    pub transpose: bool,
    /// 1 = dense; `in_ch` = depthwise (Mimi's 25→12.5 Hz upsample is depthwise / channel-wise).
    pub groups: usize,
    pub bias: bool,
}

impl ConvSpec {
    /// Streaming state length in samples this layer must remember between frames.
    pub fn state_len(&self) -> usize {
        if self.transpose {
            self.kernel.saturating_sub(self.stride)
        } else {
            ((self.kernel - 1) * self.dilation + 1).saturating_sub(self.stride)
        }
    }
    /// Weight shape: Conv1d [out, in/groups, k]; ConvTranspose1d [in, out/groups, k].
    pub fn weight_shape(&self) -> [usize; 3] {
        if self.transpose {
            [self.in_ch, self.out_ch / self.groups, self.kernel]
        } else {
            [self.out_ch, self.in_ch / self.groups, self.kernel]
        }
    }
    /// Causal left padding used by the batch reference (Mimi: `(k-1)*d + 1 - s`; stride 1: `(k-1)*d`).
    pub fn left_pad(&self) -> usize {
        ((self.kernel - 1) * self.dilation + 1).saturating_sub(self.stride)
    }
    /// Output samples per channel for `n` input samples on a frame grid (`n % stride == 0`).
    pub fn out_len(&self, n: usize) -> usize {
        if self.transpose { n * self.stride } else { n / self.stride }
    }
}

fn conv(name: String, i: usize, o: usize, k: usize, s: usize, t: bool) -> ConvSpec {
    ConvSpec { name, in_ch: i, out_ch: o, kernel: k, stride: s, dilation: 1, transpose: t, groups: 1, bias: true }
}

/// Encoder conv inventory, in execution order.
pub fn encoder_convs(cfg: &MimiConfig) -> Vec<ConvSpec> {
    let mut v = Vec::new();
    let mut ch = cfg.seanet_filters;
    v.push(conv("encoder.conv_in".into(), cfg.channels, ch, 7, 1, false));
    for (b, &r) in cfg.seanet_ratios.iter().enumerate() {
        v.push(conv(format!("encoder.block{b}.res.conv1"), ch, ch / 2, 3, 1, false));
        v.push(conv(format!("encoder.block{b}.res.conv2"), ch / 2, ch, 1, 1, false));
        v.push(conv(format!("encoder.block{b}.down"), ch, ch * 2, 2 * r, r, false));
        ch *= 2;
    }
    v.push(conv("encoder.conv_out".into(), ch, cfg.latent_dim, 3, 1, false));
    v
}

/// Decoder conv inventory, in execution order.
pub fn decoder_convs(cfg: &MimiConfig) -> Vec<ConvSpec> {
    let mut v = Vec::new();
    let mut ch = cfg.seanet_filters << cfg.seanet_ratios.len();
    v.push(conv("decoder.conv_in".into(), cfg.latent_dim, ch, 7, 1, false));
    for (b, &r) in cfg.seanet_ratios.iter().rev().enumerate() {
        v.push(conv(format!("decoder.block{b}.up"), ch, ch / 2, 2 * r, r, true));
        ch /= 2;
        v.push(conv(format!("decoder.block{b}.res.conv1"), ch, ch / 2, 3, 1, false));
        v.push(conv(format!("decoder.block{b}.res.conv2"), ch / 2, ch, 1, 1, false));
    }
    // VERIFY: Mimi `last_kernel_size = 3` (was k=7 in the wave-1 stub inventory).
    v.push(conv("decoder.conv_out".into(), ch, cfg.channels, 3, 1, false));
    v
}

/// 25 Hz ⇄ 12.5 Hz resampling convs: `[downsample, upsample]`. No bias (VERIFY).
/// Downsample: dense Conv1d(d→d, k=2f, s=f). Upsample: **depthwise** ConvTranspose1d(d→d, k=2f, s=f, groups=d).
/// Known delta: upstream downsample uses replicate padding; A5 streams use constant (skip) padding,
/// which only differs on the first output after stream start. VERIFY with golden frames.
pub fn resample_convs(cfg: &MimiConfig) -> [ConvSpec; 2] {
    let (d, f) = (cfg.latent_dim, cfg.frame_downsample);
    [
        ConvSpec { name: "downsample".into(), in_ch: d, out_ch: d, kernel: 2 * f, stride: f, dilation: 1, transpose: false, groups: 1, bias: false },
        ConvSpec { name: "upsample".into(), in_ch: d, out_ch: d, kernel: 2 * f, stride: f, dilation: 1, transpose: true, groups: d, bias: false },
    ]
}

/// ELU(α=1), elementwise `src → dst`. Same expression in the streaming path and [`reference`].
#[inline]
pub fn elu_into(src: &[f32], dst: &mut [f32]) {
    for (d, &s) in dst.iter_mut().zip(src) {
        *d = if s > 0.0 { s } else { s.exp_m1() };
    }
}

enum Kernel {
    Causal(CausalConv1dStream),
    Strided(StridedConv1dStream),
    Transposed(ConvTranspose1dStream),
}

/// A conv layer bound to a fixed per-frame input length, backed by an A5 streaming kernel.
pub struct StreamConv {
    pub spec: ConvSpec,
    pub n_in: usize,
    pub n_out: usize,
    kernel: Kernel,
}

impl StreamConv {
    /// Build from weights. Allocates (construction time only).
    pub fn build(spec: &ConvSpec, src: &dyn WeightSource, n_in: usize) -> Result<Self, LoadError> {
        if !spec.transpose && n_in % spec.stride != 0 {
            return Err(LoadError::Kernel(format!("{}: frame {n_in} not a multiple of stride {}", spec.name, spec.stride)));
        }
        let w = fetch(src, &format!("{}.weight", spec.name), &spec.weight_shape())?;
        let b = if spec.bias { Some(fetch(src, &format!("{}.bias", spec.name), &[spec.out_ch])?) } else { None };
        let ke = |e: galion_tensor::TensorError| LoadError::Kernel(format!("{}: {e}", spec.name));
        let kernel = match (spec.transpose, spec.stride) {
            (true, s) => Kernel::Transposed(ConvTranspose1dStream::new(&w, b.as_ref(), s, spec.dilation, spec.groups, n_in).map_err(ke)?),
            (false, 1) => Kernel::Causal(CausalConv1dStream::new(&w, b.as_ref(), spec.dilation, spec.groups, n_in).map_err(ke)?),
            (false, s) => Kernel::Strided(StridedConv1dStream::new(&w, b.as_ref(), s, spec.dilation, spec.groups, n_in).map_err(ke)?),
        };
        Ok(Self { spec: spec.clone(), n_in, n_out: spec.out_len(n_in), kernel })
    }
    pub fn in_floats(&self) -> usize {
        self.spec.in_ch * self.n_in
    }
    pub fn out_floats(&self) -> usize {
        self.spec.out_ch * self.n_out
    }
    /// `x[..c_in*n_in]` → `out[..c_out*n_out]`. Zero allocation on success.
    pub fn step(&mut self, x: &[f32], out: &mut [f32]) -> Result<(), String> {
        let (xi, o) = (&x[..self.in_floats()], &mut out[..self.spec.out_ch * self.n_out]);
        let r = match &mut self.kernel {
            Kernel::Causal(c) => c.step(xi, self.n_in, o),
            Kernel::Transposed(t) => t.step(xi, self.n_in, o),
            Kernel::Strided(s) => match s.step(xi, self.n_in, o) {
                Ok(m) if m == self.n_out => Ok(()),
                Ok(m) => return Err(format!("{}: strided emitted {m} != {}", self.spec.name, self.n_out)),
                Err(e) => Err(e),
            },
        };
        r.map_err(|e| format!("{}: {e}", self.spec.name))
    }
    pub fn reset(&mut self) {
        match &mut self.kernel {
            Kernel::Causal(c) => c.reset(),
            Kernel::Strided(s) => s.reset(),
            Kernel::Transposed(t) => t.reset(),
        }
    }
}

/// ELU → conv1(k3) → ELU → conv2(k1 pointwise) → + skip (identity shortcut, VERIFY `true_skip`).
pub struct ResUnit {
    pub conv1: StreamConv,
    pub conv2: StreamConv,
}

impl ResUnit {
    /// In place on `x[..ch*n]`, using `tmp`/`h` scratch (≥ the layer's activation size).
    fn step(&mut self, x: &mut [f32], tmp: &mut [f32], h: &mut [f32]) -> Result<(), String> {
        let n = self.conv1.in_floats();
        elu_into(&x[..n], &mut tmp[..n]);
        self.conv1.step(tmp, h)?;
        let m = self.conv1.out_floats();
        elu_into(&h[..m], &mut tmp[..m]);
        self.conv2.step(tmp, h)?;
        for (a, &b) in x[..n].iter_mut().zip(&h[..n]) {
            *a += b;
        }
        Ok(())
    }
    fn reset(&mut self) {
        self.conv1.reset();
        self.conv2.reset();
    }
}

fn scratch_len(convs: &[&StreamConv]) -> usize {
    convs.iter().map(|c| c.in_floats().max(c.out_floats())).max().unwrap_or(0)
}

/// Streaming SEANet encoder: `frame` PCM samples → `[latent_dim, frame/hop]`.
pub struct SeanetEncoder {
    pub conv_in: StreamConv,
    pub blocks: Vec<(ResUnit, StreamConv)>,
    pub conv_out: StreamConv,
    cur: Vec<f32>,
    tmp: Vec<f32>,
    h: Vec<f32>,
}

impl SeanetEncoder {
    pub fn new(cfg: &MimiConfig, src: &dyn WeightSource, frame: usize) -> Result<Self, LoadError> {
        let specs = encoder_convs(cfg);
        let mut n = frame;
        let mut it = specs.iter();
        let mut next = |n: &mut usize| -> Result<StreamConv, LoadError> {
            let c = StreamConv::build(it.next().expect("encoder spec"), src, *n)?;
            *n = c.n_out;
            Ok(c)
        };
        let conv_in = next(&mut n)?;
        let mut blocks = Vec::new();
        for _ in 0..cfg.seanet_ratios.len() {
            let res = ResUnit { conv1: next(&mut n)?, conv2: next(&mut n)? };
            blocks.push((res, next(&mut n)?));
        }
        let conv_out = next(&mut n)?;
        let mut all = vec![&conv_in, &conv_out];
        for (r, d) in &blocks {
            all.extend([&r.conv1, &r.conv2, d]);
        }
        let s = scratch_len(&all);
        Ok(Self { conv_in, blocks, conv_out, cur: vec![0.0; s], tmp: vec![0.0; s], h: vec![0.0; s] })
    }
    pub fn frame(&self) -> usize {
        self.conv_in.n_in
    }
    pub fn out_steps(&self) -> usize {
        self.conv_out.n_out
    }
    /// `pcm[..frame]` → `out[..latent_dim*out_steps]`. Zero allocation.
    pub fn forward(&mut self, pcm: &[f32], out: &mut [f32]) -> Result<(), String> {
        if pcm.len() != self.frame() || out.len() != self.conv_out.out_floats() {
            return Err(format!("seanet enc: pcm {} / out {}", pcm.len(), out.len()));
        }
        let (cur, tmp, h) = (&mut self.cur, &mut self.tmp, &mut self.h);
        self.conv_in.step(pcm, cur)?;
        for (res, down) in &mut self.blocks {
            res.step(cur, tmp, h)?;
            let n = down.in_floats();
            elu_into(&cur[..n], &mut tmp[..n]);
            down.step(tmp, cur)?;
        }
        let n = self.conv_out.in_floats();
        elu_into(&cur[..n], &mut tmp[..n]);
        self.conv_out.step(tmp, out)
    }
    pub fn reset(&mut self) {
        self.conv_in.reset();
        self.conv_out.reset();
        for (r, d) in &mut self.blocks {
            r.reset();
            d.reset();
        }
    }
}

/// Streaming SEANet decoder: `[latent_dim, steps]` → `steps*hop` PCM samples.
pub struct SeanetDecoder {
    pub conv_in: StreamConv,
    pub blocks: Vec<(StreamConv, ResUnit)>,
    pub conv_out: StreamConv,
    cur: Vec<f32>,
    tmp: Vec<f32>,
    h: Vec<f32>,
}

impl SeanetDecoder {
    pub fn new(cfg: &MimiConfig, src: &dyn WeightSource, steps: usize) -> Result<Self, LoadError> {
        let specs = decoder_convs(cfg);
        let mut n = steps;
        let mut it = specs.iter();
        let mut next = |n: &mut usize| -> Result<StreamConv, LoadError> {
            let c = StreamConv::build(it.next().expect("decoder spec"), src, *n)?;
            *n = c.n_out;
            Ok(c)
        };
        let conv_in = next(&mut n)?;
        let mut blocks = Vec::new();
        for _ in 0..cfg.seanet_ratios.len() {
            let up = next(&mut n)?;
            blocks.push((up, ResUnit { conv1: next(&mut n)?, conv2: next(&mut n)? }));
        }
        let conv_out = next(&mut n)?;
        let mut all = vec![&conv_in, &conv_out];
        for (u, r) in &blocks {
            all.extend([u, &r.conv1, &r.conv2]);
        }
        let s = scratch_len(&all);
        Ok(Self { conv_in, blocks, conv_out, cur: vec![0.0; s], tmp: vec![0.0; s], h: vec![0.0; s] })
    }
    pub fn in_steps(&self) -> usize {
        self.conv_in.n_in
    }
    pub fn out_samples(&self) -> usize {
        self.conv_out.n_out
    }
    /// `latent[..latent_dim*steps]` → `pcm_out[..steps*hop]`. Zero allocation.
    pub fn forward(&mut self, latent: &[f32], pcm_out: &mut [f32]) -> Result<(), String> {
        if latent.len() != self.conv_in.in_floats() || pcm_out.len() != self.conv_out.out_floats() {
            return Err(format!("seanet dec: latent {} / pcm {}", latent.len(), pcm_out.len()));
        }
        let (cur, tmp, h) = (&mut self.cur, &mut self.tmp, &mut self.h);
        self.conv_in.step(latent, cur)?;
        for (up, res) in &mut self.blocks {
            let n = up.in_floats();
            elu_into(&cur[..n], &mut tmp[..n]);
            up.step(tmp, cur)?;
            res.step(cur, tmp, h)?;
        }
        let n = self.conv_out.in_floats();
        elu_into(&cur[..n], &mut tmp[..n]);
        self.conv_out.step(tmp, pcm_out)
    }
    pub fn reset(&mut self) {
        self.conv_in.reset();
        self.conv_out.reset();
        for (u, r) in &mut self.blocks {
            u.reset();
            r.reset();
        }
    }
}

/// Whole-signal batch reference built on `galion_tensor::ops::{conv1d, conv_transpose1d}`.
/// **Allocates freely**; tests/oracle only, never on the frame path.
pub mod reference {
    use super::*;
    use galion_tensor::ops::{conv1d, conv_transpose1d, Conv1dParams, ConvTranspose1dParams};
    use galion_tensor::Tensor;

    /// One conv over a whole `[in_ch, len]` signal, causal, keeping the outputs a stream would emit.
    pub fn conv(spec: &ConvSpec, src: &dyn WeightSource, x: &[f32], len: usize) -> Result<(Vec<f32>, usize), String> {
        let w = fetch(src, &format!("{}.weight", spec.name), &spec.weight_shape()).map_err(|e| format!("{e:?}"))?;
        let b = if spec.bias { Some(fetch(src, &format!("{}.bias", spec.name), &[spec.out_ch]).map_err(|e| format!("{e:?}"))?) } else { None };
        let xt = Tensor::from_vec(x[..spec.in_ch * len].to_vec(), vec![spec.in_ch, len]).map_err(|e| e.to_string())?;
        let y = if spec.transpose {
            let p = ConvTranspose1dParams { stride: spec.stride, padding: 0, output_padding: 0, dilation: spec.dilation, groups: spec.groups };
            conv_transpose1d(&xt, &w, b.as_ref(), p)
        } else {
            let p = Conv1dParams { stride: spec.stride, padding: spec.left_pad(), dilation: spec.dilation, groups: spec.groups };
            conv1d(&xt, &w, b.as_ref(), p)
        }
        .map_err(|e| format!("{}: {e}", spec.name))?;
        let full = y.dims()[1];
        let keep = spec.out_len(len);
        let yv = y.to_vec_f32();
        let mut out = Vec::with_capacity(spec.out_ch * keep);
        for c in 0..spec.out_ch {
            out.extend_from_slice(&yv[c * full..c * full + keep]);
        }
        Ok((out, keep))
    }

    fn elu(x: &[f32]) -> Vec<f32> {
        let mut y = vec![0.0; x.len()];
        elu_into(x, &mut y);
        y
    }

    fn res(c1: &ConvSpec, c2: &ConvSpec, src: &dyn WeightSource, x: &mut [f32], len: usize) -> Result<(), String> {
        let (h, _) = conv(c1, src, &elu(x), len)?;
        let (y, _) = conv(c2, src, &elu(&h), len)?;
        x.iter_mut().zip(&y).for_each(|(a, b)| *a += b);
        Ok(())
    }

    /// Whole-signal SEANet encoder. Returns `[latent_dim, len/hop]` row-major.
    pub fn encode(cfg: &MimiConfig, src: &dyn WeightSource, pcm: &[f32]) -> Result<Vec<f32>, String> {
        let s = encoder_convs(cfg);
        let (mut x, mut len) = conv(&s[0], src, pcm, pcm.len())?;
        for b in 0..cfg.seanet_ratios.len() {
            let i = 1 + 3 * b;
            res(&s[i], &s[i + 1], src, &mut x, len)?;
            (x, len) = conv(&s[i + 2], src, &elu(&x), len)?;
        }
        Ok(conv(s.last().unwrap(), src, &elu(&x), len)?.0)
    }

    /// Whole-signal SEANet decoder. `latent`: `[latent_dim, steps]`.
    pub fn decode(cfg: &MimiConfig, src: &dyn WeightSource, latent: &[f32], steps: usize) -> Result<Vec<f32>, String> {
        let s = decoder_convs(cfg);
        let (mut x, mut len) = conv(&s[0], src, latent, steps)?;
        for b in 0..cfg.seanet_ratios.len() {
            let i = 1 + 3 * b;
            (x, len) = conv(&s[i], src, &elu(&x), len)?;
            res(&s[i + 1], &s[i + 2], src, &mut x, len)?;
        }
        Ok(conv(s.last().unwrap(), src, &elu(&x), len)?.0)
    }
}
