// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! W21-02 · Mimi passthrough lane (sector 01 / A6) for the live demo.
//!
//! One call = one 80 ms heartbeat frame:
//! ```text
//! pcm_in[1920] → MimiCodec::encode_frame → codes[n_codebooks]
//!              → MimiCodec::decode_frame → pcm_out[1920]   (sanitised: finite, clamped to [-1, 1])
//! ```
//!
//! **SYNTHETIC / STUB.** `galion_mimi::IS_STUB` is `true` and stays `true`: there are no Mimi
//! weights in this repository. The lane only ever builds the codec with
//! * [`KernelMode::ZeroWeights`] — every real streaming kernel (SEANet enc/dec, 25⇄12.5 Hz
//!   resample, both streaming transformers, split RVQ) runs on all-zero tensors, or
//! * [`KernelMode::NoKernels`] — shape-only path, no kernel compute.
//!
//! Either way the codes are all 0 and the decoded audio is silence. That is the honest output of
//! the real path with zero weights; it is **not** Mimi audio and must never be presented as such.
//! "Tone-safe": a test tone (or any input) goes in, and the output is guaranteed finite and inside
//! [-1, 1], so the lane can sit in a live audio path without risk to speakers/ears.
//! [`Monitor::DryBypass`] lets the demo stay audible by copying the input to the output *after* the
//! codec has run (timing is still real); every [`StepReport`] says so via `bypassed`.
//!
//! Hot path: [`MimiPassthrough::step_frame`] performs **zero heap allocations** (all buffers are
//! built in [`MimiPassthrough::new`]; checked by `tests/mimi_zero_alloc.rs`), no locks, no I/O, no
//! clock reads. Errors (wrong buffer sizes) are the only allocating path.
//!
//! Latency (W20-15): the full-size config (512-d, 8 layers, ffn 2048, 64 SEANet filters) on the
//! scalar kernel path is ~1.3 s per frame enc+dec — far from real time. Use
//! [`MimiLaneConfig::demo`] (reduced widths, same 1920-sample / 80 ms frame) for the live demo.
//! Measured numbers: `docs/agent-wave21/W21-02-STATUS.md` and
//! `cargo run --release -p galion-student-backend --example mimi_passthrough`.

use galion_mimi::weights::ZeroWeights;
use galion_mimi::{MimiCodec, MimiConfig};

/// Samples in one 80 ms frame at 24 kHz (== `galion_heartbeat::FRAME_SAMPLES`).
pub const FRAME_SAMPLES: usize = 1920;
/// Re-export of the codec's stub flag. Always `true` until weights + golden frames land.
pub const IS_STUB: bool = galion_mimi::IS_STUB;

/// Which codec build the lane uses. There is deliberately no variant that loads real weights:
/// that path is `galion_mimi::weights::load_codec`, which refuses until A21 clears the licence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KernelMode {
    /// Shape-only path: no kernel compute, codes 0, silence. Cheapest.
    NoKernels,
    /// Real streaming kernels on all-zero tensors (exercises real compute/timing). Codes 0, silence.
    ZeroWeights,
}

/// What `pcm_out` carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Monitor {
    /// Decoded codec output (silence today). Default.
    Codec,
    /// Codec runs, then `pcm_out` = sanitised copy of `pcm_in`. For an audible demo only;
    /// [`StepReport::bypassed`] is set so the UI can label it "bypass, not Mimi".
    DryBypass,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MimiLaneConfig {
    pub mimi: MimiConfig,
    pub kernels: KernelMode,
    pub monitor: Monitor,
}

impl MimiLaneConfig {
    /// Reduced widths for interactive speed. Same SEANet ratios → 1920 samples / 80 ms / 12.5 Hz.
    /// Hyper-parameters are demo-only and are not Mimi's.
    pub fn demo_mimi() -> MimiConfig {
        MimiConfig {
            seanet_filters: 8,
            latent_dim: 64,
            transformer_layers: 2,
            transformer_heads: 4,
            transformer_ffn: 256,
            transformer_context: 32,
            n_codebooks: 8,
            codebook_size: 256,
            codebook_dim: 32,
            ..MimiConfig::default()
        }
    }
    /// Live-demo default: reduced widths, real kernels on ZeroWeights, codec monitor.
    pub fn demo() -> Self {
        Self { mimi: Self::demo_mimi(), kernels: KernelMode::ZeroWeights, monitor: Monitor::Codec }
    }
    /// Full Mimi shapes (VERIFY values) on ZeroWeights. Not real time on the scalar path.
    pub fn full() -> Self {
        Self { mimi: MimiConfig::default(), kernels: KernelMode::ZeroWeights, monitor: Monitor::Codec }
    }
    /// Full shapes, no kernel compute. Real-time safe fallback.
    pub fn no_kernels() -> Self {
        Self { mimi: MimiConfig::default(), kernels: KernelMode::NoKernels, monitor: Monitor::Codec }
    }
    pub fn with_monitor(mut self, monitor: Monitor) -> Self {
        self.monitor = monitor;
        self
    }
}

