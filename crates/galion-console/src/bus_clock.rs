// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! One conversion between the process-bus frame clock and the domain 04 stamp clock.
//!
//! `BusMsg.t_ms` is heartbeat time: the frame index times 80 ms. It is not wall
//! clock. `galion_cascade::stamps::StampClock` (`StampClock::us`) is monotonic
//! microseconds from `std::time::Instant`. The two clocks do not share an origin.
//! This module does not import `StampClock`. A stamp here is a `u64` of those
//! microseconds.
//!
//! [`BusStampAnchor`] is the only conversion. One heartbeat millisecond is
//! [`STAMP_US_PER_MS`] stamp microseconds (1000). [`BusStampAnchor::to_frame_ms`]
//! drops sub-millisecond residue and snaps down onto the frame lattice
//! (frame × 80 ms).
//!
//! `ProcessBus::try_send` does not call this. A message keeps the `t_ms` the
//! sender wrote. Nothing here changes bus behaviour.
//!
//! No PCM. No words.

#![forbid(unsafe_code)]

use galion_heartbeat::FRAME_MS;

/// Heartbeat period in milliseconds. [`FRAME_MS`] widened to `u64`.
pub const BUS_FRAME_MS: u64 = FRAME_MS as u64;
const _: () = assert!(FRAME_MS == 80);
const _: () = assert!(BUS_FRAME_MS == 80);

/// Stamp-clock microseconds in one heartbeat millisecond.
pub const STAMP_US_PER_MS: u64 = 1_000;

/// One paired sample of the two clocks.
///
/// `frame_ms` is heartbeat time (`BusMsg.t_ms`). `stamp_us` is a domain 04
/// monotonic microsecond count (`StampClock::us`). They are the same instant
/// only because a caller said so.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BusStampAnchor {
    /// Heartbeat milliseconds. Frame index × 80. Not wall clock.
    pub frame_ms: u64,
    /// Domain 04 monotonic microseconds. Not the bus frame clock.
    pub stamp_us: u64,
}

impl BusStampAnchor {
    /// Pair a heartbeat millisecond with the stamp microsecond observed then.
    pub fn new(frame_ms: u64, stamp_us: u64) -> Self {
        Self { frame_ms, stamp_us }
    }

    /// Heartbeat milliseconds → stamp microseconds.
    ///
    /// `anchor.stamp_us + (frame_ms - anchor.frame_ms) * STAMP_US_PER_MS`,
    /// computed in `i128`. A negative result saturates at 0. A result above
    /// `u64::MAX` saturates at `u64::MAX`.
    pub fn to_stamp_us(self, frame_ms: u64) -> u64 {
        let delta_ms = (frame_ms as i128) - (self.frame_ms as i128);
        let stamp = (self.stamp_us as i128) + delta_ms * (STAMP_US_PER_MS as i128);
        if stamp < 0 {
            0
        } else if stamp > u64::MAX as i128 {
            u64::MAX
        } else {
            stamp as u64
        }
    }

