// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! Moment deltas, kept off the steady tract.
//!
//! Every gain and every shift below is a knob. None of them is a z against
//! a speaker's own neutral, and none is a hertz figure copied out of a paper.
//! The rest formants are the Burg medians already written down. They are
//! anchors for the shift, not a new measurement.

use galion_emotion_core::{Breath, BreathPhase, MomentState, VoiceBaseline};
use horbm_breath::{controls, Articulation, Drive, Frame};

/// Symphony clock. Same constant as `galion-emotion-core` and `LOCKS.toml`.
pub const FRAME_MS: u32 = 80;

/// Neutral output may differ from the input by at most this absolute amount.
/// The neutral path does not touch the buffer, so the difference is zero.
pub const NEUTRAL_TOLERANCE: f32 = 1.0e-5;

/// Positive tension raises this many dB of spectral tilt at full scale.
pub const TENSION_DB: f32 = 9.0;
/// Full breathiness lowers the tilt by this many dB. It does not add noise.
pub const BREATHINESS_DB: f32 = 8.0;
/// Shelf corner for the tilt, in hertz.
pub const TILT_PIVOT_HZ: f32 = 1_000.0;

/// Fraction of F1 moved at openness +1. A knob, so a probe can see the peak move.
pub const F1_SHIFT: f32 = 0.70;
/// Fraction of F2 moved at openness +1.
pub const F2_SHIFT: f32 = 0.30;
/// Peaking depth once openness leaves rest, in dB. The cut uses the negation.
/// Frequency and Q carry the amount; the depth does not shrink with it.
pub const FORMANT_DB: f32 = 12.0;
/// Rest Q of the F1 pair. Opening divides it.
pub const F1_Q0: f32 = 7.0;
/// Rest Q of the F2 pair.
pub const F2_Q0: f32 = 8.0;
/// `Q = Q0 / (1 + OPEN_Q_SCALE * openness)` for openness above zero.
pub const OPEN_Q_SCALE: f32 = 1.5;

/// Lower nasal antiresonance. The brief's low band, not a digitised pole.
pub const NASAL_LO_HZ: f32 = 250.0;
/// Upper nasal antiresonance, inside 1–2 kHz.
pub const NASAL_HI_HZ: f32 = 1_500.0;
/// Depth of each nasal dip at coupling 1, in dB (the peaking gain is negative).
pub const NASAL_DEPTH_DB: f32 = 18.0;

/// Bandpass centre for exhale noise, in hertz.
pub const BREATH_CENTER_HZ: f32 = 700.0;
/// Minimum noise gain once the phase is exhale, before flow is added.
pub const BREATH_FLOOR: f32 = 0.45;

const TILT_Q: f32 = 0.7;
const NASAL_Q: f32 = 5.0;
const SECTION_EPS: f32 = 1.0e-4;

/// True only when `id` is exactly `configured_id`.
pub fn applies_to(id: &str, configured_id: &str) -> bool {
    id == configured_id
}

/// Samples in one 80 ms frame at `sample_rate`. Zero when the rate is zero.
pub fn frame_samples(sample_rate: u32) -> usize {
    (sample_rate as usize).saturating_mul(FRAME_MS as usize) / 1000
}

/// Signed tilt in dB. Positive is brighter (more high-frequency energy).
pub fn tilt_db(controls: MouthControls) -> f32 {
    let tension = finite_clamp(controls.tension, -1.0, 1.0);
    let breathiness = finite_clamp(controls.breathiness, 0.0, 1.0);
    TENSION_DB * tension - BREATHINESS_DB * breathiness
}

/// Q after openness has widened a formant. Closing does not narrow past `base_q`.
pub fn formant_q(base_q: f32, openness: f32) -> f32 {
    let open = if openness.is_finite() {
        openness.max(0.0)
    } else {
        0.0
    };
    (base_q / (1.0 + OPEN_Q_SCALE * open)).clamp(0.5, 24.0)
}

