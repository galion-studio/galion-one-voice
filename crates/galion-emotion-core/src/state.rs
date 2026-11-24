// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! The core. One spring, one baseline, one moment.
//!
//! `step` is the hot path. It does not allocate, does not call libm, and
//! does not write [`crate::VoiceBaseline`].

use crate::axes::{clamp_pos, AXES};
use crate::baseline::VoiceBaseline;
use crate::clock::{DT_S, FRAME_MS};
use crate::factors::{factor_offset, standin_breath, Breath, Inside};
use crate::intent::{
    validate, IntentMsg, IntentSource, MASK_BREATH, MASK_ENERGY, MASK_FATIGUE, MASK_HEALTH,
};
use crate::moment::MomentState;
use crate::wire::{admit_pull, local_outbound, may_remember};
use crate::CoreError;
use galion_emotion::EmotionWire;

/// Spring stiffness, per second squared. Stand-in knob.
pub const STIFFNESS: f32 = 8.0;
/// Damping, per second. Slightly above critical for `STIFFNESS` so a step
/// settles without a libm square root.
pub const DAMPING: f32 = 6.0;
/// Velocity clamp, axis units per second.
pub const VEL_LIMIT: f32 = 4.0;

/// Heap allocations one `EmotionCore::step` is allowed to perform.
pub const STEP_ALLOC_BUDGET: usize = 0;
/// Arithmetic ops in `apply`, counted by hand. The measured half of the
/// budget is the zero-allocation test.
pub const STEP_FLOP_BUDGET: u32 = 96;

/// Speaker baseline plus the living moment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EmotionCore {
    baseline: VoiceBaseline,
    moment: MomentState,
    inside: Inside,
    consent: u8,
    expect_ms: u32,
    frames: u32,
    inbound_mixed: bool,
}

impl EmotionCore {
    pub fn from_baseline(baseline: VoiceBaseline) -> Self {
        Self {
            moment: MomentState::at(baseline.pos),
            baseline,
            inside: Inside::rest(),
            consent: 0,
            expect_ms: 0,
            frames: 0,
            inbound_mixed: false,
        }
    }

    pub fn at_rest() -> Self {
        Self::from_baseline(VoiceBaseline::standin())
    }

    pub fn baseline(&self) -> VoiceBaseline {
        self.baseline
    }

    pub fn moment(&self) -> MomentState {
        self.moment
    }

    pub fn inside(&self) -> Inside {
        self.inside
    }

    pub fn consent(&self) -> u8 {
        self.consent
    }

    pub fn frames(&self) -> u32 {
        self.frames
    }

    pub fn next_ms(&self) -> u32 {
        self.expect_ms
    }

    pub fn inbound_mixed(&self) -> bool {
        self.inbound_mixed
    }

    /// The persona baseline, and only when `remember` is set.
    /// The moment is never copied into it.
    pub fn persisted_baseline(&self) -> Option<VoiceBaseline> {
        if may_remember(self.consent) {
            Some(self.baseline)
        } else {
            None
        }
    }

    pub fn local_wire(&self, seq: u16) -> EmotionWire {
        local_outbound(&self.moment, self.consent, seq)
    }

    pub fn shared_wire(&self, seq: u16) -> Option<EmotionWire> {
        if crate::wire::may_share(self.consent) {
            Some(self.local_wire(seq))
        } else {
            None
        }
    }

    /// Integrate one frame. `intent.t_ms` must be the next frame on the
    /// virtual clock, starting at 0 and then every [`FRAME_MS`].
    pub fn step(&mut self, intent: &IntentMsg) -> Result<MomentState, CoreError> {
        validate(intent)?;
        if intent.t_ms != self.expect_ms {
            return Err(CoreError::OffClock {
                expected_ms: self.expect_ms,
                got_ms: intent.t_ms,
            });
        }
        self.expect_ms = self
            .expect_ms
            .checked_add(FRAME_MS)
            .ok_or(CoreError::ClockOverflow)?;
        apply(self, intent);
        Ok(self.moment)
    }

    pub fn replay(baseline: VoiceBaseline, script: &[IntentMsg]) -> Result<Self, CoreError> {
        let mut core = Self::from_baseline(baseline);
        for msg in script {
            core.step(msg)?;
        }
        Ok(core)
    }
}

fn apply(core: &mut EmotionCore, intent: &IntentMsg) {
    if intent.source == IntentSource::Own {
        if intent.factor_mask & MASK_ENERGY != 0 {
            core.inside.energy = intent.energy;
        }
        if intent.factor_mask & MASK_FATIGUE != 0 {
            core.inside.fatigue = intent.fatigue;
        }
        if intent.factor_mask & MASK_HEALTH != 0 {
            core.inside.health = intent.health;
        }
        if intent.factor_mask & MASK_BREATH != 0 {
            core.inside.breath = Breath {
                phase: intent.breath_phase,
                volume: intent.lung_volume,
                flow: intent.air_flow,
            };
        }
    }
    if intent.source != IntentSource::Own || intent.factor_mask & MASK_BREATH == 0 {
        let frame = intent.t_ms / FRAME_MS;
        core.inside.breath = standin_breath(frame);
    }

    let mut target = core.baseline.pos;
    let offset = factor_offset(&core.inside);
    let admit = admit_pull(intent);
    let mut mixed = false;
    for i in 0..AXES {
        let mut add = offset[i];
        if admit && intent.pull_mask & (1 << i) != 0 && intent.strength != 0.0 {
            add += intent.pull[i] * intent.strength;
            if intent.source == IntentSource::Inbound {
                mixed = true;
            }
        }
        target[i] = clamp_pos(i, core.baseline.pos[i] + add);
    }

    for i in 0..AXES {
        let x = core.moment.pos[i];
        let v = core.moment.vel[i];
        let acc = STIFFNESS * (target[i] - x) - DAMPING * v;
        let v2 = (v + acc * DT_S).clamp(-VEL_LIMIT, VEL_LIMIT);
        let x2 = clamp_pos(i, x + v2 * DT_S);
        core.moment.vel[i] = v2;
        core.moment.pos[i] = x2;
    }

    core.moment.t_ms = intent.t_ms;
    core.moment.tick = core.moment.tick.wrapping_add(1);
    core.frames = core.frames.wrapping_add(1);
    core.consent = intent.consent & 0x0f;
    core.inbound_mixed = mixed;
    debug_assert!(core.moment.pos.iter().all(|v| v.is_finite()));
    debug_assert!(core.moment.vel.iter().all(|v| v.is_finite()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rejected_tick_does_not_move_the_core() {
        let mut core = EmotionCore::at_rest();
        let before = core;
        let err = core.step(&IntentMsg::hold(40)).unwrap_err();
        assert_eq!(
            err,
            CoreError::OffClock {
                expected_ms: 0,
                got_ms: 40
            }
        );
        assert_eq!(core, before);
        core.step(&IntentMsg::steady(0)).unwrap();
        let mid = core;
        assert!(core.step(&IntentMsg::steady(160)).is_err());
        assert_eq!(core, mid);
    }
}