    /// Stamp microseconds → heartbeat milliseconds, snapped down to a frame.
    ///
    /// `delta_us` is `stamp_us - anchor.stamp_us` in `i128`. `delta_ms` is
    /// `delta_us.div_euclid(STAMP_US_PER_MS)`, so the division floors, including
    /// when the stamp is before the anchor. Sub-millisecond residue is dropped.
    /// `ms` is `anchor.frame_ms + delta_ms`. A result at or below 0 is 0. A
    /// result at or above `u64::MAX` is `u64::MAX` before the snap. The snap is
    /// absolute from zero: `ms - (ms % BUS_FRAME_MS)` (frame × 80).
    ///
    /// `to_frame_ms(to_stamp_us(frame_ms)) == frame_ms` holds only when both
    /// this anchor's `frame_ms` and the input `frame_ms` are multiples of
    /// [`BUS_FRAME_MS`]. The lattice is measured from zero, so an anchor or an
    /// input that is not on that lattice does not round-trip to itself.
    pub fn to_frame_ms(self, stamp_us: u64) -> u64 {
        let delta_us = (stamp_us as i128) - (self.stamp_us as i128);
        let delta_ms = delta_us.div_euclid(STAMP_US_PER_MS as i128);
        let ms = (self.frame_ms as i128) + delta_ms;
        let ms = if ms <= 0 {
            return 0;
        } else if ms >= u64::MAX as i128 {
            u64::MAX
        } else {
            ms as u64
        };
        ms - (ms % BUS_FRAME_MS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_lattice_round_trips_through_the_stamp_clock() {
        let anchor = BusStampAnchor::new(0, 5_000);
        assert_eq!(anchor.to_stamp_us(0), 5_000);
        assert_eq!(anchor.to_stamp_us(80), 5_000 + 80_000);
        assert_eq!(anchor.to_stamp_us(160), 5_000 + 160_000);
        assert_eq!(anchor.to_frame_ms(anchor.to_stamp_us(0)), 0);
        assert_eq!(anchor.to_frame_ms(anchor.to_stamp_us(80)), 80);
        assert_eq!(anchor.to_frame_ms(anchor.to_stamp_us(160)), 160);
        assert_eq!(anchor.to_frame_ms(5_000 + 80_000 + 79_999), 80);
        assert_eq!(anchor.to_frame_ms(5_000 + 160_000), 160);
        for frame in [0, 80, 160, 800, 8_000] {
            assert_eq!(anchor.to_frame_ms(anchor.to_stamp_us(frame)), frame);
        }
    }

    #[test]
    fn off_lattice_milliseconds_snap_down_to_the_frame() {
        let anchor = BusStampAnchor::new(0, 0);
        assert_eq!(anchor.to_stamp_us(100), 100_000);
        assert_eq!(anchor.to_frame_ms(anchor.to_stamp_us(100)), 80);
        assert_ne!(anchor.to_frame_ms(anchor.to_stamp_us(100)), 100);
    }

    #[test]
    fn anchor_not_at_zero_converts_both_ways() {
        let anchor = BusStampAnchor::new(240, 1_000_000);
        assert_eq!(anchor.to_stamp_us(240), 1_000_000);
        assert_eq!(anchor.to_stamp_us(160), 1_000_000 - 80_000);
        assert_eq!(anchor.to_frame_ms(1_000_000 - 80_000), 160);
        // Frame 160 covers heartbeat milliseconds 160..240. One microsecond
        // before the anchor (stamp 999_999) is still that frame.
        assert_eq!(anchor.to_frame_ms(1_000_000 - 1), 160);
        assert_eq!(anchor.to_frame_ms(999_999), 160);
        assert_eq!(anchor.to_frame_ms(1_000_000), 240);
        assert_eq!(anchor.to_frame_ms(920_000), 160);
    }

    #[test]
    fn times_before_the_origin_saturate_at_zero() {
        // 50_000 - 80 * 1000 is negative.
        assert_eq!(BusStampAnchor::new(80, 50_000).to_stamp_us(0), 0);
        assert_eq!(BusStampAnchor::new(160, 200_000).to_frame_ms(0), 0);
    }

    #[test]
    fn saturation_at_the_top_of_u64() {
        assert_eq!(BUS_FRAME_MS, 80);
        assert_eq!(STAMP_US_PER_MS, 1_000);
        assert_eq!(BUS_FRAME_MS, u64::from(galion_heartbeat::FRAME_MS));

        assert_eq!(BusStampAnchor::new(0, u64::MAX).to_stamp_us(80), u64::MAX);
        assert_eq!(
            BusStampAnchor::new(0, u64::MAX - 1000).to_stamp_us(1),
            u64::MAX
        );

        let snapped = BusStampAnchor::new(0, 0).to_frame_ms(u64::MAX);
        assert_eq!(snapped % 80, 0);
        assert_eq!(snapped % BUS_FRAME_MS, 0);
        assert!(snapped > 0);
        assert!(i128::from(snapped) <= i128::from(u64::MAX));

        // Past the top of `u64` milliseconds: clamp, then snap down.
        let topped = BusStampAnchor::new(u64::MAX, 0).to_frame_ms(STAMP_US_PER_MS);
        assert_eq!(topped % BUS_FRAME_MS, 0);
        assert_eq!(topped, u64::MAX - (u64::MAX % BUS_FRAME_MS));
        assert!(topped > 0);
    }
}
