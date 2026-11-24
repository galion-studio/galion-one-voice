// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Emotion port over `galion_emotion_core::EmotionCore`.
//!
//! One [`Emotion::step`] is one `EmotionCore::step` on the same 80 ms clock.
//! The baseline is copied in once and the step does not write it. The point
//! is the published stand-in rest, with the speaker id set to Samantha.
//! Those numbers are not a measured voice.

use galion_emotion_core::{EmotionCore, IntentMsg, VoiceBaseline, AROUSAL, DOMINANCE, VALENCE};

use crate::ports::{Affect, Emotion};

/// The loop's affect. The core lives here. The brain's own core is a
/// separate call (`Brain::step_moment`) and this tick does not use it.
pub struct CoreAffect {
    core: EmotionCore,
    last: Affect,
}

impl CoreAffect {
    /// Samantha's id on the published rest point.
    #[must_use]
    pub fn samantha() -> Self {
        let baseline = samantha_baseline();
        let last = affect_of(&baseline);
        Self {
            core: EmotionCore::from_baseline(baseline),
            last,
        }
    }

    #[must_use]
    pub fn baseline(&self) -> VoiceBaseline {
        self.core.baseline()
    }

    #[must_use]
    pub fn frames(&self) -> u32 {
        self.core.frames()
    }

    /// One intent on the core clock. An off-clock intent leaves the core
    /// where it was and returns the previous affect.
    pub fn step_intent(&mut self, intent: &IntentMsg) -> Affect {
        if let Ok(moment) = self.core.step(intent) {
            self.last = Affect {
                valence: moment.pos[VALENCE],
                arousal: moment.pos[AROUSAL],
                dominance: moment.pos[DOMINANCE],
            };
        }
        self.last
    }
}

impl Emotion for CoreAffect {
    fn step(&mut self, t_ms: u32) -> Affect {
        self.step_intent(&IntentMsg::steady(t_ms))
    }
}

fn samantha_baseline() -> VoiceBaseline {
    let rest = VoiceBaseline::standin();
    VoiceBaseline {
        persona: slot("samantha"),
        voice: slot("samantha"),
        pos: rest.pos,
        pitch_centre_hz: rest.pitch_centre_hz,
    }
}

fn affect_of(baseline: &VoiceBaseline) -> Affect {
    Affect {
        valence: baseline.pos[VALENCE],
        arousal: baseline.pos[AROUSAL],
        dominance: baseline.pos[DOMINANCE],
    }
}

fn slot(text: &str) -> [u8; 16] {
    let mut out = [0u8; 16];
    let bytes = text.as_bytes();
    let n = bytes.len().min(out.len());
    out[..n].copy_from_slice(&bytes[..n]);
    out
}
