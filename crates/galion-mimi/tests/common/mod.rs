// Galion Voice — Perpetual Open MIT License
// SPDX-License-Identifier: MIT
//! Test-only helpers. `SyntheticWeights` are deterministic pseudo-random tensors generated in
//! memory from the tensor *name*; they are never saved, never shipped and are not Mimi weights
//! (A06 plan §5 step 1: "random synthetic weights generated in-test").
#![allow(dead_code)]

use galion_mimi::weights::{LoadError, WeightSource};
use galion_mimi::MimiConfig;
use galion_tensor::Tensor;

/// Small widths, full Mimi ratios → still exactly 1920 samples / 80 ms per frame.
pub fn tiny_cfg() -> MimiConfig {
    MimiConfig {
        seanet_filters: 4,
        latent_dim: 16,
        transformer_layers: 1,
        transformer_heads: 2,
        transformer_ffn: 32,
        transformer_context: 8,
        n_codebooks: 4,
        codebook_size: 32,
        codebook_dim: 8,
        ..MimiConfig::default()
    }
}

pub struct SyntheticWeights;

fn hash(s: &str) -> u64 {
    s.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3))
}

impl WeightSource for SyntheticWeights {
    fn tensor(&self, name: &str, shape: &[usize]) -> Result<Tensor, LoadError> {
        let n: usize = shape.iter().product();
        let fan_in: usize = shape.iter().skip(1).product::<usize>().max(1);
        let scale = if name.ends_with(".bias") { 0.1 } else if name.contains("codebook") { 1.0 } else { 1.5 / (fan_in as f32).sqrt() };
        let mut x = hash(name) | 1;
        let data = (0..n)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                ((x >> 40) as f32 / (1u64 << 24) as f32 * 2.0 - 1.0) * scale
            })
            .collect();
        Ok(Tensor::from_vec(data, shape.to_vec()).unwrap())
    }
}

pub fn test_pcm(samples: usize) -> Vec<f32> {
    let mut x = 0x9e37_79b9_7f4a_7c15u64;
    (0..samples)
        .map(|i| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            let noise = (x >> 40) as f32 / (1u64 << 24) as f32 - 0.5;
            0.5 * (i as f32 * 440.0 * std::f32::consts::TAU / 24_000.0).sin() + 0.1 * noise
        })
        .collect()
}

/// Place a per-frame `[c, n]` block at frame index `f` of a whole `[c, n*frames]` buffer.
pub fn put_frame(dst: &mut [f32], frame: &[f32], c: usize, n: usize, f: usize, frames: usize) {
    for ch in 0..c {
        dst[ch * n * frames + f * n..ch * n * frames + (f + 1) * n].copy_from_slice(&frame[ch * n..(ch + 1) * n]);
    }
}

/// Extract frame `f` (`[c, n]`) from a whole `[c, n*frames]` buffer.
pub fn get_frame(src: &[f32], c: usize, n: usize, f: usize, frames: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(c * n);
    for ch in 0..c {
        out.extend_from_slice(&src[ch * n * frames + f * n..ch * n * frames + (f + 1) * n]);
    }
    out
}

pub fn bits(v: &[f32]) -> Vec<u32> {
    v.iter().map(|x| x.to_bits()).collect()
}
