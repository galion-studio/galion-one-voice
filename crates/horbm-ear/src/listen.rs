// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! How the listener feels, and how the configured persona answers it.
//!
//! The reading is not that persona's [`galion_emotion_core::MomentState`] and
//! not that persona's [`galion_emotion_core::VoiceBaseline`]. Five continuous
//! dimensions, each with its own confidence. A universal score map is a view
//! of valence and arousal. It is not a category and it does not drive the step.
//!
//! Words are [`galion_emotion::text_affect`] and the `LEXICON` table beside
//! it. That table is Perpetual Open MIT, (c) 2026 Galion Labs, used as
//! published. It is a cue list, not a model. When the word and the tone
//! disagree in sign, the tone wins.
//!
//! [`hear_frame`] steps the core with the configured persona's own intent.
//! The ear's inbound offer is left for [`crate::link::push_frame`]. Adapt
//! stays off, so that offer still does not mix. This path never sets adapt,
//! remember, or share. Nothing is written. The persona is whatever
//! [`configured_profile`] resolves. An id with no emotion profile is
//! [`None`]. This layer does not substitute another persona's profile. The
//! caller then keeps the listener reading on the persona-neutral
//! [`UNIVERSAL`] path only.
//!
//! The map below is a fixed coupling. It was not fit on a corpus. Louder,
//! higher, and faster raise arousal, which is the direction Juslin and
//! Laukka (Psychological Bulletin, 2003) treat as the reliable vocal cue.
//! Roughness raises tension. A low, quiet voice raises fatigue and lowers
//! valence. The stand-in baseline centre (216 Hz) is not the listener's
//! neutral.

use galion_emotion::consent_bits;
use galion_emotion::persona_mood::text_affect;
use galion_emotion_core::{
    project_universal, CoreError, EmotionCore, IntentMsg, IntentSource, MomentState, APPROACH,
    AROUSAL, DOMINANCE, UNIVERSAL_LEN, VALENCE,
};

pub use galion_emotion::persona_mood::{
    appraise, ObsSource, Persona, PersonaProfile, UserObservation,
};
pub use galion_emotion_core::UNIVERSAL;

use crate::link::{FloorCue, FloorTick};
use crate::{Ear, FrameOut, ProsodyFrame};

/// Process environment that names the persona id. [`profile_from_env`] reads
/// it once per call. Unset means no profile.
pub const DEV_PERSONA_ENV: &str = "GALION_HORBM_DEV_PERSONA";

/// Verb on the persona's own response. Not an emotion name.
pub const RESPONSE_VERB: &str = "with";

/// Dimension names. A new axis would be another name here, not a new emotion.
pub const DIMENSIONS: [&str; 5] = ["valence", "arousal", "tension", "fatigue", "confidence"];

/// Pull clamp. The core's `PULL_LIMIT` is 2 and is not re-exported.
const PULL_CLAMP: f32 = 2.0;

/// Same silence gate as the ear's hop RMS. Below this, and when the frame
/// is unvoiced, there is no listener evidence.
const ACOUSTIC_FLOOR: f32 = 0.012;

/// Listener pitch band, hertz. Not the persona baseline centre.
const F0_LO: f32 = 90.0;
const F0_HI: f32 = 280.0;
/// Unit pitch of a neutral adult tone inside [`F0_LO`]..[`F0_HI`].
const F0_NEUTRAL: f32 = 0.45;

const ENERGY_LO: f32 = 0.012;
const ENERGY_HI: f32 = 0.22;
const ENERGY_NEUTRAL: f32 = 0.40;

const RATE_FAST_LO: f32 = 2.0;
const RATE_FAST_HI: f32 = 6.0;

const PAUSE_HI_MS: f32 = 800.0;
const JITTER_HI: f32 = 0.08;
const SHIMMER_HI: f32 = 0.25;

/// Word and tone must both clear this before a sign clash drops the word.
const WORD_SIGN: f32 = 0.15;
const PROSODY_SIGN: f32 = 0.12;
const PROSODY_WEIGHT: f32 = 0.75;
const WORD_WEIGHT: f32 = 0.25;

/// One dimension and how much this frame supports it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reading {
    pub value: f32,
    pub confidence: f32,
}

