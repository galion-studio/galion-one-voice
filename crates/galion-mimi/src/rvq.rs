// Galion Voice — Perpetual Open MIT License
// SPDX-License-Identifier: MIT
//! Split residual vector quantizer — op inventory + shape-only stub.
//!
//! VERIFY against real config:
//! ```text
//! semantic RVQ : in_proj Conv1d(512→256,k=1) → 1 codebook  [2048 × 256] → out_proj(256→512)
//! acoustic RVQ : in_proj Conv1d(512→256,k=1) → N-1 codebooks [2048 × 256] residual → out_proj
//! latent_q = semantic_out + acoustic_out
//! ```
//! Encode op: per codebook, argmin_j ||r - C_j||² (GEMV against codebook + norms), r -= C_j.
//! Decode op: sum of codebook row gathers, then out_proj — decode is gather+add, cheap on CPU.
//! Codebooks are stored as EMA `embedding_sum / cluster_usage` in the upstream checkpoint;
//! the converter (A7) must pre-divide so `.gq` holds plain [size × dim] f32/int8 rows.

use crate::config::MimiConfig;
use crate::tensor::FakeTensor;

pub struct RvqStub {
    pub n_codebooks: usize,
    pub codebook_size: usize,
    pub codebook_dim: usize,
    pub latent_dim: usize,
}

impl RvqStub {
    pub fn new(cfg: &MimiConfig) -> Self {
        Self {
            n_codebooks: cfg.n_codebooks,
            codebook_size: cfg.codebook_size,
            codebook_dim: cfg.codebook_dim,
            latent_dim: cfg.latent_dim,
        }
    }
    /// latent: [latent_dim, steps] → codes: steps × n_codebooks (row-major by step). Stub writes 0.
    pub fn encode(&self, latent: &FakeTensor, codes: &mut [u16]) -> Result<(), String> {
        if latent.shape.len() != 2 || latent.shape[0] != self.latent_dim {
            return Err(format!("rvq in shape {:?}", latent.shape));
        }
        if codes.len() != latent.shape[1] * self.n_codebooks {
            return Err(format!("codes len {} != {}", codes.len(), latent.shape[1] * self.n_codebooks));
        }
        codes.iter_mut().for_each(|c| *c = 0);
        Ok(())
    }
    /// codes → latent [latent_dim, steps]. Validates code range; stub output is zeros.
    pub fn decode(&self, codes: &[u16], latent: &mut FakeTensor) -> Result<(), String> {
        if latent.shape.len() != 2 || latent.shape[0] != self.latent_dim || codes.len() != latent.shape[1] * self.n_codebooks {
            return Err(format!("rvq decode shapes codes={} latent={:?}", codes.len(), latent.shape));
        }
        if let Some(bad) = codes.iter().find(|&&c| c as usize >= self.codebook_size) {
            return Err(format!("code {bad} >= codebook_size {}", self.codebook_size));
        }
        latent.fill_zero();
        Ok(())
    }
}

/// Real split-RVQ kernels (encode = nearest-codeword search + residual; decode = gather + add +
/// k=1 out_proj). Weights come from a [`crate::weights::WeightSource`]; all scratch is allocated in
/// `new`, `encode_step`/`decode_step` do not allocate. One latent step (12.5 Hz) per call.
///
/// Semantic and acoustic quantizers both see the *same* projected latent (split RVQ, VERIFY);
/// projections are bias-free k=1 convs `[out, in, 1]` (VERIFY).
pub struct RvqKernels {
    d: usize,
    cd: usize,
    cs: usize,
    n_ac: usize,
    sem_in: Vec<f32>,
    sem_out: Vec<f32>,
    ac_in: Vec<f32>,
    ac_out: Vec<f32>,
    /// `[1 + n_ac, cs, cd]`: codebook 0 = semantic, then acoustic.
    cbs: Vec<f32>,
    /// `‖c‖²` per row, same layout as `cbs` rows.
    norms: Vec<f32>,
    r: Vec<f32>,
    q: Vec<f32>,
}

