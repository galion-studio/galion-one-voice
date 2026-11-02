// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! A `galion-presence` [`ReplayBackend`] driven by the real [`Heartbeat`]
//! (feature `presence`, harness only). Turn-taking and E-out come from the
//! deterministic `MockBackend`; the feeling band (Rika) is the real state
//! machine running inside [`Heartbeat::step`]. `rika_lean_in` is reported on
//! the frame the lean-in is **applied** (audible), so P6 measures
//! evidence → audible, i.e. `RikaReport::lean_in_ms()`.
//!
//! The you band is synthesised from the spec's `YouState` with the same
//! seeded signals as the G6 fixtures (LCG seed 13, 24 kHz, 1920-sample frames):
//! room tone RMS 0.001, breath RMS 0.003 white noise, speech = 200 Hz sine 0.1.
//! No WAV, no weights.

use crate::{FrameIn as HbIn, Heartbeat, HeartbeatConfig, FRAME_SAMPLES, SAMPLE_RATE};
use galion_emotion::band::consent_bits;
use galion_emotion::{DefaultConsentGate, EmotionFrame, Purpose};
use galion_presence::backend::{FrameIn, FrameOut, MockBackend, MockConfig, ReplayBackend};
use galion_presence::spec::YouState;

const SEED: u64 = 13;

/// Deterministic LCG (same constants as the G6 fixtures).
#[derive(Debug, Clone, Copy)]
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 40) as f32 / (1u64 << 24) as f32) * 2.0 - 1.0
    }
}

/// Mock turn-taking + real heartbeat/Rika feeling band.
#[derive(Debug)]
pub struct RikaReplayBackend {
    mock: MockBackend,
    hb: Heartbeat<DefaultConsentGate>,
    pcm: Box<[f32; FRAME_SAMPLES]>,
    rng: Lcg,
    me_voiced: bool,
}

impl RikaReplayBackend {
    /// Rika enabled with v0 thresholds (the backend exists to measure Rika).
    pub fn new() -> Self {
        Self::with_config(HeartbeatConfig::rika_v0())
    }

    pub fn with_config(cfg: HeartbeatConfig) -> Self {
        let mut gate = DefaultConsentGate::default();
        // Personal-assistant purpose; per-frame SENSE/ADAPT bits come from the spec
        // (the harness zeroes both upstream for prohibited purposes).
        gate.grant(0, "en", Purpose::PersonalAssistant);
        Self {
            mock: MockBackend::new(MockConfig::default()),
            hb: Heartbeat::new(cfg, gate),
            pcm: Box::new([0.0; FRAME_SAMPLES]),
            rng: Lcg(SEED),
            me_voiced: false,
        }
    }

    pub fn heartbeat(&self) -> &Heartbeat<DefaultConsentGate> {
        &self.hb
    }

    /// Mutable heartbeat access between frames (consent withdraw, purpose swap).
    pub fn heartbeat_mut(&mut self) -> &mut Heartbeat<DefaultConsentGate> {
        &mut self.hb
    }

    /// Live path (W21-01): drive one frame with **real** you-band PCM instead of the
    /// spec-synthesised signal. `pcm` is copied into the fixed frame buffer (zero-padded
    /// if shorter, truncated if longer — the heartbeat still sees `FRAME_SAMPLES`; pass
    /// `f.late` to signal a pause). Affect comes from the heartbeat's consent-gated stub
    /// detector (`affect = None`), consent from `consent_bits`. Never allocates.
    /// Returns the presence frame (turn-taking + E-out) and the heartbeat frame (Rika).
    pub fn step_live(&mut self, f: &FrameIn, pcm: &[f32], consent_bits: u8, purpose: Purpose) -> (FrameOut, crate::FrameOut) {
        let n = pcm.len().min(FRAME_SAMPLES);
        self.pcm[..n].copy_from_slice(&pcm[..n]);
        for x in self.pcm[n..].iter_mut() {
            *x = 0.0;
        }
        let out = self.hb.step(&HbIn {
            you_pcm: &self.pcm[..],
            affect: None,
            tension: None,
            consent_bits,
            purpose,
            session_open: true,
            me_speaking: self.me_voiced,
            late: f.late,
        });
        let o = self.finish(f, &out);
        (o, out)
    }

    fn finish(&mut self, f: &FrameIn, out: &crate::FrameOut) -> FrameOut {
        let mut o = self.mock.step(f);
        // Held breath: the me band may not start a new onset while Rika holds it.
        let onset = o.me_voiced && !self.me_voiced;
        if onset && !out.me_onset_allowed && !f.late {
            o.me_voiced = false;
        }
        self.hb.note_me_onset(out, o.me_voiced && !self.me_voiced);
        self.me_voiced = o.me_voiced;
        o.rika_lean_in = out.lean_in_audible();
        o
    }

    fn synth(&mut self, you: YouState, frame: u32) {
        match you {
            YouState::Silence | YouState::Breath => {
                let rms = if you == YouState::Breath { 0.003 } else { 0.001 };
                let a = rms * 3f32.sqrt();
                for x in self.pcm.iter_mut() {
                    *x = a * self.rng.next();
                }
            }
            YouState::Speech => {
                let base = frame as usize * FRAME_SAMPLES;
                for (i, x) in self.pcm.iter_mut().enumerate() {
                    *x = 0.1 * (2.0 * core::f32::consts::PI * 200.0 * ((base + i) as f32) / SAMPLE_RATE as f32).sin();
                }
            }
        }
    }
}

impl Default for RikaReplayBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl ReplayBackend for RikaReplayBackend {
    fn name(&self) -> &'static str {
        "mock+heartbeat-rika"
    }
    fn supports_rika(&self) -> bool {
        self.hb.config().rika.enabled
    }
    fn reset(&mut self) {
        self.mock.reset();
        self.hb.reset();
        self.rng = Lcg(SEED);
        self.me_voiced = false;
    }
    fn step(&mut self, f: &FrameIn) -> FrameOut {
        self.synth(f.you, f.index);
        let mut bits = 0u8;
        if f.affect_in.is_some() {
            bits |= consent_bits::SENSE;
        }
        if f.adapt {
            bits |= consent_bits::ADAPT;
        }
        let affect = f.affect_in.map(|a| EmotionFrame {
            seq: f.index,
            t_ms: f.index * crate::FRAME_MS,
            dur_ms: crate::FRAME_MS as u16,
            valence: a.valence,
            arousal: a.arousal,
            dominance: 0.5,
            confidence: 1.0, // scripted by the spec, not inferred
        });
        let out = self.hb.step(&HbIn {
            you_pcm: &self.pcm[..],
            affect,
            tension: None,
            consent_bits: bits,
            purpose: Purpose::PersonalAssistant,
            session_open: true,
            me_speaking: self.me_voiced,
            late: f.late,
        });
        self.finish(f, &out)
    }
}
