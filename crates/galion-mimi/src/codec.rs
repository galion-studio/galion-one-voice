// Galion Voice — Perpetual Open MIT License
// SPDX-License-Identifier: MIT
//! 80 ms frame API. One call = one heartbeat: 1920 PCM samples @ 24 kHz ⇄ `n_codebooks` codes.
//!
//! Pipeline per frame (default config; order follows upstream Mimi — transformers run at 25 Hz):
//! ```text
//! encode: pcm[1920] → SEANet enc → [512, 2] (25 Hz) → transformer(enc) → downsample conv k4 s2
//!         → [512, 1] (12.5 Hz) → split RVQ → codes[8]
//! decode: codes[8] → RVQ decode → [512, 1] → depthwise upsample convT k4 s2 → [512, 2]
//!         → transformer(dec) → SEANet dec → pcm[1920]
//! ```
//! Two modes, same API:
//! * **No weights** ([`MimiCodec::new`], the default): kernels are not built; codes = 0, audio =
//!   silence. Cheap, and exactly what the real path computes with all-zero weights.
//! * **Weight source** ([`MimiCodec::with_weight_source`]): SEANet enc/dec, down/upsample and RVQ run on
//!   real galion-tensor streaming kernels (A5), and so do both streaming transformers (W20-15:
//!   LayerNorm, RoPE MHA over a KV ring, GELU MLP, LayerScale). Only synthetic/zero sources exist,
//!   so output is NOT Mimi audio and [`crate::IS_STUB`] stays true. Production weights must come
//!   through [`crate::weights::load_codec`], which refuses until A21 clears the licence.
//!
//! All buffers are allocated at construction; `encode_frame`/`decode_frame` do not allocate
//! (FINAL-BUILD-SPEC §5, tests/zero_alloc.rs).

use crate::config::MimiConfig;
use crate::rvq::{RvqKernels, RvqStub};
use crate::seanet::{resample_convs, SeanetDecoder, SeanetEncoder, StreamConv};
use crate::tensor::FakeTensor;
use crate::transformer::{StreamingTransformer, TransformerStub};
use crate::weights::{LoadError, WeightSource};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StubError {
    Config(String),
    Shape(String),
}

impl std::fmt::Display for StubError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StubError::Config(s) => write!(f, "mimi config: {s}"),
            StubError::Shape(s) => write!(f, "mimi shape: {s}"),
        }
    }
}
impl std::error::Error for StubError {}

/// Real conv/RVQ kernels, present only when a weight source was supplied.
struct Kernels {
    enc: SeanetEncoder,
    dec: SeanetDecoder,
    down: StreamConv,
    up: StreamConv,
    rvq: RvqKernels,
    enc_tf: StreamingTransformer,
    dec_tf: StreamingTransformer,
}

pub struct MimiCodec {
    pub cfg: MimiConfig,
    enc_tf: TransformerStub,
    dec_tf: TransformerStub,
    rvq: RvqStub,
    kernels: Option<Box<Kernels>>,
    /// [latent_dim, frame_downsample] at 25 Hz.
    lat_25hz: FakeTensor,
    /// [latent_dim, 1] at 12.5 Hz.
    lat_frame: FakeTensor,
    frames_encoded: u64,
    frames_decoded: u64,
}

/// Wave-1 name kept for callers/gates.
pub type MimiStub = MimiCodec;

impl MimiCodec {
    /// No weights: shape path only, silence / zero codes.
    pub fn new(cfg: MimiConfig) -> Result<Self, StubError> {
        cfg.validate().map_err(StubError::Config)?;
        let steps_25 = cfg.frame_downsample;
        Ok(Self {
            enc_tf: TransformerStub::new(&cfg),
            dec_tf: TransformerStub::new(&cfg),
            rvq: RvqStub::new(&cfg),
            kernels: None,
            lat_25hz: FakeTensor::zeros(&[cfg.latent_dim, steps_25]),
            lat_frame: FakeTensor::zeros(&[cfg.latent_dim, 1]),
            frames_encoded: 0,
            frames_decoded: 0,
            cfg,
        })
    }

    /// Build the real kernel path from a [`WeightSource`] (all allocation happens here).
    /// Dev/test seam: production must go through [`crate::weights::load_codec`] (licence gate).
    pub fn with_weight_source(cfg: MimiConfig, src: &dyn WeightSource) -> Result<Self, LoadError> {
        let mut me = Self::new(cfg).map_err(|e| LoadError::Kernel(e.to_string()))?;
        let c = &me.cfg;
        let [down, up] = resample_convs(c);
        let k = Kernels {
            enc: SeanetEncoder::new(c, src, c.frame_samples())?,
            dec: SeanetDecoder::new(c, src, c.frame_downsample)?,
            down: StreamConv::build(&down, src, c.frame_downsample)?,
            up: StreamConv::build(&up, src, 1)?,
            rvq: RvqKernels::new(c, src)?,
            enc_tf: StreamingTransformer::new(c, src, "enc_transformer")?,
            dec_tf: StreamingTransformer::new(c, src, "dec_transformer")?,
        };
        debug_assert_eq!(k.enc.out_steps(), c.frame_downsample);
        debug_assert_eq!(k.dec.out_samples(), c.frame_samples());
        me.kernels = Some(Box::new(k));
        Ok(me)
    }