/// Where F1 and F2 sit for this speaker when nothing is felt.
///
/// [`Self::REST`] uses the Burg medians from the articulation layer note
/// (F1 431.512 Hz, F2 1857.581 Hz). Bandwidths are not taken from that note.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TractBaseline {
    pub f1_hz: f32,
    pub f2_hz: f32,
}

impl TractBaseline {
    pub const REST: Self = Self {
        f1_hz: 431.512,
        f2_hz: 1857.581,
    };

    /// Formant centres after an openness delta. `0` returns the rest pair.
    pub fn shifted(self, openness: f32) -> (f32, f32) {
        let o = finite_clamp(openness, -1.0, 1.0);
        let f1 = (self.f1_hz * (1.0 + F1_SHIFT * o)).clamp(120.0, 1_200.0);
        let f2 = (self.f2_hz * (1.0 + F2_SHIFT * o)).clamp(f1 + 200.0, 4_000.0);
        (f1, f2)
    }
}

/// One frame of mouth controls. Zero is the speaker at rest: no tilt, no
/// formant move, no nasal dip, no breath noise.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MouthControls {
    /// Glottal tension relative to rest. Positive raises spectral tilt.
    pub tension: f32,
    /// Breathy phonation relative to rest. Positive lowers spectral tilt.
    /// This is not the exhale. Exhale noise is [`Self::breath_noise`].
    pub breathiness: f32,
    /// Jaw / tract opening relative to rest. Positive raises F1 and F2 and
    /// widens them. Negative lowers them.
    pub openness: f32,
    /// Velopharyngeal coupling, 0 (closed) to 1. Not an emotion axis.
    pub nasal: f32,
    /// Radiated breath gain. Non-zero only for an exhale when built from a moment.
    pub breath_noise: f32,
}

impl MouthControls {
    pub const fn neutral() -> Self {
        Self {
            tension: 0.0,
            breathiness: 0.0,
            openness: 0.0,
            nasal: 0.0,
            breath_noise: 0.0,
        }
    }

    pub fn is_neutral(self) -> bool {
        tilt_db(self).abs() < SECTION_EPS
            && self.openness.abs() < SECTION_EPS
            && self.nasal.abs() < SECTION_EPS
            && self.breath_noise.abs() < SECTION_EPS
    }

    /// Exhale noise from a lungs sample, plus a velum the caller already chose.
    ///
    /// Does not write `voice` or `moment`. Does not turn the axes into
    /// glottal tension, breathiness, or mouth openness. Those come from
    /// [`Self::from_breath_frame`].
    pub fn from_moment(
        voice: &VoiceBaseline,
        moment: &MomentState,
        breath: Breath,
        velum: f32,
    ) -> Self {
        let _baseline = voice;
        let _moment = moment;
        Self {
            tension: 0.0,
            breathiness: 0.0,
            openness: 0.0,
            nasal: finite_clamp(velum, 0.0, 1.0),
            breath_noise: exhale_noise(breath.phase, breath.flow),
        }
    }

    /// Render one `horbm-breath` frame.
    ///
    /// Open quotient, breathiness, nasal coupling, and mouth openness are
    /// that frame's fields. The deltas are against [`controls`] at the rest
    /// drive, speech phase, nasal target 0, and mouth target 0.5 (the value
    /// `Lungs` starts with). This crate does not recompute Rd, Ra, or Rk.
    /// Breath noise still follows the exhale phase on the same frame.
    pub fn from_breath_frame(frame: Frame) -> Self {
        let rest = rest_speech();
        let breathiness = finite_clamp(frame.breathiness - rest.breathiness, 0.0, 1.0);
        let tension = if rest.open_quotient > 1.0e-3 {
            finite_clamp(
                (rest.open_quotient - frame.open_quotient) / rest.open_quotient,
                0.0,
                1.0,
            )
        } else {
            0.0
        };
        let span = rest.mouth_openness.max(0.05);
        let openness = finite_clamp(
            (frame.mouth_openness - rest.mouth_openness) / span,
            -1.0,
            1.0,
        );
        let nasal = finite_clamp(frame.nasal_coupling - rest.nasal_coupling, 0.0, 1.0);
        Self {
            tension,
            breathiness,
            openness,
            nasal,
            breath_noise: exhale_noise(frame.phase, frame.flow),
        }
    }
}

