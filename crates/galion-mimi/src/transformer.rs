// Galion Voice — Perpetual Open MIT License
// SPDX-License-Identifier: MIT
//! Causal streaming transformer bottleneck — op inventory, **real streaming kernel** (W20-15) and
//! the shape-only identity placeholder used when no weights are present.
//!
//! Two instances in Mimi: one after the encoder (pre-quantizer), one before the decoder, both at
//! 25 Hz. Per layer (pre-norm), VERIFY against the real config once A21 clears the weights:
//! ```text
//! x = x + LayerScale1 ⊙ out_proj(MHA(in_proj(LayerNorm1(x))))   causal, RoPE, window = context
//! x = x + LayerScale2 ⊙ fc2(GELU(fc1(LayerNorm2(x))))           Linear(d→ffn) → GELU → Linear(ffn→d)
//! ```
//! No biases on in_proj / out_proj / fc1 / fc2 (upstream `bias_attn = bias_ff = False`, VERIFY);
//! LayerNorm has weight + bias, eps [`LN_EPS`]. `in_proj` rows are `[q; k; v]`, each `[heads, head_dim]`.
//!
//! Kernels: [`StreamingTransformer`] runs one 25 Hz step at a time on galion-tensor (A4/A5):
//! `gemv_f32_into` (rows==1 GEMV, `Exec::Scalar`), [`StreamingAttention`] (RoPE-NeoX + [`galion_tensor::KvRing`]
//! sliding window of `context` steps). LayerNorm and exact-erf GELU live here (galion-tensor has
//! `rms_norm` and tanh-GELU only). All scratch is allocated in `new`; `step` is allocation-free.
//!
//! **RoPE convention (VERIFY):** galion-tensor rotates NeoX halves `(i, i + hd/2)`; upstream Moshi/Mimi
//! rotates interleaved pairs `(2i, 2i + 1)`. The two are equivalent after permuting the q and k rows
//! of `in_proj` per head at **convert time** — [`permute_qk_interleaved_to_neox`] (tested against an
//! interleaved f64 oracle in `tests/transformer.rs`). Runtime never sees interleaved weights.
//!
//! [`reference::forward`] is the whole-sequence batch reference (layer by layer, `attention_reference`);
//! the streaming kernel is bit-identical to it. A separate f64 oracle in tests checks the maths.
//!
//! No weights are fabricated: tensors come from a [`WeightSource`]; `IS_STUB` stays true.

use crate::config::MimiConfig;
use crate::tensor::FakeTensor;
use crate::weights::{fetch, LoadError, WeightSource};
use galion_tensor::{gemv_f32_into, Exec, StreamingAttention};

/// LayerNorm epsilon. VERIFY: upstream `nn.LayerNorm` default 1e-5.
pub const LN_EPS: f32 = 1e-5;
/// RoPE base (upstream `max_period`). VERIFY: 10_000.
pub const ROPE_THETA: f32 = 10_000.0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransformerShapes {
    pub d_model: usize,
    pub heads: usize,
    pub head_dim: usize,
    pub ffn: usize,
    pub layers: usize,
    pub context: usize,
}

impl TransformerShapes {
    pub fn from_cfg(cfg: &MimiConfig) -> Self {
        Self {
            d_model: cfg.latent_dim,
            heads: cfg.transformer_heads,
            head_dim: cfg.latent_dim / cfg.transformer_heads,
            ffn: cfg.transformer_ffn,
            layers: cfg.transformer_layers,
            context: cfg.transformer_context,
        }
    }
    /// KV ring floats per layer: K and V, `context × d_model` each.
    pub fn kv_floats_per_layer(&self) -> usize {
        2 * self.context * self.d_model
    }
}

/// Weight names + shapes of one layer under `prefix` (e.g. `enc_transformer`). Single source of truth
/// for both the kernel and `weights::expected_tensors`.
pub fn layer_tensors(prefix: &str, l: usize, d: usize, ffn: usize) -> Vec<(String, Vec<usize>)> {
    let p = format!("{prefix}.layer{l}");
    vec![
        (format!("{p}.norm1.weight"), vec![d]),
        (format!("{p}.norm1.bias"), vec![d]),
        (format!("{p}.attn.in_proj.weight"), vec![3 * d, d]),
        (format!("{p}.attn.out_proj.weight"), vec![d, d]),
        (format!("{p}.layer_scale1"), vec![d]),
        (format!("{p}.norm2.weight"), vec![d]),
        (format!("{p}.norm2.bias"), vec![d]),
        (format!("{p}.mlp.fc1.weight"), vec![ffn, d]),
        (format!("{p}.mlp.fc2.weight"), vec![d, ffn]),
        (format!("{p}.layer_scale2"), vec![d]),
    ]
}

