// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Emotion port over `galion_emotion_core::EmotionCore`.
//!
//! One [`Emotion::step`] is one `EmotionCore::step` on the same 80 ms clock.
//! The baseline is copied in once and the step does not write it.
//! [`CoreAffect::from_baseline`] seats the configured persona's baseline.
//! Those numbers are not a measured voice.

use galion_emotion_core::{
    Breath, EmotionCore, IntentMsg, VoiceBaseline, AROUSAL, DOMINANCE, VALENCE,
};
use horbm_voice::MomentView;

use crate::ports::{Affect, Emotion};

/// The loop's affect. The core lives here. The brain's own core is a
/// separate call (`Brain::step_moment`) and this tick does not use it.
pub struct CoreAffect {
    core: EmotionCore,
    last: Affect,
}

impl CoreAffect {
    /// Seat the affect on a persona baseline. The step does not write it.
    #[must_use]
    pub fn from_baseline(baseline: VoiceBaseline) -> Self {
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

    /// Baseline, moment, and breath for the mouth. The baseline bits are
    /// the ones this affect was seated on. This does not write them.
    #[must_use]
    pub fn view(&self) -> MomentView {
        let cue = self.core.conductor_cue();
        MomentView {
            baseline: self.core.baseline(),
            moment: self.core.moment(),
            breath: Breath {
                phase: cue.breath_phase,
                volume: cue.lung_volume,
                flow: cue.air_flow,
            },
        }
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

    fn moment_view(&self) -> Option<MomentView> {
        Some(self.view())
    }
}

fn affect_of(baseline: &VoiceBaseline) -> Affect {
    Affect {
        valence: baseline.pos[VALENCE],
        arousal: baseline.pos[AROUSAL],
        dominance: baseline.pos[DOMINANCE],
    }
}
