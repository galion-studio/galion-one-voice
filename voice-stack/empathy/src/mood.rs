// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Companion mood point. One step per 80 ms, from emotion-in's target and
//! the ASR lexicon already in `galion-emotion`. The point is hers: anger is
//! answered, not copied. `e` is [`empathy_weight`]. Joy and calm stay at 0.

use galion_emotion::empathy_weight;
use galion_emotion::express::StubExpressor;
use galion_emotion::frame::{Affect, EmotionFrame};
use galion_emotion::persona_mood::{text_affect, ObsSource, UserObservation};
use galion_emotion::EmpathyPolicy;

use crate::scan::for_each_word;

/// Heartbeat. `LOCKS.toml` `frame_ms`, `galion-emotion` `FRAME_MS`.
pub const FRAME_MS: u32 = 80;

/// Fraction of the gap closed on one 80 ms tick. Same shape as
/// `galion_emotion_in::Pad::TICK_ALPHA`. A design choice of this layer,
/// not a measured HORBM constant.
pub const MOOD_ALPHA: f32 = 0.18;

/// Swarm streaming bar for one mood update, in microseconds. A ceiling.
/// The budget test measures a run and compares it with this. The ceiling
/// is not itself a measurement.
pub const MOOD_BUDGET_US: u32 = 80_000;

/// Confidence below this is not a feeling. Same cutoff as emotion-in and
/// [`EmotionFrame::label`].
const LABEL_FLOOR: f32 = 0.20;

/// Arousal, valence, dominance. Rest matches the HORBM pad.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MoodPoint {
    pub arousal: f32,
    pub valence: f32,
    pub dominance: f32,
}

impl MoodPoint {
    pub const RESTING: Self = Self {
        arousal: 0.3,
        valence: 0.25,
        dominance: 0.5,
    };

    pub fn clamped(self) -> Self {
        let c = |x: f32, lo: f32, hi: f32, d: f32| {
            if x.is_finite() {
                x.clamp(lo, hi)
            } else {
                d
            }
        };
        Self {
            arousal: c(self.arousal, 0.0, 1.0, Self::RESTING.arousal),
            valence: c(self.valence, -1.0, 1.0, Self::RESTING.valence),
            dominance: c(self.dominance, 0.0, 1.0, Self::RESTING.dominance),
        }
    }

    pub fn step_toward(self, target: Self, alpha: f32) -> Self {
        let a = if alpha.is_finite() {
            alpha.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let t = target.clamped();
        let s = self.clamped();
        Self {
            arousal: s.arousal + (t.arousal - s.arousal) * a,
            valence: s.valence + (t.valence - s.valence) * a,
            dominance: s.dominance + (t.dominance - s.dominance) * a,
        }
        .clamped()
    }

    pub fn to_prosody_pad(self) -> galion_sam_prosody::Pad {
        let p = self.clamped();
        galion_sam_prosody::Pad {
            arousal: p.arousal,
            valence: p.valence,
            dominance: p.dominance,
        }
    }
}

impl Default for MoodPoint {
    fn default() -> Self {
        Self::RESTING
    }
}

/// One name for a feeling. A single word. Returned only when the caller
/// invited a name. Not a sentence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FeelingName {
    Sad,
    Tired,
    Anxious,
    Angry,
    Calm,
    Joy,
    Excited,
}

impl FeelingName {
    pub fn word(self) -> &'static str {
        match self {
            Self::Sad => "sad",
            Self::Tired => "tired",
            Self::Anxious => "anxious",
            Self::Angry => "angry",
            Self::Calm => "calm",
            Self::Joy => "joy",
            Self::Excited => "excited",
        }
    }

    pub fn from_affect(affect: Affect) -> Option<Self> {
        Some(match affect {
            Affect::Sad => Self::Sad,
            Affect::Tired => Self::Tired,
            Affect::Anxious => Self::Anxious,
            Affect::Angry => Self::Angry,
            Affect::Calm => Self::Calm,
            Affect::Joy => Self::Joy,
            Affect::Excited => Self::Excited,
            Affect::Neutral => return None,
        })
    }
}

