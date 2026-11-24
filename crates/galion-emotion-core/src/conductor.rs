// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! What the conductor reads. The conductor does not own the state and does
//! not decide the feeling. Floor control stays with the conductor.

use crate::axes::{AROUSAL, AXES, DOMINANCE, VALENCE};
use crate::factors::BreathPhase;
use crate::state::EmotionCore;
use crate::wire::{may_remember, may_share};

/// One look at the core. All `Copy`. Safe to take on the frame that just stepped.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConductorCue {
    pub t_ms: u32,
    pub tick: u32,
    pub next_ms: u32,
    pub baseline: [f32; AXES],
    pub moment: [f32; AXES],
    pub flow: [f32; AXES],
    pub energy: f32,
    pub fatigue: f32,
    pub health: f32,
    pub breath_phase: BreathPhase,
    pub lung_volume: f32,
    pub air_flow: f32,
    pub consent: u8,
    /// True when this frame mixed an inbound pull into the moment.
    pub inbound_mixed: bool,
    pub may_share: bool,
    pub may_remember: bool,
}

impl ConductorCue {
    pub fn valence(&self) -> f32 {
        self.moment[VALENCE]
    }
    pub fn arousal(&self) -> f32 {
        self.moment[AROUSAL]
    }
    pub fn dominance(&self) -> f32 {
        self.moment[DOMINANCE]
    }
}

impl EmotionCore {
    pub fn conductor_cue(&self) -> ConductorCue {
        let inside = self.inside();
        ConductorCue {
            t_ms: self.moment().t_ms,
            tick: self.moment().tick,
            next_ms: self.next_ms(),
            baseline: self.baseline().pos,
            moment: self.moment().pos,
            flow: self.moment().vel,
            energy: inside.energy,
            fatigue: inside.fatigue,
            health: inside.health,
            breath_phase: inside.breath.phase,
            lung_volume: inside.breath.volume,
            air_flow: inside.breath.flow,
            consent: self.consent(),
            inbound_mixed: self.inbound_mixed(),
            may_share: may_share(self.consent()),
            may_remember: may_remember(self.consent()),
        }
    }
}
