// Galion Voice — Perpetual Open MIT License
// SPDX-License-Identifier: MIT
//! Weight-loading **contract** for the real Mimi path. NO LOADER IS IMPLEMENTED and no weight
//! bytes exist in this repository (FINAL-BUILD-SPEC §5, S10). This module states what a
//! future loader must satisfy so A7 (`.gq`) and A21 (licence memo) can be checked against it,
//! and defines the [`WeightSource`] seam the real kernels (seanet/rvq/codec) read from.
//!
//! Wave 2: [`load_codec`] is the production entry point. It runs the licence/integrity gate and
//! then **refuses** (`NotImplementedStub`) until A21 clears the licence and A7's `.gq` reader is
//! wired to a [`WeightSource`]. [`ZeroWeights`] is the only built-in source: all-zero tensors
//! (silence), never a fabricated model.
//!
//! Contract for later (all MUST):
//! 1. Source: weights come only from a `.gq` file produced by the A7 converter from the
//!    upstream safetensors. Never fetched at runtime, never vendored in git, never a llama.cpp/GGML-family container.
//! 2. Licence gate: the `.gq` manifest names the licence (expected `CC-BY-4.0`, Kyutai Mimi)
//!    and the upstream URL; loading is refused if absent or if A21 has not cleared it.
//! 3. Integrity: manifest SHA-256 of the file and of each tensor; mismatch = hard error.
//! 4. Names/shapes: every entry of [`expected_tensors`] must be present with exactly that shape;
//!    unknown extra tensors are an error (no silent ignore).
//! 5. Folding at convert time: weight-norm (g, v) → plain weight; RVQ EMA codebooks
//!    `embedding_sum / cluster_usage` → plain rows. Runtime never sees g/v or EMA stats.
//! 6. Dtype: f32 reference; int8 per-row scales optional (A4). Scalar f32 is the oracle;
//!    a loaded model must pass a golden-frame test vs the upstream reference within tolerance
//!    before `IS_STUB` may flip to false.
//! 7. Memory: mmap + prefault (A7); no allocation in the frame loop after load.
//!
//! Upstream tensor *names* are intentionally not guessed here; A7's converter owns the
//! upstream-name → Galion-name map once the real checkpoint is inspected.

use crate::codec::MimiCodec;
use crate::config::MimiConfig;
use crate::seanet::{decoder_convs, encoder_convs, resample_convs};
use galion_tensor::Tensor;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TensorSpec {
    pub name: String,
    pub shape: Vec<usize>,
}

/// Everything a future loader must find (Galion-side names). Biases included for convs/linears.
pub fn expected_tensors(cfg: &MimiConfig) -> Vec<TensorSpec> {
    let mut v = Vec::new();
    let mut push = |name: String, shape: Vec<usize>| v.push(TensorSpec { name, shape });
    for c in encoder_convs(cfg).into_iter().chain(decoder_convs(cfg)).chain(resample_convs(cfg)) {
        push(format!("{}.weight", c.name), c.weight_shape().to_vec());
        if c.bias {
            push(format!("{}.bias", c.name), vec![c.out_ch]);
        }
    }
    let d = cfg.latent_dim;
    for tf in ["enc_transformer", "dec_transformer"] {
        for l in 0..cfg.transformer_layers {
            for (name, shape) in crate::transformer::layer_tensors(tf, l, d, cfg.transformer_ffn) {
                push(name, shape);
            }
        }
    }
    for (q, n) in [("rvq_semantic", 1usize), ("rvq_acoustic", cfg.n_codebooks - 1)] {
        push(format!("{q}.in_proj.weight"), vec![cfg.codebook_dim, d, 1]);
        push(format!("{q}.out_proj.weight"), vec![d, cfg.codebook_dim, 1]);
        for i in 0..n {
            push(format!("{q}.codebook{i}"), vec![cfg.codebook_size, cfg.codebook_dim]);
        }
    }
    v
}

/// Parameter count implied by the contract (sanity number for planning; not a real model size).
pub fn expected_param_count(cfg: &MimiConfig) -> usize {
    expected_tensors(cfg).iter().map(|t| t.shape.iter().product::<usize>()).sum()
}

/// Manifest fields a future `.gq` must carry for the licence/integrity gate.
#[derive(Clone, Debug, Default)]
pub struct WeightManifest {
    pub licence: Option<String>,
    pub upstream_url: Option<String>,
    pub file_sha256: Option<String>,
    pub licence_cleared_by_a21: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadError {
    /// Always returned today: this crate is a stub and has no loader.
    NotImplementedStub,
    LicenceMissing,
    LicenceNotCleared,
    IntegrityMissing,
    /// A [`WeightSource`] has no tensor of this name.
    Missing(String),
    /// A [`WeightSource`] returned a tensor whose shape differs from the contract.
    ShapeMismatch { name: String, want: Vec<usize>, got: Vec<usize> },
    /// A galion-tensor kernel refused the weights / frame geometry.
    Kernel(String),
}

/// Where the real kernels get their tensors. Shapes follow [`expected_tensors`] exactly.
/// Called only at construction time (allocation allowed there, never in the frame loop).
pub trait WeightSource {
    fn tensor(&self, name: &str, shape: &[usize]) -> Result<Tensor, LoadError>;
}

/// All-zero weights of the contract shapes. Produces silence / code 0. Not a model; it exists so
/// the real kernel path (and its output lengths) can be exercised before any weight is cleared.
pub struct ZeroWeights;

impl WeightSource for ZeroWeights {
    fn tensor(&self, _name: &str, shape: &[usize]) -> Result<Tensor, LoadError> {
        Ok(Tensor::zeros(shape.to_vec()))
    }
}

/// Fetch + shape-check one tensor (construction time only).
pub fn fetch(src: &dyn WeightSource, name: &str, shape: &[usize]) -> Result<Tensor, LoadError> {
    let t = src.tensor(name, shape)?;
    if t.dims() != shape {
        return Err(LoadError::ShapeMismatch { name: name.into(), want: shape.to_vec(), got: t.dims().to_vec() });
    }
    Ok(t)
}

/// Gate check a future loader must pass *before* reading any tensor bytes.
pub fn check_manifest(m: &WeightManifest) -> Result<(), LoadError> {
    if m.licence.as_deref().unwrap_or("").is_empty() || m.upstream_url.is_none() {
        return Err(LoadError::LicenceMissing);
    }
    if !m.licence_cleared_by_a21 {
        return Err(LoadError::LicenceNotCleared);
    }
    if m.file_sha256.as_deref().map_or(true, |s| s.len() != 64) {
        return Err(LoadError::IntegrityMissing);
    }
    Ok(())
}

/// Placeholder entry point. Even a fully valid manifest yields `NotImplementedStub`.
pub fn load(m: &WeightManifest) -> Result<(), LoadError> {
    check_manifest(m)?;
    Err(LoadError::NotImplementedStub)
}

/// Production entry point for a codec with real weights. Today it **always refuses**: the gate in
/// [`load`] runs first, then `NotImplementedStub` (A21 licence clearance + A7 `.gq` → [`WeightSource`]
/// adapter outstanding). When it lands, it will end in `MimiCodec::with_weight_source(cfg, &gq_source)`.
pub fn load_codec(cfg: &MimiConfig, m: &WeightManifest) -> Result<MimiCodec, LoadError> {
    cfg.validate().map_err(LoadError::Kernel)?;
    load(m)?;
    Err(LoadError::NotImplementedStub)
}