impl Default for MimiLaneConfig {
    fn default() -> Self {
        Self::demo()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum MimiLaneError {
    /// Codec construction failed (config invalid / kernel refused shapes).
    Build(String),
    /// A caller buffer has the wrong length.
    Shape { what: &'static str, got: usize, want: usize },
    /// The codec rejected a frame.
    Codec(String),
}

impl std::fmt::Display for MimiLaneError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MimiLaneError::Build(s) => write!(f, "mimi lane build: {s}"),
            MimiLaneError::Shape { what, got, want } => write!(f, "mimi lane: {what} has {got} samples, want {want}"),
            MimiLaneError::Codec(s) => write!(f, "mimi lane codec: {s}"),
        }
    }
}
impl std::error::Error for MimiLaneError {}

/// Per-frame result. `Copy`, no allocation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StepReport {
    /// 0-based count of frames that went through the codec since construction/reset.
    pub frame: u64,
    /// Input was not exactly [`FRAME_SAMPLES`] (late/short frame): codec not stepped,
    /// output is silence. A late frame is a pause, never a glitch.
    pub paused: bool,
    /// The codec (encode + decode) ran this frame.
    pub codec_ran: bool,
    /// `pcm_out` is a dry copy of the input ([`Monitor::DryBypass`]), not codec output.
    pub bypassed: bool,
    /// Any code this frame was non-zero (always false on ZeroWeights / NoKernels).
    pub codes_nonzero: bool,
    /// Peak |sample| of the codec's decoded output (before any bypass).
    pub codec_peak: f32,
    /// Samples replaced (non-finite → 0) or clamped to [-1, 1] in `pcm_out`.
    pub sanitized: u32,
    /// Always `true` today (mirror of `galion_mimi::IS_STUB`). UI must show "synthetic".
    pub is_stub: bool,
}

/// PCM → Mimi encode → codes → Mimi decode → PCM, one 80 ms frame per call.
pub struct MimiPassthrough {
    cfg: MimiLaneConfig,
    codec: MimiCodec,
    codes: Vec<u16>,
    frame: u64,
}

impl std::fmt::Debug for MimiPassthrough {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MimiPassthrough")
            .field("kernels", &self.cfg.kernels)
            .field("monitor", &self.cfg.monitor)
            .field("latent_dim", &self.cfg.mimi.latent_dim)
            .field("frame", &self.frame)
            .field("is_stub", &IS_STUB)
            .finish()
    }
}

impl MimiPassthrough {
    /// Build the lane. All allocation happens here.
    pub fn new(cfg: MimiLaneConfig) -> Result<Self, MimiLaneError> {
        let codec = match cfg.kernels {
            KernelMode::NoKernels => MimiCodec::new(cfg.mimi.clone()).map_err(|e| MimiLaneError::Build(e.to_string()))?,
            KernelMode::ZeroWeights => {
                MimiCodec::with_weight_source(cfg.mimi.clone(), &ZeroWeights).map_err(|e| MimiLaneError::Build(format!("{e:?}")))?
            }
        };
        if codec.frame_samples() != FRAME_SAMPLES {
            return Err(MimiLaneError::Build(format!("codec frame {} != {FRAME_SAMPLES}", codec.frame_samples())));
        }
        let codes = vec![0u16; codec.codes_per_frame()];
        Ok(Self { cfg, codec, codes, frame: 0 })
    }

    /// Live-demo default ([`MimiLaneConfig::demo`]).
    pub fn demo() -> Result<Self, MimiLaneError> {
        Self::new(MimiLaneConfig::demo())
    }