    /// True when the real conv/RVQ kernels are built (still a stub overall: see `IS_STUB`).
    pub fn has_kernels(&self) -> bool {
        self.kernels.is_some()
    }
    pub fn frame_samples(&self) -> usize {
        self.cfg.frame_samples()
    }
    pub fn codes_per_frame(&self) -> usize {
        self.cfg.n_codebooks
    }
    pub fn frames_encoded(&self) -> u64 {
        self.frames_encoded
    }
    pub fn frames_decoded(&self) -> u64 {
        self.frames_decoded
    }

    /// pcm[1920] → 12.5 Hz latent `[latent_dim]` (pre-quantizer). Zero allocation.
    pub fn encode_latent_frame(&mut self, pcm: &[f32], latent: &mut [f32]) -> Result<(), StubError> {
        if pcm.len() != self.frame_samples() {
            return Err(StubError::Shape(format!("pcm frame {} != {}", pcm.len(), self.frame_samples())));
        }
        if latent.len() != self.cfg.latent_dim {
            return Err(StubError::Shape(format!("latent {} != {}", latent.len(), self.cfg.latent_dim)));
        }
        match self.kernels.as_deref_mut() {
            Some(k) => {
                k.enc.forward(pcm, &mut self.lat_25hz.data).map_err(StubError::Shape)?;
                k.enc_tf.step(&mut self.lat_25hz).map_err(StubError::Shape)?;
                k.down.step(&self.lat_25hz.data, latent).map_err(StubError::Shape)?;
            }
            None => {
                self.lat_25hz.fill_zero();
                self.enc_tf.step(&mut self.lat_25hz).map_err(StubError::Shape)?;
                latent.iter_mut().for_each(|v| *v = 0.0);
            }
        }
        Ok(())
    }

    /// 12.5 Hz latent `[latent_dim]` → pcm[1920]. Zero allocation.
    pub fn decode_latent_frame(&mut self, latent: &[f32], pcm_out: &mut [f32]) -> Result<(), StubError> {
        if pcm_out.len() != self.frame_samples() {
            return Err(StubError::Shape(format!("pcm_out {} != {}", pcm_out.len(), self.frame_samples())));
        }
        if latent.len() != self.cfg.latent_dim {
            return Err(StubError::Shape(format!("latent {} != {}", latent.len(), self.cfg.latent_dim)));
        }
        match self.kernels.as_deref_mut() {
            Some(k) => {
                k.up.step(latent, &mut self.lat_25hz.data).map_err(StubError::Shape)?;
                k.dec_tf.step(&mut self.lat_25hz).map_err(StubError::Shape)?;
                k.dec.forward(&self.lat_25hz.data, pcm_out).map_err(StubError::Shape)?;
            }
            None => {
                self.lat_25hz.fill_zero();
                self.dec_tf.step(&mut self.lat_25hz).map_err(StubError::Shape)?;
                pcm_out.iter_mut().for_each(|s| *s = 0.0);
            }
        }
        Ok(())
    }

    /// Encode exactly one 80 ms frame. Without weights: codes are all 0.
    pub fn encode_frame(&mut self, pcm: &[f32], codes: &mut [u16]) -> Result<(), StubError> {
        if codes.len() != self.cfg.n_codebooks {
            return Err(StubError::Shape(format!("codes {} != {}", codes.len(), self.cfg.n_codebooks)));
        }
        let mut lat = std::mem::take(&mut self.lat_frame);
        let mut r = self.encode_latent_frame(pcm, &mut lat.data);
        if r.is_ok() {
            r = match self.kernels.as_deref_mut() {
                Some(k) => {
                    k.rvq.encode_step(&lat.data, codes);
                    Ok(())
                }
                None => self.rvq.encode(&lat, codes).map_err(StubError::Shape),
            };
        }
        self.lat_frame = lat;
        r?;
        self.frames_encoded += 1;
        Ok(())
    }

    /// Decode exactly one frame of codes to 80 ms of PCM. Without weights: silence.
    pub fn decode_frame(&mut self, codes: &[u16], pcm_out: &mut [f32]) -> Result<(), StubError> {
        // Shape + code-range check (shared by both modes).
        self.rvq.decode(codes, &mut self.lat_frame).map_err(StubError::Shape)?;
        let mut lat = std::mem::take(&mut self.lat_frame);
        if let Some(k) = self.kernels.as_deref_mut() {
            k.rvq.decode_step(codes, &mut lat.data);
        }
        let r = self.decode_latent_frame(&lat.data, pcm_out);
        self.lat_frame = lat;
        r?;
        self.frames_decoded += 1;
        Ok(())
    }

    /// Drop all streaming state (conv histories, KV rings). Call on stream restart.
    pub fn reset(&mut self) {
        self.enc_tf.reset();
        self.dec_tf.reset();
        if let Some(k) = self.kernels.as_deref_mut() {
            k.enc.reset();
            k.dec.reset();
            k.down.reset();
            k.up.reset();
            k.enc_tf.reset();
            k.dec_tf.reset();
        }
        self.lat_25hz.fill_zero();
        self.lat_frame.fill_zero();
        self.frames_encoded = 0;
        self.frames_decoded = 0;
    }
}
