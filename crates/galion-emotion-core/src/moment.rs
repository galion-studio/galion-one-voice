// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! The moment: where the speaker is in the open space, and how fast they
//! are moving. The baseline is not stored here.

use crate::axes::AXES;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MomentState {
    /// Time of the last completed step. 0 before the first step as well,
    /// so callers use [`crate::EmotionCore::frames`] to tell those apart.
    pub t_ms: u32,
    /// Completed steps.
    pub tick: u32,
    pub pos: [f32; AXES],
    /// Units per second along each axis.
    pub vel: [f32; AXES],
}

impl MomentState {
    pub const fn at(pos: [f32; AXES]) -> Self {
        Self {
            t_ms: 0,
            tick: 0,
            pos,
            vel: [0.0; AXES],
        }
    }
}