    pub fn config(&self) -> &MimiLaneConfig {
        &self.cfg
    }
    pub fn frame_samples(&self) -> usize {
        FRAME_SAMPLES
    }
    pub fn codes_per_frame(&self) -> usize {
        self.codes.len()
    }
    /// Codes produced by the most recent [`step_frame`](Self::step_frame).
    pub fn last_codes(&self) -> &[u16] {
        &self.codes
    }
    pub fn frames(&self) -> u64 {
        self.frame
    }
    pub fn is_stub(&self) -> bool {
        IS_STUB
    }
    /// Switch monitor between frames (not a codec rebuild; no allocation).
    pub fn set_monitor(&mut self, m: Monitor) {
        self.cfg.monitor = m;
    }

    /// Advance exactly one 80 ms frame. Zero allocation on success.
    ///
    /// * `pcm_out.len()` must be [`FRAME_SAMPLES`] (else `Err(Shape)`).
    /// * `pcm_in.len() != FRAME_SAMPLES` → paused frame: output silence, codec not stepped, `Ok`.
    pub fn step_frame(&mut self, pcm_in: &[f32], pcm_out: &mut [f32]) -> Result<StepReport, MimiLaneError> {
        if pcm_out.len() != FRAME_SAMPLES {
            return Err(MimiLaneError::Shape { what: "pcm_out", got: pcm_out.len(), want: FRAME_SAMPLES });
        }
        let mut rep = StepReport {
            frame: self.frame,
            paused: false,
            codec_ran: false,
            bypassed: false,
            codes_nonzero: false,
            codec_peak: 0.0,
            sanitized: 0,
            is_stub: IS_STUB,
        };
        if pcm_in.len() != FRAME_SAMPLES {
            pcm_out.iter_mut().for_each(|s| *s = 0.0);
            rep.paused = true;
            return Ok(rep);
        }
        self.codec.encode_frame(pcm_in, &mut self.codes).map_err(|e| MimiLaneError::Codec(e.to_string()))?;
        self.codec.decode_frame(&self.codes, pcm_out).map_err(|e| MimiLaneError::Codec(e.to_string()))?;
        rep.codec_ran = true;
        rep.codes_nonzero = self.codes.iter().any(|&c| c != 0);
        rep.codec_peak = pcm_out.iter().fold(0.0f32, |m, s| if s.is_finite() { m.max(s.abs()) } else { m });
        if self.cfg.monitor == Monitor::DryBypass {
            pcm_out.copy_from_slice(pcm_in);
            rep.bypassed = true;
        }
        rep.sanitized = sanitize(pcm_out);
        self.frame += 1;
        Ok(rep)
    }

    /// Fresh stream (conv histories, KV rings, counters). Not a hot-path call; no allocation.
    pub fn reset(&mut self) {
        self.codec.reset();
        self.codes.iter_mut().for_each(|c| *c = 0);
        self.frame = 0;
    }
}

/// Non-finite → 0, clamp to [-1, 1]. Returns how many samples were touched.
fn sanitize(pcm: &mut [f32]) -> u32 {
    let mut n = 0u32;
    for s in pcm.iter_mut() {
        if !s.is_finite() {
            *s = 0.0;
            n += 1;
        } else if *s > 1.0 || *s < -1.0 {
            *s = s.clamp(-1.0, 1.0);
            n += 1;
        }
    }
    n
}

/// 440 Hz test tone at amplitude `amp`, frame `f` (phase-continuous). Test/demo helper; allocates.
pub fn tone_frame(f: u64, amp: f32) -> Vec<f32> {
    let base = f as usize * FRAME_SAMPLES;
    (0..FRAME_SAMPLES).map(|i| amp * ((base + i) as f32 * 440.0 * std::f32::consts::TAU / 24_000.0).sin()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_clamps_and_zeroes() {
        let mut v = [0.5, 2.0, -3.0, f32::NAN, f32::INFINITY, -0.25];
        assert_eq!(sanitize(&mut v), 4);
        assert_eq!(v, [0.5, 1.0, -1.0, 0.0, 0.0, -0.25]);
    }

    #[test]
    fn demo_config_keeps_80ms_frame() {
        let c = MimiLaneConfig::demo_mimi();
        c.validate().unwrap();
        assert_eq!(c.frame_samples(), FRAME_SAMPLES);
        assert_eq!(c.frame_ms(), 80);
    }
}