/// Voice side of one tick, taken from [`galion_emotion_in::Reading`].
///
/// `arousal`, `valence`, and `dominance` are that crate's mood target.
/// The target is the reduction of its live prosody (pitch, energy, rate,
/// pauses). This layer does not run a second estimator on the samples.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InboundSignal {
    pub arousal: f32,
    pub valence: f32,
    pub dominance: f32,
    pub confidence: f32,
}

impl InboundSignal {
    pub fn from_reading(reading: galion_emotion_in::Reading) -> Self {
        let pad = reading.mood_target();
        Self {
            arousal: pad.arousal,
            valence: pad.valence,
            dominance: pad.dominance,
            confidence: reading.mapped.confidence,
        }
    }
}

/// Result of one mood step. `cue` is the last feeling-word in the ASR text.
/// The public tick hides it unless the caller invited a name.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MoodStep {
    pub mood: MoodPoint,
    pub e: f32,
    pub confidence: f32,
    pub valence: f32,
    pub arousal: f32,
    pub dominance: f32,
    pub cue: Option<FeelingName>,
}

pub(crate) fn step_mood(
    current: MoodPoint,
    inbound: Option<InboundSignal>,
    asr: &str,
    frame_ms: u32,
) -> MoodStep {
    let voice = inbound.and_then(frame_from_inbound);
    let text = if asr.is_empty() {
        None
    } else {
        Some(text_affect(asr))
    };
    let mut obs = UserObservation::fuse(voice.as_ref(), true, text);
    let cue = if asr.is_empty() { None } else { last_cue(asr) };
    if obs.source == ObsSource::Text {
        if let Some(cue) = cue {
            obs.dominance = dominance_of(cue);
        }
    }
    let frame = obs.as_frame();
    let e = if frame.confidence < LABEL_FLOOR {
        0.0
    } else {
        empathy_weight(Some(&frame))
    };
    let target = target_of(&frame);
    let alpha = if frame_ms == 0 {
        0.0
    } else {
        (MOOD_ALPHA * (frame_ms as f32 / FRAME_MS as f32)).clamp(0.0, 1.0)
    };
    MoodStep {
        mood: current.step_toward(target, alpha),
        e,
        confidence: frame.confidence,
        valence: frame.valence,
        arousal: frame.arousal,
        dominance: frame.dominance,
        cue,
    }
}

fn frame_from_inbound(signal: InboundSignal) -> Option<EmotionFrame> {
    if !signal.confidence.is_finite() || signal.confidence < LABEL_FLOOR {
        return None;
    }
    let rest_v = galion_emotion_in::Pad::RESTING.valence;
    let dv = signal.valence - rest_v;
    let span = if dv >= 0.0 {
        1.0 - rest_v
    } else {
        rest_v - (-1.0)
    };
    let valence = if span > 0.0 { dv / span } else { 0.0 };
    Some(
        EmotionFrame {
            seq: 0,
            t_ms: 0,
            dur_ms: FRAME_MS as u16,
            valence,
            arousal: signal.arousal,
            dominance: signal.dominance,
            confidence: signal.confidence,
        }
        .clamped(),
    )
}

fn target_of(frame: &EmotionFrame) -> MoodPoint {
    if frame.confidence < LABEL_FLOOR {
        return MoodPoint::RESTING;
    }
    match frame.label() {
        Affect::Neutral | Affect::Calm => MoodPoint::RESTING,
        _ => {
            let policy = StubExpressor.respond(Some(frame));
            MoodPoint {
                arousal: policy.arousal,
                valence: policy.valence,
                dominance: policy.dominance,
            }
            .clamped()
        }
    }
}

fn dominance_of(name: FeelingName) -> f32 {
    match name {
        FeelingName::Angry => 0.75,
        FeelingName::Anxious => 0.30,
        FeelingName::Tired | FeelingName::Sad => 0.40,
        FeelingName::Joy | FeelingName::Excited => 0.55,
        FeelingName::Calm => 0.50,
    }
}