/// LayerNorm over one vector: `(x - mean) / sqrt(var + eps) * w + b` (biased variance). No allocation.
pub fn layer_norm_into(x: &[f32], w: &[f32], b: &[f32], eps: f32, out: &mut [f32]) {
    let n = x.len() as f32;
    let mut s = 0.0f32;
    for &v in x {
        s += v;
    }
    let mean = s / n;
    let mut var = 0.0f32;
    for &v in x {
        let c = v - mean;
        var += c * c;
    }
    let inv = 1.0 / (var / n + eps).sqrt();
    for i in 0..x.len() {
        out[i] = (x[i] - mean) * inv * w[i] + b[i];
    }
}

/// erf, Abramowitz & Stegun 7.1.26 (|err| ≤ 1.5e-7). Rust std has no `erf`.
#[inline]
pub fn erf(x: f32) -> f32 {
    let s = x.signum();
    let a = x.abs();
    let t = 1.0 / (1.0 + 0.327_591_1 * a);
    let y = 1.0 - (((((1.061_405_4 * t - 1.453_152_1) * t) + 1.421_413_7) * t - 0.284_496_74) * t + 0.254_829_6) * t * (-a * a).exp();
    s * y
}

/// Exact (erf) GELU, upstream `F.gelu` default (VERIFY). In place, no allocation.
#[inline]
pub fn gelu_erf_inplace(x: &mut [f32]) {
    const INV_SQRT2: f32 = std::f32::consts::FRAC_1_SQRT_2;
    for v in x {
        *v = 0.5 * *v * (1.0 + erf(*v * INV_SQRT2));
    }
}

/// Convert-time helper: reorder the q and k rows of a `[3d, d]` `in_proj` so that NeoX RoPE on the
/// result equals interleaved RoPE on the original. Per head: new row `i` ← old `2i`, new row
/// `hd/2 + i` ← old `2i + 1`. V rows untouched (attention output is invariant). For A7's converter.
pub fn permute_qk_interleaved_to_neox(in_proj: &mut [f32], d: usize, heads: usize) {
    assert_eq!(in_proj.len(), 3 * d * d);
    let hd = d / heads;
    let half = hd / 2;
    for blk in 0..2 {
        let rows = &mut in_proj[blk * d * d..(blk + 1) * d * d];
        let orig = rows.to_vec();
        for h in 0..heads {
            for i in 0..half {
                let (dst_a, dst_b) = (h * hd + i, h * hd + half + i);
                let (src_a, src_b) = (h * hd + 2 * i, h * hd + 2 * i + 1);
                rows[dst_a * d..(dst_a + 1) * d].copy_from_slice(&orig[src_a * d..(src_a + 1) * d]);
                rows[dst_b * d..(dst_b + 1) * d].copy_from_slice(&orig[src_b * d..(src_b + 1) * d]);
            }
        }
    }
}

struct LayerWeights {
    n1w: Vec<f32>,
    n1b: Vec<f32>,
    in_proj: Vec<f32>,
    out_proj: Vec<f32>,
    ls1: Vec<f32>,
    n2w: Vec<f32>,
    n2b: Vec<f32>,
    fc1: Vec<f32>,
    fc2: Vec<f32>,
    ls2: Vec<f32>,
}

impl LayerWeights {
    fn load(src: &dyn WeightSource, prefix: &str, l: usize, s: &TransformerShapes) -> Result<Self, LoadError> {
        let mut v = Vec::with_capacity(10);
        for (name, shape) in layer_tensors(prefix, l, s.d_model, s.ffn) {
            v.push(fetch(src, &name, &shape)?.to_vec_f32());
        }
        let mut it = v.into_iter();
        let mut n = || it.next().unwrap();
        Ok(Self { n1w: n(), n1b: n(), in_proj: n(), out_proj: n(), ls1: n(), n2w: n(), n2b: n(), fc1: n(), fc2: n(), ls2: n() })
    }
}

