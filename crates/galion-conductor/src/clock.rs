// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! Frame counter. One tick is [`FRAME_MS`] milliseconds.
//!
//! This is the same 80 ms quantum as `LOCKS.toml` `[clock] frame_ms`.
//! The counter moves only when the host calls [`VirtualClock::advance`].
//! It does not read a wall clock.

/// Locked frame. `LOCKS.toml` `[clock] frame_ms`.
pub const FRAME_MS: u32 = 80;

/// Barge-in lock. `LOCKS.toml` `[gates] barge_in_max_ms`.
pub const BARGE_IN_MAX_MS: u32 = 200;

/// Galion Research side band. `LOCKS.toml` `[gates] side_band_cap_ms`.
///
/// A result is merged only when its arrival, on this frame clock, is at or
/// under this many milliseconds. The value is not a frame multiple. The
/// comparison is in milliseconds, not in frames.
pub const SIDE_BAND_CAP_MS: u64 = 3_000;

/// Duck, copied from `galion-heartbeat` `mixer::DUCK_MS`.
pub const DUCK_MS: u32 = 80;

/// Silence arms on the frame that would play past this.
/// Copied from `galion-heartbeat` `mixer::PAUSE_MS`.
pub const PAUSE_MS: u32 = BARGE_IN_MAX_MS;

/// The mixer yields the floor after this much voiced overlap.
/// This conductor cancels a non-backchannel at [`PAUSE_MS`] instead.
/// The listed-backchannel rule is what keeps a short overlap alive.
pub const MIXER_YIELD_MS: u32 = 800;

/// Quiet that lets the mixer resume a short overlap.
/// Recorded so the conductor and the mixer name the same ladder.
pub const MIXER_RESUME_QUIET_MS: u32 = 240;

/// Unchanged partial, in frames. Two ticks is 160 ms
/// (`docs/research/galion-voice-v2/domains/04-latency-streaming.md` §4).
pub const STABLE_TICKS: u32 = 2;

/// Silence frames after a stable prefix before eager end-of-turn.
/// One frame is the R4 §4.3 hang (80 ms on this clock).
pub const HANG_FRAMES: u32 = 1;

/// Silence frames that confirm the turn and release the clause.
/// The second quiet frame is the confirm. The first was the hang.
pub const COMMIT_SILENCE_FRAMES: u32 = 2;

const _: () = assert!(FRAME_MS == 80);
const _: () = assert!(BARGE_IN_MAX_MS == 200);
const _: () = assert!(SIDE_BAND_CAP_MS == 3_000);
const _: () = assert!(PAUSE_MS == 200);
const _: () = assert!(DUCK_MS == FRAME_MS);
const _: () = assert!(STABLE_TICKS == 2);
const _: () = assert!(HANG_FRAMES == 1);
const _: () = assert!(COMMIT_SILENCE_FRAMES == 2);
const _: () = assert!(MIXER_YIELD_MS == 800);
const _: () = assert!(MIXER_RESUME_QUIET_MS == 240);

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

    /// Start of the current frame, in microseconds on this clock.
    pub const fn t_us(self) -> u64 {
        self.t_ms().saturating_mul(1_000)
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
        assert_eq!(clock.frame(), 0);
        clock.advance();
        assert_eq!(clock.t_ms(), 80);
        assert_eq!(clock.t_us(), 80_000);
        clock.advance();
        assert_eq!(clock.t_ms(), 160);
    }
}