impl Reading {
    const fn rest(value: f32) -> Self {
        Self {
            value,
            confidence: 0.0,
        }
    }
}

/// The listener, this frame. Not the persona's moment. Not the persona's
/// baseline. Not a store: the caller drops it by dropping the value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ListenerFeel {
    /// Pleasure, −1..=1.
    pub valence: Reading,
    /// Activation, 0..=1. 0.5 is the circumplex origin.
    pub arousal: Reading,
    /// Pressed, rough, strained. 0..=1.
    pub tension: Reading,
    /// Low, quiet, drawn out. 0..=1. Not the persona's body fatigue factor.
    pub fatigue: Reading,
    /// How assured the voice sounds, 0..=1. [`Reading::confidence`] is how
    /// sure the reading is, which is a different number.
    pub confidence: Reading,
    /// Universal scores of valence and arousal. Not a distribution.
    pub scores: [f32; UNIVERSAL_LEN],
    /// True when the word and the tone disagreed and the tone was kept.
    pub prosody_led: bool,
}

impl ListenerFeel {
    /// No evidence. Every confidence is 0. Valence is 0, arousal is the
    /// circumplex origin, the other dimensions are 0.
    pub fn neutral() -> Self {
        let valence = 0.0;
        let arousal = 0.5;
        Self {
            valence: Reading::rest(valence),
            arousal: Reading::rest(arousal),
            tension: Reading::rest(0.0),
            fatigue: Reading::rest(0.0),
            confidence: Reading::rest(0.0),
            scores: project_scores(valence, arousal),
            prosody_led: false,
        }
    }

    /// True when no dimension is claimed. The off switch is this, and so is
    /// a frame with nothing voiced.
    pub fn unclaimed(self) -> bool {
        zero(self.valence.confidence)
            && zero(self.arousal.confidence)
            && zero(self.tension.confidence)
            && zero(self.fatigue.confidence)
            && zero(self.confidence.confidence)
            && !self.prosody_led
    }
}

fn zero(x: f32) -> bool {
    x.to_bits() == 0
}

/// What one call of [`hear_frame`] did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HearOut {
    pub frame: FrameOut,
    pub listener: ListenerFeel,
    pub moment: Option<MomentState>,
    pub cue: Option<FloorCue>,
}

/// Prosody plus the ASR words for this frame.
///
/// `words` is the transcript already in hand. This function does not decode
/// speech. A frame with no voicing returns [`ListenerFeel::neutral`] and
/// does not consult the words. An empty string is prosody alone.
pub fn listener_feel(frame: &ProsodyFrame, words: &str) -> ListenerFeel {
    if !frame.voiced || frame.energy < ACOUSTIC_FLOOR {
        return ListenerFeel::neutral();
    }
    let cues = Cues::from_frame(frame);
    let prosody_v = cues.valence();
    let arousal = cues.arousal();
    let tension = cues.tension();
    let fatigue = cues.fatigue();
    let vocal = cues.vocal(frame.voicing);
    let evidence = (0.55 + 0.45 * frame.voicing.clamp(0.0, 1.0)).clamp(0.0, 1.0);
    let word = text_affect(words);
    let (valence, prosody_led, valence_conf) =
        fuse_valence(prosody_v, word.valence, word.confidence, evidence);
    ListenerFeel {
        valence: Reading {
            value: valence,
            confidence: valence_conf,
        },
        arousal: Reading {
            value: arousal,
            confidence: evidence,
        },
        tension: Reading {
            value: tension,
            confidence: evidence,
        },
        fatigue: Reading {
            value: fatigue,
            confidence: evidence,
        },
        confidence: Reading {
            value: vocal,
            confidence: evidence,
        },
        scores: project_scores(valence, arousal),
        prosody_led,
    }
}

/// Emotion profile for a configured persona id.
///
/// Pure. Resolves `id` through [`Persona::parse`] and [`Persona::profile`].
/// [`None`], a blank id, and an id with no emotion profile are [`None`].
/// There is no fallback to another persona. When this returns [`None`], the
/// caller keeps [`listener_feel`] on the persona-neutral [`UNIVERSAL`] path
/// and does not invent a profile for [`response_intent`] or [`hear_frame`].
pub fn configured_profile(id: Option<&str>) -> Option<&'static PersonaProfile> {
    let id = id?;
    Persona::parse(id).map(Persona::profile)
}