impl RvqKernels {
    pub fn new(cfg: &MimiConfig, src: &dyn crate::weights::WeightSource) -> Result<Self, crate::weights::LoadError> {
        use crate::weights::fetch;
        let (d, cd, cs, n_ac) = (cfg.latent_dim, cfg.codebook_dim, cfg.codebook_size, cfg.n_codebooks - 1);
        let f = |n: &str, s: &[usize]| fetch(src, n, s).map(|t| t.to_vec_f32());
        let mut cbs = f("rvq_semantic.codebook0", &[cs, cd])?;
        for i in 0..n_ac {
            cbs.extend(f(&format!("rvq_acoustic.codebook{i}"), &[cs, cd])?);
        }
        let norms = cbs.chunks_exact(cd).map(|row| row.iter().map(|v| v * v).sum()).collect();
        Ok(Self {
            d, cd, cs, n_ac,
            sem_in: f("rvq_semantic.in_proj.weight", &[cd, d, 1])?,
            sem_out: f("rvq_semantic.out_proj.weight", &[d, cd, 1])?,
            ac_in: f("rvq_acoustic.in_proj.weight", &[cd, d, 1])?,
            ac_out: f("rvq_acoustic.out_proj.weight", &[d, cd, 1])?,
            cbs, norms,
            r: vec![0.0; cd],
            q: vec![0.0; cd],
        })
    }

    fn matvec(w: &[f32], x: &[f32], out: &mut [f32], add: bool) {
        let n = x.len();
        for (o, row) in out.iter_mut().zip(w.chunks_exact(n)) {
            let v: f32 = row.iter().zip(x).map(|(a, b)| a * b).sum();
            *o = if add { *o + v } else { v };
        }
    }

    /// argmin_j ‖r − c_j‖² = argmin_j (‖c_j‖² − 2 c_j·r); ties → lowest index.
    fn nearest(&self, book: usize, r: &[f32]) -> usize {
        let base = book * self.cs;
        let mut best = (0usize, f32::INFINITY);
        for j in 0..self.cs {
            let row = &self.cbs[(base + j) * self.cd..(base + j + 1) * self.cd];
            let dot: f32 = row.iter().zip(r).map(|(a, b)| a * b).sum();
            let dist = self.norms[base + j] - 2.0 * dot;
            if dist < best.1 {
                best = (j, dist);
            }
        }
        best.0
    }

    fn row(&self, book: usize, j: usize) -> &[f32] {
        let i = book * self.cs + j;
        &self.cbs[i * self.cd..(i + 1) * self.cd]
    }

    /// `latent[d]` → `codes[1 + n_ac]`.
    pub fn encode_step(&mut self, latent: &[f32], codes: &mut [u16]) {
        let mut r = std::mem::take(&mut self.r);
        Self::matvec(&self.sem_in, latent, &mut r, false);
        codes[0] = self.nearest(0, &r) as u16;
        if self.n_ac > 0 {
            Self::matvec(&self.ac_in, latent, &mut r, false);
            for i in 0..self.n_ac {
                let j = self.nearest(1 + i, &r);
                codes[1 + i] = j as u16;
                let i0 = ((1 + i) * self.cs + j) * self.cd;
                for (k, v) in r.iter_mut().enumerate() {
                    *v -= self.cbs[i0 + k];
                }
            }
        }
        self.r = r;
    }

    /// `codes[1 + n_ac]` (range-checked by caller) → `latent[d]`.
    pub fn decode_step(&mut self, codes: &[u16], latent: &mut [f32]) {
        let mut q = std::mem::take(&mut self.q);
        q.copy_from_slice(self.row(0, codes[0] as usize));
        Self::matvec(&self.sem_out, &q, latent, false);
        if self.n_ac > 0 {
            q.iter_mut().for_each(|v| *v = 0.0);
            for i in 0..self.n_ac {
                let row = self.row(1 + i, codes[1 + i] as usize);
                q.iter_mut().zip(row).for_each(|(a, b)| *a += b);
            }
            Self::matvec(&self.ac_out, &q, latent, true);
        }
        self.q = q;
        debug_assert_eq!(latent.len(), self.d);
    }
}
