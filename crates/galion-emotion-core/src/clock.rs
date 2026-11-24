// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! Virtual frame clock. `LOCKS.toml` `[clock] frame_ms` is 80. There is no
//! other cadence. This clock does not read the wall time.

use crate::CoreError;

/// Heartbeat. Same quantum as `galion-emotion` and `galion-mne-contract`.
pub const FRAME_MS: u32 = 80;

/// One step in seconds. `FRAME_MS / 1000`, written once so the spring and
/// the lock cannot drift apart. Exact in the f32 that `80 / 1000` produces.
pub const DT_S: f32 = FRAME_MS as f32 / 1000.0;

/// Counts frames. `advance` yields 0, then 80, then 160, and so on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VirtualClock {
    frames_done: u32,
}

impl Default for VirtualClock {
    fn default() -> Self {
        Self::new()
    }
}

impl VirtualClock {
    pub const fn new() -> Self {
        Self { frames_done: 0 }
    }

    pub const fn frames_done(&self) -> u32 {
        self.frames_done
    }

    /// Time the next `advance` will yield.
    pub fn next_ms(&self) -> Result<u32, CoreError> {
        self.frames_done
            .checked_mul(FRAME_MS)
            .ok_or(CoreError::ClockOverflow)
    }

    /// Yield the current frame time, then move one frame forward.
    pub fn advance(&mut self) -> Result<u32, CoreError> {
        let t = self.next_ms()?;
        self.frames_done = self
            .frames_done
            .checked_add(1)
            .ok_or(CoreError::ClockOverflow)?;
        Ok(t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advance_is_eighty_milliseconds() {
        let mut clock = VirtualClock::new();
        assert_eq!(clock.advance().unwrap(), 0);
        assert_eq!(clock.advance().unwrap(), 80);
        assert_eq!(clock.advance().unwrap(), 160);
        assert_eq!(clock.frames_done(), 3);
        assert_eq!(clock.next_ms().unwrap(), 240);
        assert_eq!(FRAME_MS, 80);
        assert!((DT_S - 0.08).abs() < 1e-6);
    }
}
