//! Outbound: target emotion -> prosody controls for Galion's voice.
//!
//! Two steps, deliberately separated:
//! 1. [`EmpathyPolicy`] decides *what Galion should feel* given the caller's
//!    frame. It is NOT a mirror: anger is met with calm warmth, sadness with
//!    gentle low-arousal warmth, joy is shared. This is the "love and
//!    understanding" rule encoded as a tiny, auditable function.
//! 2. [`ProsodyExpressor`] turns that target into knobs the speech decoder
//!    consumes (pitch, rate, energy, warmth, pause). Stub = rule table; real
//!    version = conditioning vector into the student model's depth decoder.

use crate::frame::{Affect, EmotionFrame};

/// Decoder-agnostic prosody knobs. Neutral = all defaults.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProsodyControl {
    /// Semitones relative to the voice's base pitch, clamped to ±4.
    pub pitch_shift_st: f32,
    /// Pitch range multiplier (expressiveness), 0.5..1.5.
    pub pitch_range: f32,
    /// Speaking-rate multiplier, 0.8..1.2.
    pub rate: f32,
    /// Loudness multiplier, 0.6..1.2.
    pub energy: f32,
    /// 0..1 spectral warmth / breathiness hint.
    pub warmth: f32,
    /// Extra pause before speaking, ms (0..400) — gives room to an upset caller.
    pub pre_pause_ms: u16,
}

impl Default for ProsodyControl {
    fn default() -> Self {
        Self { pitch_shift_st: 0.0, pitch_range: 1.0, rate: 1.0, energy: 1.0, warmth: 0.5, pre_pause_ms: 0 }
    }
}

impl ProsodyControl {
    pub fn clamped(mut self) -> Self {
        self.pitch_shift_st = self.pitch_shift_st.clamp(-4.0, 4.0);
        self.pitch_range = self.pitch_range.clamp(0.5, 1.5);
        self.rate = self.rate.clamp(0.8, 1.2);
        self.energy = self.energy.clamp(0.6, 1.2);
        self.warmth = self.warmth.clamp(0.0, 1.0);
        self.pre_pause_ms = self.pre_pause_ms.min(400);
        self
    }
}

pub trait EmpathyPolicy {
    /// Given the caller's (consented) frame, return Galion's target frame.
    /// `None` caller frame (no consent / unknown) -> neutral-warm default.
    fn respond(&self, caller: Option<&EmotionFrame>) -> EmotionFrame;
}

pub trait ProsodyExpressor {
    fn plan(&self, target: &EmotionFrame) -> ProsodyControl;
    /// Stub hook for applying controls directly to PCM (energy only here).
    fn apply_energy(&self, ctrl: &ProsodyControl, pcm: &mut [f32]) {
        for s in pcm.iter_mut() {
            *s = (*s * ctrl.energy).clamp(-1.0, 1.0);
        }
    }
}

/// Reference rule-based implementation of both traits.
#[derive(Debug, Default, Clone, Copy)]
pub struct StubExpressor;

impl EmpathyPolicy for StubExpressor {
    fn respond(&self, caller: Option<&EmotionFrame>) -> EmotionFrame {
        let base = |seq, t, d| EmotionFrame {
            seq, t_ms: t, dur_ms: d, valence: 0.3, arousal: 0.35, dominance: 0.5, confidence: 1.0,
        };
        let Some(c) = caller else { return base(0, 0, 0) };
        let mut out = base(c.seq, c.t_ms, c.dur_ms);
        match c.label() {
            // De-escalate: calmer, lower dominance, still warm.
            Affect::Angry => { out.arousal = 0.2; out.dominance = 0.35; out.valence = 0.2; }
            Affect::Anxious => { out.arousal = 0.15; out.dominance = 0.55; out.valence = 0.3; }
            Affect::Sad | Affect::Tired => { out.arousal = 0.2; out.valence = 0.25; out.dominance = 0.45; }
            // Share positive energy, slightly below the caller.
            Affect::Joy | Affect::Excited => {
                out.valence = (c.valence * 0.9).max(0.3);
                out.arousal = (c.arousal * 0.85).max(0.35);
            }
            Affect::Calm | Affect::Neutral => {}
        }
        out.clamped()
    }
}

impl ProsodyExpressor for StubExpressor {
    fn plan(&self, t: &EmotionFrame) -> ProsodyControl {
        let t = t.clamped();
        let low_arousal_negative_context = t.arousal < 0.25;
        ProsodyControl {
            pitch_shift_st: 2.0 * t.valence + 1.5 * (t.arousal - 0.35),
            pitch_range: 0.8 + 0.6 * t.arousal,
            rate: 0.9 + 0.25 * t.arousal,
            energy: 0.75 + 0.4 * t.arousal,
            warmth: 0.5 + 0.4 * t.valence.max(0.0) + 0.1 * (1.0 - t.dominance),
            pre_pause_ms: if low_arousal_negative_context { 250 } else { 0 },
        }
        .clamped()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(v: f32, a: f32, d: f32) -> EmotionFrame {
        EmotionFrame { seq: 3, t_ms: 240, dur_ms: 80, valence: v, arousal: a, dominance: d, confidence: 0.9 }
    }

    #[test]
    fn angry_caller_gets_calm_galion() {
        let p = StubExpressor;
        let caller = f(-0.8, 0.9, 0.8);
        let tgt = p.respond(Some(&caller));
        assert!(tgt.arousal < caller.arousal);
        assert!(tgt.valence > 0.0);
        let ctrl = p.plan(&tgt);
        assert!(ctrl.rate < 1.0 && ctrl.energy < 1.0);
        assert!(ctrl.pre_pause_ms > 0);
    }

    #[test]
    fn joyful_caller_is_shared() {
        let p = StubExpressor;
        let tgt = p.respond(Some(&f(0.8, 0.8, 0.5)));
        assert!(tgt.valence >= 0.7 && tgt.arousal >= 0.6);
        let ctrl = p.plan(&tgt);
        assert!(ctrl.pitch_shift_st > 0.0 && ctrl.rate > 1.0);
    }

    #[test]
    fn no_consent_yields_neutral_warm() {
        let p = StubExpressor;
        let tgt = p.respond(None);
        assert!(tgt.valence > 0.0);
        let c = p.plan(&tgt);
        assert!(c.warmth > 0.5);
    }

    #[test]
    fn controls_always_in_range() {
        let p = StubExpressor;
        for v in [-1.0f32, 0.0, 1.0] {
            for a in [0.0f32, 0.5, 1.0] {
                let c = p.plan(&f(v, a, 0.5));
                assert!((-4.0..=4.0).contains(&c.pitch_shift_st));
                assert!((0.8..=1.2).contains(&c.rate));
                assert!((0.6..=1.2).contains(&c.energy));
                assert!((0.0..=1.0).contains(&c.warmth));
            }
        }
    }

    #[test]
    fn apply_energy_scales_and_clips() {
        let p = StubExpressor;
        let mut pcm = vec![0.5, -0.9, 1.0];
        let c = ProsodyControl { energy: 1.2, ..Default::default() };
        p.apply_energy(&c, &mut pcm);
        assert!((pcm[0] - 0.6).abs() < 1e-6);
        assert_eq!(pcm[1], -1.0);
        assert_eq!(pcm[2], 1.0);
    }
}
