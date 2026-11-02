// Galion Voice — Perpetual Open MIT License
// SPDX-License-Identifier: MIT
//! Mimi hyper-parameters. Shape-only; no values here are learned weights.
//!
//! `VERIFY`: taken from Kyutai's public Mimi/Moshi description; must be re-checked against
//! the config file that accompanies the real CC-BY weights before the real path lands.

/// Locked heartbeat (LOCKS.toml / FINAL-BUILD-SPEC §1): one frame = 80 ms.
pub const FRAME_MS: u32 = 80;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MimiConfig {
    /// Audio sample rate in Hz. VERIFY: 24_000.
    pub sample_rate: u32,
    /// Audio channels (mono).
    pub channels: usize,
    /// SEANet base filter count. VERIFY: 64.
    pub seanet_filters: usize,
    /// SEANet encoder stride ratios (applied in this order on the encoder; reversed on decoder).
    /// VERIFY: [4, 5, 6, 8] → hop 960 → 25 Hz.
    pub seanet_ratios: [usize; 4],
    /// Extra stride-2 conv after SEANet (25 Hz → 12.5 Hz). VERIFY: 2.
    pub frame_downsample: usize,
    /// Latent (transformer d_model) dimension. VERIFY: 512.
    pub latent_dim: usize,
    /// Transformer layers. VERIFY: 8.
    pub transformer_layers: usize,
    /// Attention heads. VERIFY: 8.
    pub transformer_heads: usize,
    /// MLP hidden size. VERIFY: 2048.
    pub transformer_ffn: usize,
    /// Causal attention context in frames (KV ring capacity). VERIFY: 250.
    pub transformer_context: usize,
    /// Codebooks used per frame (1 semantic + rest acoustic). Moshi uses 8 of Mimi's 32. VERIFY.
    pub n_codebooks: usize,
    /// Entries per codebook. VERIFY: 2048.
    pub codebook_size: usize,
    /// Codebook vector dimension (after input projection). VERIFY: 256.
    pub codebook_dim: usize,
}

impl Default for MimiConfig {
    fn default() -> Self {
        Self {
            sample_rate: 24_000,
            channels: 1,
            seanet_filters: 64,
            seanet_ratios: [4, 5, 6, 8],
            frame_downsample: 2,
            latent_dim: 512,
            transformer_layers: 8,
            transformer_heads: 8,
            transformer_ffn: 2048,
            transformer_context: 250,
            n_codebooks: 8,
            codebook_size: 2048,
            codebook_dim: 256,
        }
    }
}

impl MimiConfig {
    /// Total SEANet hop in samples (product of ratios).
    pub fn seanet_hop(&self) -> usize {
        self.seanet_ratios.iter().product()
    }
    /// Samples per codec frame (SEANet hop × downsample).
    pub fn frame_samples(&self) -> usize {
        self.seanet_hop() * self.frame_downsample
    }
    /// Codec frame rate in milli-Hz (12_500 = 12.5 Hz).
    pub fn frame_rate_mhz(&self) -> u32 {
        (self.sample_rate as u64 * 1000 / self.frame_samples() as u64) as u32
    }
    /// Frame duration in ms derived from the shapes (must equal [`FRAME_MS`]).
    pub fn frame_ms(&self) -> u32 {
        (self.frame_samples() as u64 * 1000 / self.sample_rate as u64) as u32
    }
    /// Check the shape invariants the rest of the stack relies on.
    pub fn validate(&self) -> Result<(), String> {
        if self.frame_ms() != FRAME_MS || self.frame_samples() * 1000 != self.sample_rate as usize * FRAME_MS as usize {
            return Err(format!("frame is {} samples = {} ms, locked heartbeat is {FRAME_MS} ms", self.frame_samples(), self.frame_ms()));
        }
        if self.latent_dim % self.transformer_heads != 0 {
            return Err("latent_dim must divide by heads".into());
        }
        if self.n_codebooks == 0 || self.codebook_size == 0 || self.codebook_size > u16::MAX as usize + 1 {
            return Err("codebooks must be 1.. and codebook_size must fit u16".into());
        }
        Ok(())
    }
}
