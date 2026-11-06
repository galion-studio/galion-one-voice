//! Inbound: caller audio -> EmotionFrame.
//!
//! Phase 2 stub: RMS energy + zero-crossing rate heuristics only (the maths
//! lives in [`crate::loudness::LoudnessEstimator`]; see that module for the
//! estimator seam where valence plugs in). The real
//! detector will be a tiny head on the shared audio encoder (Mimi-style codec
//! latents of the distilled PersonaPlex student) so affect costs ~0 extra
//! compute — that is the "delete the part" move: no separate SER model.
//!
//! [`EnergyStubDetector::recognize`] maps those same features onto the closed
//! [`Affect`] set. Arousal and dominance come from loudness. Valence is applied
//! only when the caller already has an opinion (text lexicon, offline reference).
//! RMS and zero-crossing rate do not invent one. Confidence stays the heuristic
//! value. Nothing here is a measured accuracy, and nothing here reads weights.

use crate::consent::{ConsentGate, Purpose};
use crate::frame::{Affect, EmotionFrame};
use crate::loudness::{AcousticFeatures, Estimate, GatedDetector, LoudnessEstimator};

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

    /// Consent-gated closed-set recognition.
    ///
    /// The gate runs inside [`AffectDetector::detect`] before PCM is read.
    /// `valence` is an opinion from another feature already in this crate.
    /// Pass `None` when the only evidence is RMS and zero-crossing rate.
    /// The label uses the smoothed frame, so it matches what the band stored.
    pub fn recognize(
        &mut self,
        pcm: &[f32],
        sample_rate: u32,
        t_ms: u32,
        gate: &dyn ConsentGate,
        purpose: Purpose,
        valence: Option<f32>,
    ) -> Result<Recognition, DetectError> {
        let frame = self.0.detect(pcm, sample_rate, t_ms, gate, purpose)?;
        let features = AcousticFeatures::from_pcm(pcm);
        let opinion = valence.filter(|v| v.is_finite());
        let labeled = EmotionFrame { valence: opinion.unwrap_or(frame.valence), ..frame }.clamped();
        Ok(Recognition {
            features,
            estimate: Estimate {
                valence: None,
                arousal: frame.arousal,
                dominance: frame.dominance,
                confidence: frame.confidence,
            },
            label: labeled.label(),
            valence_known: opinion.is_some(),
            labeled,
            frame,
        })
    }
}

/// One closed-set decision. `label` is always an [`Affect`] variant.
///
/// `frame` is the loudness detector frame (valence unknown → 0.0).
/// `labeled` is that decision with the caller valence applied, when one was
/// supplied. `estimate.valence` stays `None`: loudness did not measure it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Recognition {
    pub frame: EmotionFrame,
    pub labeled: EmotionFrame,
    pub features: AcousticFeatures,
    pub estimate: Estimate,
    pub label: Affect,
    pub valence_known: bool,
}

/// Map RMS / zero-crossing features through the loudness heuristic onto [`Affect`].
///
/// The eight [`Affect`] variants are the whole label set. `valence` is used
/// only when it is finite. This is a rule, not a weight file, and `confidence`
/// is the heuristic's constant rather than an accuracy.
pub fn recognize_features(
    features: &AcousticFeatures,
    estimator: &LoudnessEstimator,
    valence: Option<f32>,
) -> Recognition {
    let estimate = estimator.estimate_features(*features);
    let opinion = valence.filter(|v| v.is_finite());
    let frame = EmotionFrame {
        seq: 0,
        t_ms: 0,
        dur_ms: 0,
        valence: 0.0,
        arousal: estimate.arousal,
        dominance: estimate.dominance,
        confidence: estimate.confidence,
    }
    .clamped();
    let labeled = EmotionFrame {
        valence: opinion.unwrap_or(0.0),
        ..frame
    }
    .clamped();
    Recognition {
        frame,
        labeled,
        features: *features,
        estimate: Estimate {
            valence: None,
            arousal: frame.arousal,
            dominance: frame.dominance,
            confidence: frame.confidence,
        },
        label: labeled.label(),
        valence_known: opinion.is_some(),
    }
}