const CUES: &[(&str, FeelingName)] = &[
    ("furious", FeelingName::Angry),
    ("angry", FeelingName::Angry),
    ("anger", FeelingName::Angry),
    ("annoyed", FeelingName::Angry),
    ("hate", FeelingName::Angry),
    ("anxious", FeelingName::Anxious),
    ("worried", FeelingName::Anxious),
    ("scared", FeelingName::Anxious),
    ("afraid", FeelingName::Anxious),
    ("panic", FeelingName::Anxious),
    ("stressed", FeelingName::Anxious),
    ("exhausted", FeelingName::Tired),
    ("tired", FeelingName::Tired),
    ("sleepy", FeelingName::Tired),
    ("drained", FeelingName::Tired),
    ("depressed", FeelingName::Sad),
    ("lonely", FeelingName::Sad),
    ("miserable", FeelingName::Sad),
    ("crying", FeelingName::Sad),
    ("hurt", FeelingName::Sad),
    ("sad", FeelingName::Sad),
    ("excited", FeelingName::Joy),
    ("amazing", FeelingName::Joy),
    ("awesome", FeelingName::Joy),
    ("fantastic", FeelingName::Joy),
    ("happy", FeelingName::Joy),
    ("glad", FeelingName::Joy),
    ("love", FeelingName::Joy),
    ("peaceful", FeelingName::Calm),
    ("relaxed", FeelingName::Calm),
    ("calm", FeelingName::Calm),
];

const NEGATORS: &[&str] = &[
    "not", "no", "never", "don't", "dont", "isn't", "isnt", "wasn't", "can't", "cant",
];

pub(crate) fn last_cue(text: &str) -> Option<FeelingName> {
    let mut prev_neg = false;
    let mut found = None;
    for_each_word(text, |word| {
        let neg = NEGATORS.iter().any(|n| n.as_bytes() == word);
        if !neg && !prev_neg {
            if let Some(name) = CUES
                .iter()
                .find(|(k, _)| k.as_bytes() == word)
                .map(|(_, n)| *n)
            {
                found = Some(name);
            }
        }
        prev_neg = neg;
    });
    found
}

pub(crate) fn invitation(text: &str) -> bool {
    const PHRASES: &[&str] = &[
        "how do i feel",
        "how am i feeling",
        "what am i feeling",
        "what do i feel",
        "name the feeling",
        "name that feeling",
        "name this feeling",
        "what emotion",
        "what feeling",
        "what do you think i feel",
        "how do you think i feel",
    ];
    PHRASES
        .iter()
        .any(|p| crate::scan::contains_phrase(text, p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_stays_at_rest_and_a_zero_frame_does_not_move() {
        let step = step_mood(MoodPoint::RESTING, None, "", FRAME_MS);
        assert_eq!(step.mood, MoodPoint::RESTING);
        assert_eq!(step.e, 0.0);
        let wild = MoodPoint {
            arousal: 0.9,
            valence: 0.8,
            dominance: 0.2,
        };
        let held = step_mood(wild, None, "", 0);
        assert_eq!(held.mood, wild.clamped());
    }

    #[test]
    fn anger_is_answered_and_joy_does_not_raise_e() {
        let mut mood = MoodPoint::RESTING;
        let mut e = 0.0;
        for _ in 0..40 {
            let step = step_mood(mood, None, "I am angry", FRAME_MS);
            mood = step.mood;
            e = step.e;
        }
        assert!(e > 0.2, "{e}");
        assert!(mood.valence > 0.0, "{}", mood.valence);
        assert!(mood.valence < 0.35, "{}", mood.valence);
        assert!(mood.arousal < MoodPoint::RESTING.arousal);

        let joy = step_mood(MoodPoint::RESTING, None, "I am happy", FRAME_MS);
        assert_eq!(joy.e, 0.0);
        assert!(joy.mood.valence >= MoodPoint::RESTING.valence);
    }
}