/// Rest speech articulation from `horbm-breath`. Mouth 0.5 is `Lungs`' start.
fn rest_speech() -> Articulation {
    controls(&Drive::rest(), BreathPhase::Speech, 0.0, 0.5)
}

fn exhale_noise(phase: BreathPhase, flow: f32) -> f32 {
    if phase != BreathPhase::Exhale {
        return 0.0;
    }
    let outward = if flow.is_finite() && flow < 0.0 {
        -flow
    } else {
        0.0
    };
    outward.max(BREATH_FLOOR).clamp(0.0, 1.0)
}

pub(crate) fn tilt_q() -> f32 {
    TILT_Q
}

pub(crate) fn nasal_q() -> f32 {
    NASAL_Q
}

pub(crate) fn section_eps() -> f32 {
    SECTION_EPS
}

fn finite_clamp(x: f32, lo: f32, hi: f32) -> f32 {
    if x.is_finite() {
        x.clamp(lo, hi)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use galion_emotion_core::{
        Breath, BreathPhase, MomentState, VoiceBaseline, APPROACH, AROUSAL, VALENCE,
    };
    use horbm_breath::{controls, Drive, Frame};

    fn rest_moment() -> (VoiceBaseline, MomentState) {
        let voice = VoiceBaseline::standin();
        let moment = MomentState::at(voice.pos);
        (voice, moment)
    }

    #[test]
    fn rest_moment_is_neutral_and_does_not_write_the_baseline() {
        let (voice, moment) = rest_moment();
        let bits = voice.identity_bits();
        let controls = MouthControls::from_moment(&voice, &moment, Breath::rest(), 0.0);
        assert!(controls.is_neutral());
        assert_eq!(controls.breath_noise, 0.0);
        assert_eq!(voice.identity_bits(), bits);
        assert_eq!(moment.pos, voice.pos);
    }

    #[test]
    fn the_moment_does_not_invent_glottal_or_mouth_controls() {
        let (voice, mut moment) = rest_moment();
        moment.pos[1] = 1.0;
        moment.pos[2] = 1.0;
        let controls = MouthControls::from_moment(&voice, &moment, Breath::rest(), 0.0);
        assert_eq!(controls.tension, 0.0);
        assert_eq!(controls.breathiness, 0.0);
        assert_eq!(controls.openness, 0.0);
        assert_eq!(controls.nasal, 0.0);
        assert_eq!(controls.breath_noise, 0.0);
    }

    #[test]
    fn valence_and_approach_do_not_move_a_knob() {
        let (voice, mut moment) = rest_moment();
        moment.pos[VALENCE] = 0.9;
        moment.pos[APPROACH] = -0.8;
        let controls = MouthControls::from_moment(&voice, &moment, Breath::rest(), 0.0);
        assert!(controls.is_neutral(), "{controls:?}");
    }

    #[test]
    fn breath_noise_follows_the_exhale_phase_only() {
        let (voice, moment) = rest_moment();
        let at = |phase, flow| {
            let breath = Breath {
                phase,
                volume: 0.5,
                flow,
            };
            MouthControls::from_moment(&voice, &moment, breath, 0.0).breath_noise
        };
        assert!(at(BreathPhase::Exhale, -0.6) >= 0.6);
        assert!(at(BreathPhase::Exhale, 0.0) >= BREATH_FLOOR);
        assert_eq!(at(BreathPhase::Rest, 0.0), 0.0);
        assert_eq!(at(BreathPhase::Inhale, 0.7), 0.0);
        assert_eq!(at(BreathPhase::Hold, 0.0), 0.0);
        assert_eq!(at(BreathPhase::Speech, -0.4), 0.0);
    }

    #[test]
    fn velum_is_the_only_nasal_input() {
        let (voice, mut moment) = rest_moment();
        moment.pos[AROUSAL] = 1.0;
        let shut = MouthControls::from_moment(&voice, &moment, Breath::rest(), 0.0);
        let open = MouthControls::from_moment(&voice, &moment, Breath::rest(), 0.4);
        assert_eq!(shut.nasal, 0.0);
        assert!((open.nasal - 0.4).abs() < 1.0e-6);
        assert!((open.tension - shut.tension).abs() < 1.0e-6);
    }

    fn frame_of(art: horbm_breath::Articulation, phase: BreathPhase, flow: f32) -> Frame {
        Frame {
            phase,
            volume: 0.55,
            flow,
            pressure: 1.0,
            loudness_db: 0.0,
            loudness_gain: 1.0,
            pause: false,
            open_quotient: art.open_quotient,
            breathiness: art.breathiness,
            nasal_coupling: art.nasal_coupling,
            mouth_openness: art.mouth_openness,
        }
    }

    #[test]
    fn a_breath_frame_is_the_only_glottal_and_mouth_source() {
        let rest_art = controls(&Drive::rest(), BreathPhase::Speech, 0.0, 0.5);
        let rest = MouthControls::from_breath_frame(frame_of(rest_art, BreathPhase::Speech, -0.4));
        assert!(rest.is_neutral(), "{rest:?}");
        assert_eq!(rest.breath_noise, 0.0);

        let mut tired = Drive::rest();
        tired.fatigue = 0.9;
        tired.health = 0.3;
        let tired_art = controls(&tired, BreathPhase::Speech, 0.0, 0.5);
        let tired_controls =
            MouthControls::from_breath_frame(frame_of(tired_art, BreathPhase::Speech, -0.4));
        assert!(tired_controls.breathiness > rest.breathiness);

        let open = controls(&Drive::rest(), BreathPhase::Speech, 1.0, 1.0);
        let shaped = MouthControls::from_breath_frame(frame_of(open, BreathPhase::Speech, 0.0));
        assert!(shaped.nasal > 0.5, "nasal {}", shaped.nasal);
        assert!(shaped.openness > 0.4, "openness {}", shaped.openness);

        let mut pressed = rest_art;
        pressed.open_quotient *= 0.6;
        let tight = MouthControls::from_breath_frame(frame_of(pressed, BreathPhase::Speech, 0.0));
        assert!(tight.tension > 0.2, "tension {}", tight.tension);

        let exhaled =
            MouthControls::from_breath_frame(frame_of(rest_art, BreathPhase::Exhale, -0.8));
        assert!(exhaled.breath_noise >= 0.8);
        let inhaled =
            MouthControls::from_breath_frame(frame_of(rest_art, BreathPhase::Inhale, 0.8));
        assert_eq!(inhaled.breath_noise, 0.0);
    }

    #[test]
    fn applies_to_is_the_configured_id_only() {
        assert!(applies_to("northwind", "northwind"));
        assert!(!applies_to("Northwind", "northwind"));
        assert!(!applies_to("northwind ", "northwind"));
        assert!(!applies_to("parked-fixture", "northwind"));
        assert!(!applies_to("", "northwind"));
        assert!(!applies_to("northwind", ""));
    }

    #[test]
    fn opening_lowers_formant_q() {
        assert!(formant_q(F1_Q0, 1.0) < formant_q(F1_Q0, 0.35));
        assert!((formant_q(F1_Q0, -0.5) - F1_Q0).abs() < 1.0e-5);
    }

    #[test]
    fn frame_length_matches_the_clock() {
        assert_eq!(frame_samples(24_000), 1_920);
        assert_eq!(frame_samples(16_000), 1_280);
        assert_eq!(frame_samples(22_050), 1_764);
        assert_eq!(frame_samples(0), 0);
    }
}