/// Closed label set: the eight [`Affect`] names, and no others.
pub fn closed_label_name(label: Affect) -> &'static str {
    match label {
        Affect::Neutral => "neutral",
        Affect::Calm => "calm",
        Affect::Joy => "joy",
        Affect::Excited => "excited",
        Affect::Sad => "sad",
        Affect::Tired => "tired",
        Affect::Angry => "angry",
        Affect::Anxious => "anxious",
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

    fn sine(amp: f32, hz: f32, n: usize) -> Vec<f32> {
        let sr = 24_000.0;
        (0..n).map(|i| amp * (std::f32::consts::TAU * hz * i as f32 / sr).sin()).collect()
    }

    fn alternate(amp: f32, n: usize) -> Vec<f32> {
        (0..n).map(|i| if i % 2 == 0 { amp } else { -amp }).collect()
    }

    fn recognize_one(pcm: &[f32], valence: Option<f32>) -> Recognition {
        let g = granted();
        let mut d = EnergyStubDetector::default();
        d.recognize(pcm, 24_000, 0, &g, Purpose::PersonalAssistant, valence).unwrap()
    }

    #[test]
    fn recognize_refuses_without_consent_and_on_prohibited_purpose() {
        let mut d = EnergyStubDetector::default();
        let deny = DefaultConsentGate::default();
        let pcm = sine(0.4, 200.0, 1920);
        let denied = d.recognize(&pcm, 24_000, 0, &deny, Purpose::PersonalAssistant, Some(0.8));
        assert_eq!(denied, Err(DetectError::NotPermitted(ConsentDecision::DenyNoConsent)));

        let mut prohibited = DefaultConsentGate::default();
        prohibited.grant(0, "hr", Purpose::Workplace);
        let work = d.recognize(&pcm, 24_000, 0, &prohibited, Purpose::Workplace, Some(-0.8));
        assert_eq!(work, Err(DetectError::NotPermitted(ConsentDecision::DenyProhibitedPurpose)));
        let school = d.recognize(&pcm, 24_000, 0, &prohibited, Purpose::Education, None);
        assert_eq!(school, Err(DetectError::NotPermitted(ConsentDecision::DenyProhibitedPurpose)));
    }

    #[test]
    fn acoustics_alone_stay_in_neutral_or_calm() {
        let calm = recognize_one(&sine(0.04, 180.0, 1920), None);
        let neutral = recognize_one(&sine(0.2, 180.0, 1920), None);
        assert_eq!(calm.label, Affect::Calm);
        assert_eq!(neutral.label, Affect::Neutral);
        assert!(neutral.frame.arousal > calm.frame.arousal);
        for r in [calm, neutral] {
            assert!(!r.valence_known);
            assert_eq!(r.frame.valence, 0.0);
            assert!(r.estimate.valence.is_none());
            assert_eq!(r.label, r.frame.label());
            assert_eq!(r.labeled, r.frame);
            assert_eq!(r.estimate.confidence, LoudnessEstimator::default().confidence);
            assert_eq!(r.frame.dur_ms, 80);
        }
    }

    #[test]
    fn silence_stays_neutral_when_a_valence_opinion_is_supplied() {
        let r = recognize_one(&vec![0.0; 1920], Some(0.9));
        assert_eq!(r.label, Affect::Neutral);
        assert_eq!(r.frame.confidence, 0.0);
        assert_eq!(r.labeled.confidence, 0.0);
        assert!(r.valence_known);
        assert_eq!(r.frame.valence, 0.0);
        assert!((r.labeled.valence - 0.9).abs() < 1e-6);
        assert!(r.estimate.valence.is_none());
    }

    #[test]
    fn closed_set_is_reached_from_rms_zcr_and_a_valence_opinion() {
        let cases = [
            (sine(0.04, 180.0, 1920), None, Affect::Calm),
            (sine(0.2, 180.0, 1920), None, Affect::Neutral),
            (sine(0.08, 180.0, 1920), Some(0.6), Affect::Joy),
            (sine(0.4, 200.0, 1920), Some(0.6), Affect::Excited),
            (sine(0.04, 160.0, 1920), Some(-0.5), Affect::Tired),
            (sine(0.16, 160.0, 1920), Some(-0.6), Affect::Sad),
            (sine(0.4, 200.0, 1920), Some(-0.8), Affect::Angry),
            (alternate(0.25, 1920), Some(-0.8), Affect::Anxious),
        ];
        let mut seen = [false; 8];
        for (pcm, valence, want) in cases {
            let r = recognize_one(&pcm, valence);
            assert_eq!(r.label, want, "{}", closed_label_name(r.label));
            assert_eq!(r.label, r.labeled.label());
            assert_eq!(r.frame.valence, 0.0);
            assert!(r.estimate.valence.is_none());
            assert_eq!(r.valence_known, valence.is_some());
            assert_eq!(closed_label_name(r.label), closed_label_name(want));
            seen[r.label as u8 as usize] = true;
        }
        assert_eq!(seen, [true; 8]);
    }

    #[test]
    fn nonfinite_valence_is_not_an_opinion() {
        let features = AcousticFeatures::from_pcm(&sine(0.2, 180.0, 1920));
        let loud = LoudnessEstimator::default();
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let r = recognize_features(&features, &loud, Some(bad));
            assert!(!r.valence_known);
            assert!(r.estimate.valence.is_none());
            assert_eq!(r.label, Affect::Neutral);
        }
    }

    #[test]
    fn lexicon_valence_plus_loudness_features_labels_angry() {
        let g = granted();
        let mut d = EnergyStubDetector::default();
        let text = crate::persona_mood::text_affect("I am angry");
        let valence = text.valence.expect("lexicon contains angry");
        assert!(valence < 0.0);
        let r = d.recognize(&sine(0.4, 200.0, 1920), 24_000, 0, &g, Purpose::PersonalAssistant, Some(valence)).unwrap();
        assert_eq!(r.label, Affect::Angry);
        assert_eq!(closed_label_name(r.label), "angry");
        assert!(!d.inner().valence_known(), "loudness still has no valence opinion");
        assert_eq!(r.frame.seq, 0);
        let again = d.recognize(&sine(0.4, 200.0, 1920), 24_000, 80, &g, Purpose::PersonalAssistant, Some(valence)).unwrap();
        assert_eq!(again.frame.seq, 1);
    }

    #[test]
    fn recognize_rejects_empty_window_and_zero_rate() {
        let g = granted();
        let mut d = EnergyStubDetector::default();
        assert!(matches!(
            d.recognize(&[], 24_000, 0, &g, Purpose::PersonalAssistant, Some(0.2)),
            Err(DetectError::BadInput(_))
        ));
        assert!(matches!(
            d.recognize(&[0.1, -0.1], 0, 0, &g, Purpose::PersonalAssistant, None),
            Err(DetectError::BadInput(_))
        ));
    }

    #[test]
    fn closed_names_are_the_eight_affect_labels() {
        let labels = [
            Affect::Neutral,
            Affect::Calm,
            Affect::Joy,
            Affect::Excited,
            Affect::Sad,
            Affect::Tired,
            Affect::Angry,
            Affect::Anxious,
        ];
        let mut names: Vec<&'static str> = labels.into_iter().map(closed_label_name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), labels.len());
        for label in labels {
            assert_eq!(Affect::from_u8(label as u8), label);
            assert!(!closed_label_name(label).is_empty());
        }
    }
}
