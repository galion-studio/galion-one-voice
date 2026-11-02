// Galion Voice — Perpetual Open MIT License
// SPDX-License-Identifier: MIT
//! Causal multi-head / grouped-query attention with RoPE-NeoX and a [`KvRing`] cache.
//!
//! * [`attention_reference`]: whole-sequence scalar reference built on [`crate::ops::rope_neox`]
//!   (allocating; ground truth for tests).
//! * [`StreamingAttention`]: one token per [`StreamingAttention::step`]; all scratch (RoPE
//!   frequencies, scores, rotated q/k, KV ring) allocated once in `new` — **zero heap allocation
//!   per frame**. Output is **bit-identical** to `attention_reference` (same RoPE formula, same
//!   softmax as [`crate::ops::softmax`], same chronological accumulation order). When the ring is
//!   full the window slides: token `t` attends to `max(0, t-capacity+1) ..= t`.
//!
//! Layout: q `[n_heads, head_dim]`, k/v `[n_kv_heads, head_dim]` per token; `n_heads % n_kv_heads == 0`
//! (GQA: query head `h` reads kv head `h / (n_heads / n_kv_heads)`).

use crate::kv::KvRing;
use crate::ops::rope_neox;
use crate::{Result, Tensor, TensorError};

/// Precomputed NeoX rotate-half frequencies. Bit-identical to [`rope_neox`] (same expressions).
#[derive(Debug, Clone)]
pub struct RopeNeox {
    head_dim: usize,
    inv_freq: Vec<f32>,
}

impl RopeNeox {
    pub fn new(head_dim: usize, theta: f32) -> Result<Self> {
        if head_dim == 0 || head_dim % 2 != 0 {
            return Err(TensorError::InvalidShape(format!("rope: head_dim must be even, got {head_dim}")));
        }
        let inv_freq = (0..head_dim / 2).map(|i| theta.powf(-2.0 * i as f32 / head_dim as f32)).collect();
        Ok(Self { head_dim, inv_freq })
    }

    /// Rotate one token in place. `x`: `[heads, head_dim]`. No allocation.
    pub fn apply(&self, x: &mut [f32], heads: usize, pos: usize) {
        let (hd, half) = (self.head_dim, self.head_dim / 2);
        debug_assert_eq!(x.len(), heads * hd);
        let p = pos as f32;
        for (i, &freq) in self.inv_freq.iter().enumerate() {
            let (s, c) = (p * freq).sin_cos();
            for h in 0..heads {
                let base = h * hd;
                let (a, b) = (x[base + i], x[base + i + half]);
                x[base + i] = a * c - b * s;
                x[base + i + half] = a * s + b * c;
            }
        }
    }
}

/// Same arithmetic as `ops::softmax` on one row (kept identical for bit-exactness).
#[inline]
fn softmax_inplace(r: &mut [f32]) {
    let m = r.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let mut s = 0.0;
    for e in r.iter_mut() { *e = (*e - m).exp(); s += *e; }
    let inv = 1.0 / s;
    r.iter_mut().for_each(|e| *e *= inv);
}

#[inline]
fn dot(a: &[f32], b: &[f32]) -> f32 {
    let mut acc = 0.0f32;
    for (x, y) in a.iter().zip(b) { acc += x * y; }
    acc
}

fn check_heads(n_heads: usize, n_kv_heads: usize) -> Result<()> {
    if n_heads == 0 || n_kv_heads == 0 || n_heads % n_kv_heads != 0 {
        return Err(TensorError::InvalidShape(format!("attention: n_heads {n_heads} must be a multiple of n_kv_heads {n_kv_heads}")));
    }
    Ok(())
}