fn kerr(e: galion_tensor::TensorError) -> String {
    e.to_string()
}

/// Per-token pieces shared by the streaming kernel and [`reference`] (same expressions ⇒ same bits).
fn linear(w: &[f32], m: usize, k: usize, x: &[f32], y: &mut [f32]) -> Result<(), String> {
    gemv_f32_into(w, m, k, x, None, y, Exec::Scalar).map_err(kerr)
}
fn add_scaled(x: &mut [f32], scale: &[f32], o: &[f32]) {
    for i in 0..x.len() {
        x[i] += scale[i] * o[i];
    }
}

/// Real streaming transformer: one 25 Hz step per token, KV ring of `context` steps per layer.
pub struct StreamingTransformer {
    pub shapes: TransformerShapes,
    layers: Vec<(LayerWeights, StreamingAttention)>,
    tok: Vec<f32>,
    h: Vec<f32>,
    qkv: Vec<f32>,
    att: Vec<f32>,
    o: Vec<f32>,
    ff: Vec<f32>,
}

impl StreamingTransformer {
    /// Build from a weight source under `prefix` (`enc_transformer` / `dec_transformer`).
    pub fn new(cfg: &MimiConfig, src: &dyn WeightSource, prefix: &str) -> Result<Self, LoadError> {
        let s = TransformerShapes::from_cfg(cfg);
        let mut layers = Vec::with_capacity(s.layers);
        for l in 0..s.layers {
            let w = LayerWeights::load(src, prefix, l, &s)?;
            let a = StreamingAttention::new(s.heads, s.heads, s.head_dim, s.context, ROPE_THETA, 0).map_err(|e| LoadError::Kernel(e.to_string()))?;
            layers.push((w, a));
        }
        let d = s.d_model;
        Ok(Self { tok: vec![0.0; d], h: vec![0.0; d], qkv: vec![0.0; 3 * d], att: vec![0.0; d], o: vec![0.0; d], ff: vec![0.0; s.ffn], layers, shapes: s })
    }

    /// Absolute position of the next step (same for every layer).
    pub fn position(&self) -> usize {
        self.layers.first().map_or(0, |(_, a)| a.position())
    }

    /// One token in place: `x[d_model]`. No allocation.
    pub fn step_token(&mut self, x: &mut [f32]) -> Result<(), String> {
        let (d, f) = (self.shapes.d_model, self.shapes.ffn);
        if x.len() != d {
            return Err(format!("transformer token {} != {d}", x.len()));
        }
        for (w, attn) in self.layers.iter_mut() {
            layer_norm_into(x, &w.n1w, &w.n1b, LN_EPS, &mut self.h);
            linear(&w.in_proj, 3 * d, d, &self.h, &mut self.qkv)?;
            let (q, kv) = self.qkv.split_at(d);
            let (k, v) = kv.split_at(d);
            attn.step(q, k, v, &mut self.att).map_err(kerr)?;
            linear(&w.out_proj, d, d, &self.att, &mut self.o)?;
            add_scaled(x, &w.ls1, &self.o);
            layer_norm_into(x, &w.n2w, &w.n2b, LN_EPS, &mut self.h);
            linear(&w.fc1, f, d, &self.h, &mut self.ff)?;
            gelu_erf_inplace(&mut self.ff);
            linear(&w.fc2, d, f, &self.ff, &mut self.o)?;
            add_scaled(x, &w.ls2, &self.o);
        }
        Ok(())
    }

    /// `x`: `[d_model, steps]` channel-major (the codec's latent layout). In place, no allocation.
    pub fn step(&mut self, x: &mut FakeTensor) -> Result<(), String> {
        let d = self.shapes.d_model;
        if x.shape.len() != 2 || x.shape[0] != d {
            return Err(format!("transformer in shape {:?}", x.shape));
        }
        let n = x.shape[1];
        let mut tok = std::mem::take(&mut self.tok);
        let mut r = Ok(());
        for t in 0..n {
            for c in 0..d {
                tok[c] = x.data[c * n + t];
            }
            r = self.step_token(&mut tok);
            if r.is_err() {
                break;
            }
            for c in 0..d {
                x.data[c * n + t] = tok[c];
            }
        }
        self.tok = tok;
        r
    }

