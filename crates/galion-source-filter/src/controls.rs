// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! Moment deltas, kept off the steady tract.
//!
//! Every gain and every shift below is a knob. None of them is a z against
//! Samantha's own neutral, and none is a hertz figure copied out of a paper.
//! The rest formants are the Burg medians already written down for this
//! speaker. They are anchors for the shift, not a new measurement.

use galion_emotion_core::{Breath, BreathPhase, MomentState, VoiceBaseline, AROUSAL, DOMINANCE};

/// The only persona this mouth shapes.
pub const PERSONA: &str = "samantha";

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

/// True only for the exact persona id `samantha`.
pub fn applies_to(persona: &str) -> bool {
    persona == PERSONA
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
/// `SAMANTHA` uses the Burg medians from the articulation layer note
/// (F1 431.512 Hz, F2 1857.581 Hz). Bandwidths are not taken from that note.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TractBaseline {
    pub f1_hz: f32,
    pub f2_hz: f32,
}

impl TractBaseline {
    pub const SAMANTHA: Self = Self {
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

    /// Deltas from this speaker's rest. Does not write `voice` or `moment`.
    ///
    /// Tension and breathiness share one glottal slope and sit on opposite
    /// sides of it, so a single arousal step does not push both. Openness
    /// follows arousal only. Valence and approach do not move a knob: a
    /// smile is an intent, and this crate does not invent one. Nasal coupling
    /// is `velum` alone. The emotion core has no nasalance, and the nose
    /// planner's feeling gain ships at zero. Breath noise is the exhale
    /// phase, including when the feeling is still on the baseline.
    pub fn from_moment(
        voice: &VoiceBaseline,
        moment: &MomentState,
        breath: Breath,
        velum: f32,
    ) -> Self {
        let d_ar = axis_delta(moment.pos[AROUSAL], voice.pos[AROUSAL]);
        let d_dom = axis_delta(moment.pos[DOMINANCE], voice.pos[DOMINANCE]);
        let raw = finite_clamp(d_ar + 0.5 * d_dom, -1.0, 1.0);
        let breath_noise = match breath.phase {
            BreathPhase::Exhale => {
                let outward = if breath.flow.is_finite() && breath.flow < 0.0 {
                    -breath.flow
                } else {
                    0.0
                };
                outward.max(BREATH_FLOOR).clamp(0.0, 1.0)
            }
            BreathPhase::Rest | BreathPhase::Inhale | BreathPhase::Hold | BreathPhase::Speech => {
                0.0
            }
        };
        Self {
            tension: raw.max(0.0),
            breathiness: (-raw).max(0.0),
            openness: finite_clamp(d_ar, -1.0, 1.0),
            nasal: finite_clamp(velum, 0.0, 1.0),
            breath_noise,
        }
    }
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

fn axis_delta(moment: f32, rest: f32) -> f32 {
    let d = moment - rest;
    if d.is_finite() {
        d
    } else {
        0.0
    }
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
    use galion_emotion_core::{Breath, BreathPhase, MomentState, VoiceBaseline, APPROACH, VALENCE};

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
    fn arousal_splits_tension_openness_and_breathiness() {
        let (voice, mut moment) = rest_moment();
        moment.pos[AROUSAL] = (voice.pos[AROUSAL] + 0.25).min(1.0);
        let up = MouthControls::from_moment(&voice, &moment, Breath::rest(), 0.0);
        assert!(up.tension > 0.0);
        assert!(up.openness > 0.0);
        assert_eq!(up.breathiness, 0.0);
        assert_eq!(up.nasal, 0.0);
        assert_eq!(up.breath_noise, 0.0);

        moment.pos[AROUSAL] = (voice.pos[AROUSAL] - 0.20).max(0.0);
        let down = MouthControls::from_moment(&voice, &moment, Breath::rest(), 0.0);
        assert_eq!(down.tension, 0.0);
        assert!(down.breathiness > 0.0);
        assert!(down.openness < 0.0);
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
    fn dominance_presses_without_opening_the_jaw() {
        let (voice, mut moment) = rest_moment();
        moment.pos[DOMINANCE] = (voice.pos[DOMINANCE] + 0.40).min(1.0);
        let controls = MouthControls::from_moment(&voice, &moment, Breath::rest(), 0.0);
        assert!(controls.tension > 0.0);
        assert_eq!(controls.openness, 0.0);
        assert_eq!(controls.breathiness, 0.0);
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

    #[test]
    fn persona_gate_is_exact() {
        assert!(applies_to("samantha"));
        assert!(!applies_to("Samantha"));
        assert!(!applies_to("jarvis"));
        assert!(!applies_to("samantha "));
        assert!(!applies_to(""));
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