/// [`configured_profile`] of [`DEV_PERSONA_ENV`], read once per call.
///
/// Unset, blank, or an id with no emotion profile is [`None`]. This does not
/// read a file and does not substitute another persona's profile. The caller
/// then keeps the listener reading on the persona-neutral [`UNIVERSAL`] path
/// only.
pub fn profile_from_env() -> Option<&'static PersonaProfile> {
    let id = std::env::var(DEV_PERSONA_ENV).ok();
    configured_profile(id.as_deref())
}

/// The persona's own intent for this reading.
///
/// `profile` is the configured emotion profile. Factor mask stays 0: the
/// listener's fatigue is not written into the persona's body. Consent is the
/// sense bit only.
pub fn response_intent(
    feel: &ListenerFeel,
    t_ms: u32,
    profile: &PersonaProfile,
) -> Result<IntentMsg, CoreError> {
    let obs = UserObservation {
        valence: feel.valence.value,
        arousal: feel.arousal.value,
        dominance: 0.5,
        confidence: feel.valence.confidence,
        source: ObsSource::Fused,
    };
    let mood = appraise(profile, &obs);
    let rest = profile.dynamics.rest;
    let strength = feel
        .valence
        .confidence
        .max(feel.arousal.confidence)
        .max(feel.tension.confidence)
        .clamp(0.0, 1.0);
    let mut msg = IntentMsg::hold(t_ms);
    msg.source = IntentSource::Own;
    msg.consent = consent_bits::SENSE;
    msg.set_verb(RESPONSE_VERB)?;
    msg.strength = strength;
    let valence =
        (mood.valence - rest.valence - 0.10 * feel.fatigue.value).clamp(-PULL_CLAMP, PULL_CLAMP);
    let arousal = (mood.arousal - rest.arousal + 0.20 * feel.tension.value
        - 0.12 * feel.fatigue.value)
        .clamp(-PULL_CLAMP, PULL_CLAMP);
    let dominance = (mood.dominance - rest.dominance).clamp(-PULL_CLAMP, PULL_CLAMP);
    let approach = if feel.valence.value < -0.15 {
        (0.30 * (-feel.valence.value)).clamp(0.0, PULL_CLAMP)
    } else if feel.valence.value > 0.20 {
        (0.15 * feel.valence.value).clamp(0.0, PULL_CLAMP)
    } else {
        0.0
    };
    msg.set_pull(VALENCE, valence)?;
    msg.set_pull(AROUSAL, arousal)?;
    msg.set_pull(DOMINANCE, dominance)?;
    msg.set_pull(APPROACH, approach)?;
    msg.factor_mask = 0;
    msg.encode()?;
    Ok(msg)
}

/// Hear one frame, keep the listener reading apart, and step the persona.
///
/// `profile` is the configured emotion profile ([`configured_profile`],
/// [`profile_from_env`]). This function does not look up a persona and does
/// not substitute one. Sensing off ([`Ear::deny`], [`crate::EarConfig`] with
/// `sense: false`, [`Ear::from_env`] on `0` / `off` / `deny`) returns
/// [`ListenerFeel::neutral`] and does not step the core. [`Ear::push`] does
/// not read the samples on that path. The words are ignored. Workplace and
/// education are already shut by the ear, so they take the same path.
///
/// The baseline is not an argument. The core already holds it, and this
/// function does not write it.
pub fn hear_frame(
    ear: &mut Ear,
    core: &mut EmotionCore,
    floor: &mut dyn FloorTick,
    pcm: &[f32],
    words: &str,
    profile: &PersonaProfile,
) -> Result<HearOut, CoreError> {
    if !ear.sensing() {
        let frame = ear.push(pcm);
        floor.tick_floor(FloorCue::Quiet);
        return Ok(HearOut {
            frame,
            listener: ListenerFeel::neutral(),
            moment: None,
            cue: Some(FloorCue::Quiet),
        });
    }
    match ear.push(pcm) {
        FrameOut::Denied => {
            floor.tick_floor(FloorCue::Quiet);
            Ok(HearOut {
                frame: FrameOut::Denied,
                listener: ListenerFeel::neutral(),
                moment: None,
                cue: Some(FloorCue::Quiet),
            })
        }
        FrameOut::Rejected => Ok(HearOut {
            frame: FrameOut::Rejected,
            listener: ListenerFeel::neutral(),
            moment: None,
            cue: None,
        }),
        FrameOut::Heard(heard) => {
            let listener = listener_feel(&heard, words);
            let cue = if heard.voiced {
                FloorCue::Voiced
            } else {
                FloorCue::Quiet
            };
            let msg = response_intent(&listener, core.next_ms(), profile)?;
            let moment = core.step(&msg)?;
            floor.tick_floor(cue);
            Ok(HearOut {
                frame: FrameOut::Heard(heard),
                listener,
                moment: Some(moment),
                cue: Some(cue),
            })
        }
    }
}

