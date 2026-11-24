// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Static int8 vectors.
//!
//! The hot path is a feature-hashing table: each token adds a signed count
//! into a fixed-width vector, the vector is L2-normalised, then stored as
//! symmetric int8 with one scale. That is the diary shape from HORBM R1
//! (one int8 vector per card, exact scan) without a Transformer and without
//! `ort` or `candle`. A neural table can replace [`HashingEmbedder`] later;
//! see `v2/docs/MEMORY_STACK.md`.

use crate::text::tokens;

pub const EMB_DIM: usize = 256;

#[derive(Clone, Debug)]
pub struct Int8Vec {
    pub values: Vec<i8>,
    pub scale: f32,
    pub norm: f32,
}

/// Feature hashing into [`EMB_DIM`] floats, then int8 quantization.
#[derive(Clone, Copy, Debug)]
pub struct HashingEmbedder;

impl HashingEmbedder {
    pub fn embed(&self, text: &str) -> Int8Vec {
        let mut acc = [0f32; EMB_DIM];
        for tok in tokens(text) {
            let h = fnv1a(tok.as_bytes());
            let i1 = (h as usize) % EMB_DIM;
            let i2 = ((h >> 8) as usize) % EMB_DIM;
            let s1 = if h & 1 == 0 { 1.0 } else { -1.0 };
            let s2 = if (h >> 1) & 1 == 0 { 1.0 } else { -1.0 };
            acc[i1] += s1;
            if i2 != i1 {
                acc[i2] += s2 * 0.5;
            }
        }
        let mut norm_sq = 0f32;
        for x in &acc {
            norm_sq += x * x;
        }
        let n = norm_sq.sqrt();
        if n > 0.0 {
            for x in &mut acc {
                *x /= n;
            }
        }
        quantize(&acc)
    }
}

fn quantize(v: &[f32]) -> Int8Vec {
    let mut max = 0f32;
    for x in v {
        max = max.max(x.abs());
    }
    let scale = if max == 0.0 { 1.0 } else { max / 127.0 };
    let mut values = Vec::with_capacity(v.len());
    let mut norm_sq = 0f32;
    for x in v {
        let q = (x / scale).round().clamp(-127.0, 127.0) as i8;
        values.push(q);
        let recon = q as f32 * scale;
        norm_sq += recon * recon;
    }
    Int8Vec {
        values,
        scale,
        norm: norm_sq.sqrt(),
    }
}

/// Cosine of two quantized vectors. Zero when either norm is zero.
pub fn cosine(q: &Int8Vec, values: &[i8], scale: f32, norm: f32) -> f32 {
    if q.norm == 0.0 || norm == 0.0 || q.values.len() != values.len() {
        return 0.0;
    }
    let mut dot: i32 = 0;
    for (a, b) in q.values.iter().zip(values.iter()) {
        dot += *a as i32 * *b as i32;
    }
    (dot as f32) * q.scale * scale / (q.norm * norm)
}

/// True when the int8 scan will use AVX2. Detected once per search.
pub fn avx2_available() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        std::is_x86_feature_detected!("avx2")
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        false
    }
}

pub fn simd_lane() -> &'static str {
    if avx2_available() {
        "avx2"
    } else {
        "scalar"
    }
}

/// Dot product of two 256-d int8 vectors.
pub fn dot_i8(q: &[i8; EMB_DIM], row: &[i8], avx2: bool) -> i32 {
    debug_assert_eq!(row.len(), EMB_DIM);
    #[cfg(target_arch = "x86_64")]
    if avx2 {
        // Safety: `avx2_available` checked the CPU, and `q` / `row` are 256 bytes.
        return unsafe { dot_avx2(q, row) };
    }
    dot_scalar(q, row)
}

fn dot_scalar(q: &[i8; EMB_DIM], row: &[i8]) -> i32 {
    let mut dot: i32 = 0;
    let mut k = 0;
    while k < EMB_DIM {
        dot += q[k] as i32 * row[k] as i32;
        dot += q[k + 1] as i32 * row[k + 1] as i32;
        dot += q[k + 2] as i32 * row[k + 2] as i32;
        dot += q[k + 3] as i32 * row[k + 3] as i32;
        k += 4;
    }
    dot
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn dot_avx2(q: &[i8; EMB_DIM], row: &[i8]) -> i32 {
    use std::arch::x86_64::{
        __m128i, __m256i, _mm256_add_epi32, _mm256_cvtepi8_epi16, _mm256_madd_epi16,
        _mm256_setzero_si256, _mm256_storeu_si256, _mm_loadu_si128,
    };
    let mut acc = _mm256_setzero_si256();
    let mut off = 0;
    while off < EMB_DIM {
        let qv = _mm_loadu_si128(q.as_ptr().add(off) as *const __m128i);
        let dv = _mm_loadu_si128(row.as_ptr().add(off) as *const __m128i);
        let prod = _mm256_madd_epi16(_mm256_cvtepi8_epi16(qv), _mm256_cvtepi8_epi16(dv));
        acc = _mm256_add_epi32(acc, prod);
        off += 16;
    }
    let mut tmp = [0i32; 8];
    _mm256_storeu_si256(tmp.as_mut_ptr() as *mut __m256i, acc);
    tmp.iter().sum()
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_text_is_near_one() {
        let emb = HashingEmbedder;
        let a = emb.embed("the great wall at beijing");
        let b = emb.embed("the great wall at beijing");
        let score = cosine(&a, &b.values, b.scale, b.norm);
        assert!(score > 0.98, "score {score}");
    }

    #[test]
    fn shared_tokens_beat_unrelated_text() {
        let emb = HashingEmbedder;
        let q = emb.embed("simhash neighbourhood merge");
        let near = emb.embed("a simhash neighbourhood of consented claims");
        let far = emb.embed("piper synthesizes a short clause of audio");
        let near_s = cosine(&q, &near.values, near.scale, near.norm);
        let far_s = cosine(&q, &far.values, far.scale, far.norm);
        assert!(near_s > far_s, "near {near_s} far {far_s}");
    }

    #[test]
    fn width_is_the_diary_width() {
        let v = HashingEmbedder.embed("width");
        assert_eq!(v.values.len(), EMB_DIM);
    }

    #[test]
    fn avx2_matches_scalar_when_present() {
        if !super::avx2_available() {
            return;
        }
        let emb = HashingEmbedder;
        let a = emb.embed("simhash neighbourhood of consented claims");
        let b = emb.embed("a different sentence about piper audio");
        let mut qa = [0i8; EMB_DIM];
        let mut qb = [0i8; EMB_DIM];
        qa.copy_from_slice(&a.values);
        qb.copy_from_slice(&b.values);
        assert_eq!(
            super::dot_i8(&qa, &b.values, false),
            super::dot_i8(&qa, &b.values, true)
        );
        assert_eq!(
            super::dot_i8(&qa, &a.values, false),
            super::dot_i8(&qa, &a.values, true)
        );
        assert_eq!(
            super::dot_i8(&qb, &a.values, false),
            super::dot_i8(&qb, &a.values, true)
        );
    }
}