/// Whole-sequence causal attention reference. q `[seq, n_heads, hd]`, k/v `[seq, n_kv_heads, hd]`.
/// RoPE-NeoX applied to q and k at positions `pos0 + t`. `window`: `None` = full causal,
/// `Some(w)` = each token sees at most the last `w` tokens (itself included). Returns `[seq, n_heads, hd]`.
pub fn attention_reference(q: &Tensor, k: &Tensor, v: &Tensor, pos0: usize, theta: f32, window: Option<usize>) -> Result<Tensor> {
    if q.shape().rank() != 3 || k.dims() != v.dims() || k.shape().rank() != 3 || q.dims()[0] != k.dims()[0] || q.dims()[2] != k.dims()[2] {
        return Err(TensorError::ShapeMismatch { op: "attention", lhs: q.dims().to_vec(), rhs: k.dims().to_vec() });
    }
    let (seq, nh, hd) = (q.dims()[0], q.dims()[1], q.dims()[2]);
    let nkv = k.dims()[1];
    check_heads(nh, nkv)?;
    if window == Some(0) { return Err(TensorError::InvalidShape("attention: window must be > 0".into())); }
    let qr = rope_neox(q, pos0, theta)?.to_vec_f32();
    let kr = rope_neox(k, pos0, theta)?.to_vec_f32();
    let vv = v.to_vec_f32();
    let scale = 1.0 / (hd as f32).sqrt();
    let rep = nh / nkv;
    let mut out = vec![0f32; seq * nh * hd];
    let mut scores = Vec::with_capacity(seq);
    for t in 0..seq {
        let lo = window.map_or(0, |w| (t + 1).saturating_sub(w));
        for h in 0..nh {
            let kh = h / rep;
            let qv = &qr[(t * nh + h) * hd..][..hd];
            scores.clear();
            for j in lo..=t {
                scores.push(dot(qv, &kr[(j * nkv + kh) * hd..][..hd]) * scale);
            }
            softmax_inplace(&mut scores);
            let o = &mut out[(t * nh + h) * hd..][..hd];
            for (idx, j) in (lo..=t).enumerate() {
                let p = scores[idx];
                let vj = &vv[(j * nkv + kh) * hd..][..hd];
                for d in 0..hd { o[d] += p * vj[d]; }
            }
        }
    }
    Tensor::from_vec(out, [seq, nh, hd])
}

/// Streaming one-token-per-step causal attention over a preallocated [`KvRing`].
#[derive(Debug, Clone)]
pub struct StreamingAttention {
    n_heads: usize,
    n_kv_heads: usize,
    head_dim: usize,
    scale: f32,
    pos0: usize,
    rope: RopeNeox,
    kv: KvRing,
    q_buf: Vec<f32>,
    k_buf: Vec<f32>,
    scores: Vec<f32>,
}

impl StreamingAttention {
    /// `capacity`: KV ring slots (= attention window). `pos0`: absolute position of the first token.
    pub fn new(n_heads: usize, n_kv_heads: usize, head_dim: usize, capacity: usize, theta: f32, pos0: usize) -> Result<Self> {
        check_heads(n_heads, n_kv_heads)?;
        let rope = RopeNeox::new(head_dim, theta)?;
        let kv = KvRing::new(capacity, n_kv_heads, head_dim)?;
        Ok(Self {
            n_heads, n_kv_heads, head_dim, scale: 1.0 / (head_dim as f32).sqrt(), pos0, rope, kv,
            q_buf: vec![0.0; n_heads * head_dim],
            k_buf: vec![0.0; n_kv_heads * head_dim],
            scores: vec![0.0; capacity],
        })
    }

    pub fn kv(&self) -> &KvRing { &self.kv }
    /// Absolute RoPE position the next token will use.
    pub fn position(&self) -> usize { self.pos0 + self.kv.next_pos() }
    /// New utterance: clear the cache (no allocation). Positions restart at `pos0`.
    pub fn reset(&mut self) { self.kv.clear(); }

