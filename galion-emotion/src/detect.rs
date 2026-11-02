//! Inbound: caller audio -> EmotionFrame.
//!
//! Phase 2 stub: RMS energy + zero-crossing rate heuristics only (the maths
//! lives in [`crate::loudness::LoudnessEstimator`]; see that module for the
//! estimator seam where valence plugs in). The real
//! detector will be a tiny head on the shared audio encoder (Mimi-style codec
//! latents of the distilled PersonaPlex student) so affect costs ~0 extra
//! compute — that is the "delete the part" move: no separate SER model.

use crate::consent::{ConsentGate, Purpose};
use crate::frame::EmotionFrame;
use crate::loudness::{GatedDetector, LoudnessEstimator};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectError {
    /// Consent gate refused; caller receives no inference. Not a failure of
    /// the call — the conversation continues without the affect band.
    NotPermitted(crate::consent::ConsentDecision),
    BadInput(&'static str),
}

pub trait AffectDetector {
    /// Analyse one window of mono f32 PCM in [-1, 1].
    /// Implementations MUST call the gate before touching `pcm`.
    fn detect(
        &mut self,
        pcm: &[f32],
        sample_rate: u32,
        t_ms: u32,
        gate: &dyn ConsentGate,
        purpose: Purpose,
    ) -> Result<EmotionFrame, DetectError>;

    /// Drop all internal state (call on consent withdrawal / hang-up).
    fn reset(&mut self);
}

/// Heuristic stub. Arousal ~ loudness, valence unknown (0), confidence low.
///
/// Since A11 wave 2 this is a thin newtype over
/// [`GatedDetector`]`<`[`LoudnessEstimator`]`>` — same maths, bit-identical
/// frames (tested in `loudness.rs`). To get valence without a model, swap the
/// estimator (`SyntheticEstimator`, `ReferenceEstimator`) instead of editing
/// this type; the consent / store / band path does not change.
#[derive(Debug, Default)]
pub struct EnergyStubDetector(GatedDetector<LoudnessEstimator>);

impl EnergyStubDetector {
    /// The underlying gated loudness detector.
    pub fn inner(&self) -> &GatedDetector<LoudnessEstimator> {
        &self.0
    }
}

impl AffectDetector for EnergyStubDetector {
    fn detect(
        &mut self,
        pcm: &[f32],
        sample_rate: u32,
        t_ms: u32,
        gate: &dyn ConsentGate,
        purpose: Purpose,
    ) -> Result<EmotionFrame, DetectError> {
        self.0.detect(pcm, sample_rate, t_ms, gate, purpose)
    }

    fn reset(&mut self) {
        self.0.reset()
    }
}

/// Boxed detectors are detectors (lets hosts pick an estimator at runtime).
impl<D: AffectDetector + ?Sized> AffectDetector for Box<D> {
    fn detect(
        &mut self,
        pcm: &[f32],
        sample_rate: u32,
        t_ms: u32,
        gate: &dyn ConsentGate,
        purpose: Purpose,
    ) -> Result<EmotionFrame, DetectError> {
        (**self).detect(pcm, sample_rate, t_ms, gate, purpose)
    }

    fn reset(&mut self) {
        (**self).reset()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consent::{ConsentDecision, DefaultConsentGate};

    fn tone(amp: f32, n: usize) -> Vec<f32> {
        (0..n).map(|i| amp * (i as f32 * 0.1).sin()).collect()
    }

    fn granted() -> DefaultConsentGate {
        let mut g = DefaultConsentGate::default();
        g.grant(0, "hr", Purpose::PersonalAssistant);
        g
    }

    #[test]
    fn refuses_without_consent() {
        let mut d = EnergyStubDetector::default();
        let g = DefaultConsentGate::default();
        let r = d.detect(&tone(0.1, 1920), 24_000, 0, &g, Purpose::PersonalAssistant);
        assert_eq!(r, Err(DetectError::NotPermitted(ConsentDecision::DenyNoConsent)));
    }

    #[test]
    fn louder_means_more_arousal() {
        let g = granted();
        let mut d1 = EnergyStubDetector::default();
        let mut d2 = EnergyStubDetector::default();
        let quiet = d1.detect(&tone(0.02, 1920), 24_000, 0, &g, Purpose::PersonalAssistant).unwrap();
        let loud = d2.detect(&tone(0.4, 1920), 24_000, 0, &g, Purpose::PersonalAssistant).unwrap();
        assert!(loud.arousal > quiet.arousal);
        assert_eq!(loud.dur_ms, 80);
    }

    #[test]
    fn silence_has_zero_confidence() {
        let g = granted();
        let mut d = EnergyStubDetector::default();
        let f = d.detect(&vec![0.0; 1920], 24_000, 0, &g, Purpose::PersonalAssistant).unwrap();
        assert_eq!(f.confidence, 0.0);
    }

    #[test]
    fn bad_input() {
        let g = granted();
        let mut d = EnergyStubDetector::default();
        assert!(matches!(d.detect(&[], 24_000, 0, &g, Purpose::PersonalAssistant), Err(DetectError::BadInput(_))));
        assert!(matches!(d.detect(&[0.1], 0, 0, &g, Purpose::PersonalAssistant), Err(DetectError::BadInput(_))));
    }

    #[test]
    fn seq_increments() {
        let g = granted();
        let mut d = EnergyStubDetector::default();
        let a = d.detect(&tone(0.1, 1920), 24_000, 0, &g, Purpose::PersonalAssistant).unwrap();
        let b = d.detect(&tone(0.1, 1920), 24_000, 80, &g, Purpose::PersonalAssistant).unwrap();
        assert_eq!(b.seq, a.seq + 1);
    }
}
