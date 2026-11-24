// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Conductor frame clock.
//!
//! Same quantum as `galion-conductor` (`crates/galion-conductor/src/clock.rs`).
//! That crate is its own workspace, so this path keeps the same numbers
//! instead of taking a path dependency.
//! The counter moves only when the host calls [`VirtualClock::advance`].
//! [`crate::VoiceLoop::tick`] is the only caller. It does not read a wall clock.
//!
//! Silence is armed on the frame whose running overlap would play past
//! [`PAUSE_MS`], and applied on the next frame. That frame starts at 160 ms,
//! which is the frame that contains the 200 ms lock.

/// Locked frame. `LOCKS.toml` `[clock] frame_ms`.
pub const FRAME_MS: u32 = 80;

/// Barge-in lock. `LOCKS.toml` `[gates] barge_in_max_ms`.
pub const BARGE_IN_MAX_MS: u32 = 200;

/// Silence arms when the next frame would play past this.
/// Same predicate as the conductor (`overlap + FRAME_MS > PAUSE_MS`).
pub const PAUSE_MS: u32 = BARGE_IN_MAX_MS;

/// End of speech to the first mouth frame, on this clock.
/// The schedule is one frame (80 ms). The gate allows two (160 ms),
/// inside the published 240 ms end-of-speech target.
pub const FIRST_AUDIO_BUDGET_MS: u32 = 160;

/// Ear and fake-mouth rate. 16 kHz mono.
pub const SAMPLE_RATE: u32 = 16_000;

/// Samples in one frame at [`SAMPLE_RATE`].
pub const FRAME_SAMPLES: usize = (SAMPLE_RATE as usize * FRAME_MS as usize) / 1000;

const _: () = assert!(FRAME_MS == 80);
const _: () = assert!(BARGE_IN_MAX_MS == 200);
const _: () = assert!(PAUSE_MS == 200);
const _: () = assert!(FIRST_AUDIO_BUDGET_MS == 160);
const _: () = assert!(SAMPLE_RATE == 16_000);
const _: () = assert!(FRAME_SAMPLES == 1_280);

/// Host-owned frame index. `t_ms = frame * FRAME_MS`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VirtualClock {
    frame: u64,
}

impl Default for VirtualClock {
    fn default() -> Self {
        Self::new()
    }
}

impl VirtualClock {
    pub const fn new() -> Self {
        Self { frame: 0 }
    }

    pub const fn frame(self) -> u64 {
        self.frame
    }

    /// Start of the current frame, in milliseconds on this clock.
    pub const fn t_ms(self) -> u64 {
        self.frame.saturating_mul(FRAME_MS as u64)
    }

    pub fn advance(&mut self) {
        self.frame = self.frame.saturating_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advance_is_one_frame() {
        let mut clock = VirtualClock::new();
        assert_eq!(clock.t_ms(), 0);
        clock.advance();
        assert_eq!(clock.t_ms(), 80);
        clock.advance();
        assert_eq!(clock.t_ms(), 160);
    }
}