    /// One token. q `[n_heads, hd]`, k/v `[n_kv_heads, hd]` (pre-RoPE), out `[n_heads, hd]`. No allocation.
    pub fn step(&mut self, q: &[f32], k: &[f32], v: &[f32], out: &mut [f32]) -> Result<()> {
        let (nh, nkv, hd) = (self.n_heads, self.n_kv_heads, self.head_dim);
        if q.len() != nh * hd || k.len() != nkv * hd || v.len() != nkv * hd || out.len() != nh * hd {
            return Err(TensorError::InvalidShape("attention step: q/out [n_heads,hd], k/v [n_kv_heads,hd]".into()));
        }
        let pos = self.position();
        self.q_buf.copy_from_slice(q);
        self.k_buf.copy_from_slice(k);
        self.rope.apply(&mut self.q_buf, nh, pos);
        self.rope.apply(&mut self.k_buf, nkv, pos);
        self.kv.push(&self.k_buf, v)?;
        let n = self.kv.len();
        let rep = nh / nkv;
        out.iter_mut().for_each(|e| *e = 0.0);
        for h in 0..nh {
            let kh = h / rep;
            let qv = &self.q_buf[h * hd..(h + 1) * hd];
            let scores = &mut self.scores[..n];
            for (j, s) in scores.iter_mut().enumerate() {
                *s = dot(qv, self.kv.key(j, kh)) * self.scale;
            }
            softmax_inplace(scores);
            let o = &mut out[h * hd..(h + 1) * hd];
            for (j, &p) in scores.iter().enumerate() {
                let vj = self.kv.value(j, kh);
                for d in 0..hd { o[d] += p * vj[d]; }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::softmax;
    use crate::Exec;

    fn lcg(n: usize, seed: u32) -> Vec<f32> {
        let mut s = seed;
        (0..n).map(|_| { s = s.wrapping_mul(1664525).wrapping_add(1013904223); ((s >> 8) as f32 / (1u32 << 24) as f32) * 2.0 - 1.0 }).collect()
    }
    fn bits(v: &[f32]) -> Vec<u32> { v.iter().map(|f| f.to_bits()).collect() }

    #[test]
    fn rope_cache_bit_exact_vs_rope_neox() {
        let (seq, h, hd) = (5, 3, 8);
        let x = lcg(seq * h * hd, 11);
        let want = rope_neox(&Tensor::from_vec(x.clone(), [seq, h, hd]).unwrap(), 17, 10_000.0).unwrap().to_vec_f32();
        let r = RopeNeox::new(hd, 10_000.0).unwrap();
        let mut got = x;
        for t in 0..seq { r.apply(&mut got[t * h * hd..(t + 1) * h * hd], h, 17 + t); }
        assert_eq!(bits(&got), bits(&want));
        assert!(RopeNeox::new(3, 1e4).is_err());
    }

    #[test]
    fn softmax_matches_ops_softmax_bits() {
        let x = lcg(13, 5);
        let want = softmax(&Tensor::from_vec(x.clone(), [13]).unwrap(), Exec::Scalar).unwrap().to_vec_f32();
        let mut got = x;
        softmax_inplace(&mut got);
        assert_eq!(bits(&got), bits(&want));
    }

    fn stream_vs_ref(nh: usize, nkv: usize, hd: usize, seq: usize, cap: usize, pos0: usize) {
        let q = lcg(seq * nh * hd, 1);
        let k = lcg(seq * nkv * hd, 2);
        let v = lcg(seq * nkv * hd, 3);
        let window = (cap < seq).then_some(cap);
        let want = attention_reference(
            &Tensor::from_vec(q.clone(), [seq, nh, hd]).unwrap(),
            &Tensor::from_vec(k.clone(), [seq, nkv, hd]).unwrap(),
            &Tensor::from_vec(v.clone(), [seq, nkv, hd]).unwrap(),
            pos0, 10_000.0, window,
        ).unwrap().to_vec_f32();
        let mut a = StreamingAttention::new(nh, nkv, hd, cap, 10_000.0, pos0).unwrap();
        let mut got = vec![0f32; seq * nh * hd];
        let (qs, ks) = (nh * hd, nkv * hd);
        for t in 0..seq {
            a.step(&q[t * qs..][..qs], &k[t * ks..][..ks], &v[t * ks..][..ks], &mut got[t * qs..][..qs]).unwrap();
        }
        assert_eq!(bits(&got), bits(&want), "nh={nh} nkv={nkv} hd={hd} seq={seq} cap={cap}");
        assert_eq!(a.position(), pos0 + seq);
    }

    #[test]
    fn streaming_bit_exact_full_causal() {
        stream_vs_ref(1, 1, 4, 6, 6, 0);
        stream_vs_ref(4, 4, 8, 10, 16, 0);
        stream_vs_ref(4, 2, 8, 9, 9, 3); // GQA + pos offset
    }

    #[test]
    fn streaming_bit_exact_sliding_window() {
        stream_vs_ref(2, 1, 4, 20, 5, 0);
        stream_vs_ref(8, 2, 16, 40, 7, 100);
        stream_vs_ref(1, 1, 2, 3, 1, 0); // window 1 => out == v of current token
    }

    #[test]
    fn first_token_returns_its_value_and_reset() {
        let mut a = StreamingAttention::new(2, 1, 4, 4, 1e4, 0).unwrap();
        let (q, k, v) = (lcg(8, 1), lcg(4, 2), lcg(4, 3));
        let mut o = vec![0f32; 8];
        a.step(&q, &k, &v, &mut o).unwrap();
        assert_eq!(&o[..4], &v[..]);
        assert_eq!(&o[4..], &v[..]);
        a.step(&q, &k, &v, &mut o).unwrap();
        a.reset();
        assert_eq!((a.position(), a.kv().len()), (0, 0));
        assert!(a.step(&q[..4], &k, &v, &mut o).is_err());
        assert!(StreamingAttention::new(3, 2, 4, 4, 1e4, 0).is_err());
    }
}
