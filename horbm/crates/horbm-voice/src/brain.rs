// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! The voice asks [`horbm::Brain`] for the spoken line. Nothing else writes it.

use galion_emotion_core::Breath;
use horbm::{Brain, CardLookup, Pad, VoiceBaseline};

use crate::moment_voice::{samantha_baseline, MomentView};

/// One line the mouth may speak, plus the mood the brain already computed.
#[derive(Clone, Debug, PartialEq)]
pub struct SpokenLine {
    pub text: String,
    pub mood: Pad,
    pub rate: f32,
    pub energy: f32,
    pub pitch_st: f32,
    pub pause_ms: f32,
}

/// One brain turn. The voice loop calls this on end of speech and nowhere else.
pub trait TurnBrain {
    fn turn(&mut self, heard: &str) -> SpokenLine;

    /// The moment on this frame, when the brain has one. The default is none,
    /// and the mouth keeps the unsteered packet.
    fn moment_view(&self) -> Option<MomentView> {
        None
    }
}

/// [`horbm::Brain`] behind [`TurnBrain`].
pub struct HorbmTurn {
    brain: Brain,
}

impl HorbmTurn {
    /// Samantha's voice. The moment starts on her baseline. [`Self::seat`]
    /// is how Aria stays parked.
    pub fn new(mut brain: Brain) -> Self {
        brain.seat(samantha_baseline());
        Self { brain }
    }

    pub fn with_cards(cards: impl CardLookup + Send + 'static) -> Self {
        Self::new(Brain::with_cards(cards, None))
    }

    pub fn brain(&self) -> &Brain {
        &self.brain
    }

    pub fn brain_mut(&mut self) -> &mut Brain {
        &mut self.brain
    }

    pub fn seat(&mut self, baseline: VoiceBaseline) {
        self.brain.seat(baseline);
    }
}

impl TurnBrain for HorbmTurn {
    fn turn(&mut self, heard: &str) -> SpokenLine {
        let out = self.brain.turn(heard);
        SpokenLine {
            text: out.reply,
            mood: out.mood,
            rate: out.mood_meta.rate,
            energy: out.mood_meta.energy,
            pitch_st: out.mood_meta.pitch_st,
            pause_ms: out.mood_meta.pause_ms,
        }
    }

    fn moment_view(&self) -> Option<MomentView> {
        let cue = self.brain.conductor_cue();
        Some(MomentView {
            baseline: self.brain.emotion().baseline(),
            moment: self.brain.emotion().moment(),
            breath: Breath {
                phase: cue.breath_phase,
                volume: cue.lung_volume,
                flow: cue.air_flow,
            },
        })
    }
}

#[cfg(all(test, any(feature = "piper", feature = "vosk")))]
pub(crate) fn kettle_turn() -> HorbmTurn {
    HorbmTurn::with_cards(KettleCards)
}

#[cfg(all(test, any(feature = "piper", feature = "vosk")))]
struct KettleCards;

#[cfg(all(test, any(feature = "piper", feature = "vosk")))]
impl CardLookup for KettleCards {
    fn lookup(&self, query: &str) -> Option<horbm::CardHit> {
        if query.to_ascii_lowercase().contains("kettle") {
            Some(horbm::CardHit {
                id: "kettle".into(),
                gist: "The kettle is on the hob.".into(),
            })
        } else {
            None
        }
    }
}
