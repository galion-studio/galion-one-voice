// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! W21-02 hook: route the me band through the student backend's Mimi lane
//! (`StudentBackend::step_frame`, 1920 samples / 80 ms @ 24 kHz) every frame.
//! The codec is synthetic (`IS_STUB`, ZeroWeights, demo widths). For an audible demo
//! the lane runs in `Monitor::DryBypass` (codec runs for real timing, output = dry
//! input) and every frame is labelled "MIMI synthetic (IS_STUB)" / "BYPASS".
//! Never the full-size config live (≈1.3 s/frame on the scalar path).
use crate::{MeCtx, MeVoice};
use galion_heartbeat::FRAME_SAMPLES;
use galion_student_backend::{MimiLaneConfig, MimiPassthrough, Monitor, StudentBackend};

/// Which Mimi lane to use (`--mimi off|bypass|codec|nokernels`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MimiMode {
    Off,
    /// Demo widths, ZeroWeights kernels, DryBypass monitor (audible). Release default.
    Bypass,
    /// Demo widths, ZeroWeights kernels, codec monitor (decoded output = silence today).
    Codec,
    /// Full shapes, no kernel compute, DryBypass (cheapest fallback).
    NoKernels,
}

impl MimiMode {
    pub fn parse(s: &str) -> Result<Self, String> {
        Ok(match s {
            "off" => Self::Off,
            "bypass" => Self::Bypass,
            "codec" => Self::Codec,
            "nokernels" => Self::NoKernels,
            o => return Err(format!("--mimi: unknown '{o}' (off|bypass|codec|nokernels)")),
        })
    }
    /// Release builds route through Mimi (bypass) by default; debug builds skip it (speed).
    pub fn default_for_build() -> Self {
        if cfg!(debug_assertions) {
            Self::Off
        } else {
            Self::Bypass
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Bypass => "bypass",
            Self::Codec => "codec",
            Self::NoKernels => "nokernels",
        }
    }
}

/// Inner voice → Mimi lane → me band.
pub struct MimiVoice<V: MeVoice> {
    inner: V,
    student: StudentBackend,
    scratch: Box<[f32; FRAME_SAMPLES]>,
    tag: &'static str,
    name: &'static str,
    pub errors: u32,
    pub codec_frames: u64,
}

impl<V: MeVoice> MimiVoice<V> {
    pub fn new(inner: V, mode: MimiMode) -> Result<Self, String> {
        let (cfg, name) = match mode {
            MimiMode::Off => return Err("MimiVoice with --mimi off".into()),
            MimiMode::Bypass => (MimiLaneConfig::demo().with_monitor(Monitor::DryBypass), "tone→mimi(demo,ZeroWeights,DryBypass)"),
            MimiMode::Codec => (MimiLaneConfig::demo(), "tone→mimi(demo,ZeroWeights,Codec)"),
            MimiMode::NoKernels => (MimiLaneConfig::no_kernels().with_monitor(Monitor::DryBypass), "tone→mimi(no_kernels,DryBypass)"),
        };
        let lane = MimiPassthrough::new(cfg).map_err(|e| e.to_string())?;
        Ok(Self { inner, student: StudentBackend::with_mimi(lane), scratch: Box::new([0.0; FRAME_SAMPLES]), tag: "", name, errors: 0, codec_frames: 0 })
    }
}

impl<V: MeVoice> MeVoice for MimiVoice<V> {
    fn name(&self) -> &'static str {
        self.name
    }
    fn reset(&mut self) {
        self.inner.reset();
        if let Some(l) = self.student.mimi_mut() {
            l.reset();
        }
    }
    fn tag(&self) -> &'static str {
        self.tag
    }
    fn planned(&self) -> bool {
        self.inner.planned()
    }
    fn render(&mut self, ctx: &MeCtx, out: &mut [f32]) {
        self.inner.render(ctx, &mut self.scratch[..]);
        match self.student.step_frame(&self.scratch[..], out) {
            Ok(rep) => {
                self.codec_frames += rep.codec_ran as u64;
                self.tag = match (rep.is_stub, rep.bypassed) {
                    (true, true) => "MIMI synthetic (IS_STUB) BYPASS",
                    (true, false) => "MIMI synthetic (IS_STUB)",
                    (false, true) => "MIMI BYPASS",
                    (false, false) => "MIMI",
                };
            }
            Err(_) => {
                // Never glitch: fall back to the dry inner voice for this frame.
                self.errors += 1;
                out.copy_from_slice(&self.scratch[..out.len()]);
                self.tag = "MIMI ERR→dry";
            }
        }
    }
}