fn fuse_valence(
    prosody: f32,
    word: Option<f32>,
    word_conf: f32,
    evidence: f32,
) -> (f32, bool, f32) {
    let Some(word) = word else {
        return (prosody, false, evidence);
    };
    let disagree = word * prosody < 0.0 && word.abs() >= WORD_SIGN && prosody.abs() >= PROSODY_SIGN;
    if disagree {
        (prosody, true, evidence)
    } else {
        let mixed = (PROSODY_WEIGHT * prosody + WORD_WEIGHT * word).clamp(-1.0, 1.0);
        let conf = (evidence + 0.15 * word_conf).clamp(0.0, 1.0);
        (mixed, false, conf)
    }
}

fn project_scores(valence: f32, arousal: f32) -> [f32; UNIVERSAL_LEN] {
    let view = MomentState {
        t_ms: 0,
        tick: 0,
        pos: [valence, arousal, 0.5, 0.0],
        vel: [0.0; 4],
    };
    project_universal(&view)
}

struct Cues {
    pitch: f32,
    energy: f32,
    rate: f32,
    pause: f32,
    rough: f32,
    rate_hz: f32,
}

impl Cues {
    fn from_frame(frame: &ProsodyFrame) -> Self {
        let pitch = if frame.voiced {
            unit(frame.f0_hz, F0_LO, F0_HI)
        } else {
            F0_NEUTRAL
        };
        let energy = unit(frame.energy, ENERGY_LO, ENERGY_HI);
        let rate = if frame.speaking_rate_hz >= RATE_FAST_LO {
            unit(frame.speaking_rate_hz, RATE_FAST_LO, RATE_FAST_HI)
        } else {
            0.0
        };
        let pause = unit(frame.pause_ms as f32, 0.0, PAUSE_HI_MS);
        let rough =
            0.5 * (unit(frame.jitter, 0.0, JITTER_HI) + unit(frame.shimmer, 0.0, SHIMMER_HI));
        Self {
            pitch,
            energy,
            rate,
            pause,
            rough: rough.clamp(0.0, 1.0),
            rate_hz: frame.speaking_rate_hz,
        }
    }

    fn valence(&self) -> f32 {
        (0.70 * (self.pitch - F0_NEUTRAL)
            + 0.55 * (self.energy - ENERGY_NEUTRAL)
            + 0.30 * self.rate
            - 0.35 * self.pause
            - 0.25 * self.rough)
            .clamp(-1.0, 1.0)
    }

    fn arousal(&self) -> f32 {
        (0.50
            + 0.38 * (self.energy - ENERGY_NEUTRAL)
            + 0.32 * (self.pitch - F0_NEUTRAL)
            + 0.22 * self.rate
            - 0.28 * self.pause)
            .clamp(0.0, 1.0)
    }

    fn tension(&self) -> f32 {
        (0.85 * self.rough + 0.15 * self.pitch * self.energy * self.rough).clamp(0.0, 1.0)
    }

    fn fatigue(&self) -> f32 {
        let low_e = ((ENERGY_NEUTRAL - self.energy).max(0.0) / ENERGY_NEUTRAL).clamp(0.0, 1.0);
        let low_f = ((F0_NEUTRAL - self.pitch).max(0.0) / F0_NEUTRAL).clamp(0.0, 1.0);
        let slow = quiet_slow(self.energy, self.rate_hz);
        (0.50 * low_e + 0.35 * low_f + 0.25 * self.pause + 0.20 * slow).clamp(0.0, 1.0)
    }

