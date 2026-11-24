// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! The voice asks [`horbm::Brain`] for the spoken line. Nothing else writes it.

use horbm::{Brain, CardLookup, Pad};

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
}

/// [`horbm::Brain`] behind [`TurnBrain`].
pub struct HorbmTurn {
    brain: Brain,
}

impl HorbmTurn {
    pub fn new(brain: Brain) -> Self {
        Self { brain }
    }

    pub fn with_cards(cards: impl CardLookup + Send + 'static) -> Self {
        Self {
            brain: Brain::with_cards(cards, None),
        }
    }

    pub fn brain_mut(&mut self) -> &mut Brain {
        &mut self.brain
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