    /// Clear every layer's KV ring (stream restart). No allocation.
    pub fn reset(&mut self) {
        self.layers.iter_mut().for_each(|(_, a)| a.reset());
    }
}

/// Whole-sequence batch reference: layer by layer over all steps, `attention_reference` with a
/// `context` window. Allocating; tests only. Bit-identical to [`StreamingTransformer`].
pub mod reference {
    use super::*;
    use galion_tensor::{attention_reference, Tensor};

    /// `x`: `[d_model, steps]` channel-major. Returns the same layout.
    pub fn forward(cfg: &MimiConfig, src: &dyn WeightSource, prefix: &str, x: &[f32], steps: usize) -> Result<Vec<f32>, String> {
        let s = TransformerShapes::from_cfg(cfg);
        let (d, f, nh, hd) = (s.d_model, s.ffn, s.heads, s.head_dim);
        if x.len() != d * steps {
            return Err("reference transformer: bad input length".into());
        }
        // token-major working copy
        let mut xs: Vec<Vec<f32>> = (0..steps).map(|t| (0..d).map(|c| x[c * steps + t]).collect()).collect();
        let (mut h, mut qkv, mut o, mut ff) = (vec![0.0; d], vec![0.0; 3 * d], vec![0.0; d], vec![0.0; f]);
        for l in 0..s.layers {
            let w = LayerWeights::load(src, prefix, l, &s).map_err(|e| format!("{e:?}"))?;
            let (mut q, mut k, mut v) = (Vec::new(), Vec::new(), Vec::new());
            for xt in &xs {
                layer_norm_into(xt, &w.n1w, &w.n1b, LN_EPS, &mut h);
                linear(&w.in_proj, 3 * d, d, &h, &mut qkv)?;
                q.extend_from_slice(&qkv[..d]);
                k.extend_from_slice(&qkv[d..2 * d]);
                v.extend_from_slice(&qkv[2 * d..]);
            }
            let t3 = |v: Vec<f32>| Tensor::from_vec(v, [steps, nh, hd]).map_err(kerr);
            let att = attention_reference(&t3(q)?, &t3(k)?, &t3(v)?, 0, ROPE_THETA, Some(s.context)).map_err(kerr)?.to_vec_f32();
            for (t, xt) in xs.iter_mut().enumerate() {
                linear(&w.out_proj, d, d, &att[t * d..(t + 1) * d], &mut o)?;
                add_scaled(xt, &w.ls1, &o);
                layer_norm_into(xt, &w.n2w, &w.n2b, LN_EPS, &mut h);
                linear(&w.fc1, f, d, &h, &mut ff)?;
                gelu_erf_inplace(&mut ff);
                linear(&w.fc2, d, f, &ff, &mut o)?;
                add_scaled(xt, &w.ls2, &o);
            }
        }
        let mut out = vec![0.0; d * steps];
        for (t, xt) in xs.iter().enumerate() {
            for c in 0..d {
                out[c * steps + t] = xt[c];
            }
        }
        Ok(out)
    }
}

/// Shape-only streaming transformer used when **no weight source** is present. Holds a fake-zero
/// KV ring sized like the real one.
pub struct TransformerStub {
    pub shapes: TransformerShapes,
    pub kv: Vec<FakeTensor>,
    pub pos: usize,
}

impl TransformerStub {
    pub fn new(cfg: &MimiConfig) -> Self {
        let shapes = TransformerShapes::from_cfg(cfg);
        let kv = (0..shapes.layers).map(|_| FakeTensor::zeros(&[2, shapes.context, shapes.d_model])).collect();
        Self { shapes, kv, pos: 0 }
    }
    /// x: [d_model, steps]. **IDENTITY PLACEHOLDER** for the no-weights path only — leaves `x`
    /// unchanged (it is all zeros there) and advances the stream position. With a weight source the
    /// codec uses [`StreamingTransformer`] instead.
    pub fn step(&mut self, x: &mut FakeTensor) -> Result<(), String> {
        if x.shape.len() != 2 || x.shape[0] != self.shapes.d_model {
            return Err(format!("transformer in shape {:?}", x.shape));
        }
        self.pos += x.shape[1];
        Ok(())
    }
    pub fn reset(&mut self) {
        self.pos = 0;
        self.kv.iter_mut().for_each(FakeTensor::fill_zero);
    }
}