    fn vocal(&self, voicing: f32) -> f32 {
        let level = unit_from_norm(self.energy);
        let clear = (1.0 - self.rough).clamp(0.0, 1.0);
        let steady = (1.0 - self.pause).clamp(0.0, 1.0);
        (voicing.clamp(0.0, 1.0) * level * clear * steady).clamp(0.0, 1.0)
    }
}

/// Quiet and not fast. A mid-level steady tone is not this.
fn quiet_slow(energy: f32, rate_hz: f32) -> f32 {
    if energy >= 0.25 {
        return 0.0;
    }
    let sparse = (1.0 - unit(rate_hz, 0.0, RATE_FAST_LO)).clamp(0.0, 1.0);
    let quiet = (1.0 - energy / 0.25).clamp(0.0, 1.0);
    sparse * quiet
}

/// Normalised energy back to RMS, then to a level where a quiet voice is
/// near 0 and a clear voice is near 1.
fn unit_from_norm(energy_unit: f32) -> f32 {
    // 0.02 RMS is just above the floor (unit ~0.04). 0.14 RMS is a clear tone
    // (unit ~0.62). Below the low end the voice does not sound assured.
    let rms = ENERGY_LO + energy_unit * (ENERGY_HI - ENERGY_LO);
    unit(rms, 0.02, 0.14)
}

fn unit(x: f32, lo: f32, hi: f32) -> f32 {
    if !x.is_finite() || hi <= lo {
        return 0.0;
    }
    ((x - lo) / (hi - lo)).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(f0: f32, energy: f32, rate: f32, jitter: f32, shimmer: f32) -> ProsodyFrame {
        ProsodyFrame {
            f0_hz: f0,
            voiced: true,
            voicing: 0.9,
            energy,
            speaking_rate_hz: rate,
            pause_ms: 0,
            jitter,
            shimmer,
            offer: crate::ListenerOffer {
                t_ms: 0,
                consent: consent_bits::SENSE,
                point: [0.0, 0.5, 0.5, 0.0],
                strength: 0.5,
            },
        }
    }

    #[test]
    fn fine_is_positive_and_a_low_tone_drops_it() {
        let word = text_affect("I'm fine");
        let wv = word.valence.expect("fine is in the lexicon");
        assert!(wv > 0.15, "word valence {wv}");
        let low = listener_feel(&tone(105.0, 0.028, 0.4, 0.0, 0.0), "I'm fine");
        assert!(low.prosody_led);
        assert!(low.valence.value < 0.0, "valence {}", low.valence.value);
        assert!(low.valence.value < wv);
        let high = listener_feel(&tone(260.0, 0.16, 3.5, 0.0, 0.0), "I'm fine");
        assert!(!high.prosody_led);
        assert!(high.valence.value > 0.2, "valence {}", high.valence.value);
        assert!(low.valence.value < high.valence.value);
    }

    #[test]
    fn standin_pitch_centre_is_not_the_listener_zero() {
        let mid = listener_feel(&tone(176.0, 0.095, 0.4, 0.0, 0.0), "");
        let centre = listener_feel(&tone(216.0, 0.095, 0.4, 0.0, 0.0), "");
        assert!(
            centre.valence.value > mid.valence.value + 0.05,
            "216 Hz {} vs 176 Hz {}",
            centre.valence.value,
            mid.valence.value
        );
    }

    #[test]
    fn silence_ignores_the_words() {
        let quiet = ProsodyFrame {
            f0_hz: 0.0,
            voiced: false,
            voicing: 0.0,
            energy: 0.0,
            speaking_rate_hz: 0.0,
            pause_ms: 80,
            jitter: 0.0,
            shimmer: 0.0,
            offer: crate::ListenerOffer {
                t_ms: 0,
                consent: 0,
                point: [0.0; 4],
                strength: 0.0,
            },
        };
        let feel = listener_feel(&quiet, "I'm fine");
        assert_eq!(feel, ListenerFeel::neutral());
        assert!(feel.unclaimed());
    }
}
