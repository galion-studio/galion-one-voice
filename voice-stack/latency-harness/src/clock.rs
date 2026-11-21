// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! 80 ms heartbeat constants and the TTFA drain grid.
//!
//! `FRAME_MS`, `SAMPLE_RATE_HZ`, and `FRAME_SAMPLES` are the same numbers as
//! `crates/galion-heartbeat` (`FRAME_MS`, `SAMPLE_RATE`, `FRAME_SAMPLES`).
//! `STEP_BUDGET_US` is `crates/galion-heartbeat/src/symphony.rs`
//! `STEP_BUDGET_US` (1% of the tick). This file does not remeasure that step.
//!
//! The play grid is the drain in `scripts/eval/piper_stream_ttfa.py`: the
//! tick starts at the first PCM byte, a tick with no full frame is a pause,
//! and a late frame does not play a catch-up burst.

/// Heartbeat period. Locked at 80 ms.
pub const FRAME_MS: u32 = 80;
/// Me-band rate after resample (Mimi / heartbeat, 24 kHz).
pub const SAMPLE_RATE_HZ: u32 = 24_000;
/// Samples in one 80 ms frame at 24 kHz.
pub const FRAME_SAMPLES: usize = (SAMPLE_RATE_HZ as usize * FRAME_MS as usize) / 1000;
/// 12.5 Hz, stored in tenths. 80 ms * 125 = 10_000.
pub const FRAME_HZ_TENTHS: u32 = 125;
/// One heartbeat frame, in microseconds. The per-stage bar.
pub const BAR_US: u64 = (FRAME_MS as u64) * 1000;
/// Non-inference ceiling cited from the symphony wire. Not measured here.
pub const STEP_BUDGET_US: u32 = 800;
/// Piper `en_US-ljspeech-medium` rate. Confirmed from the pack json at runtime.
pub const PIPER_HZ: u32 = 22_050;

const _: () = assert!(FRAME_MS == 80);
const _: () = assert!(SAMPLE_RATE_HZ == 24_000);
const _: () = assert!(FRAME_SAMPLES == 1_920);
const _: () = assert!(FRAME_MS as u32 * FRAME_HZ_TENTHS == 10_000);
const _: () = assert!(BAR_US == 80_000);
const _: () = assert!(STEP_BUDGET_US == 800);

/// Move `pad` 10% of the way toward [`crate::Pad::RESTING`].
///
/// This is the harness stand-in used when no HORBM brain is linked. It is
/// not that brain's mood update. One call is one 80 ms tick.
pub fn advance_toward_rest(pad: crate::Pad) -> crate::Pad {
    fn step(v: f32, rest: f32) -> f32 {
        v + (rest - v) * 0.1
    }
    crate::Pad {
        arousal: step(pad.arousal, crate::Pad::RESTING.arousal),
        valence: step(pad.valence, crate::Pad::RESTING.valence),
        dominance: step(pad.dominance, crate::Pad::RESTING.dominance),
    }
}

/// Input samples required before one 24 kHz heartbeat frame can be built.
pub fn input_samples_for_one_frame(in_rate: u32) -> usize {
    if in_rate == 0 {
        return 0;
    }
    if in_rate == SAMPLE_RATE_HZ {
        return FRAME_SAMPLES;
    }
    let need = (FRAME_SAMPLES as u64) * (in_rate as u64) / (SAMPLE_RATE_HZ as u64);
    (need as usize).saturating_add(1)
}

/// Linear resample into one heartbeat frame. Output length is [`FRAME_SAMPLES`].
///
/// Same interpolation as the Piper TTFA drain (sample `i` reads input at
/// `i * in_rate / 24_000`). Equal rates copy. No allocation.
pub fn resample_to_frame(src: &[i16], in_rate: u32, out: &mut [f32]) {
    assert_eq!(out.len(), FRAME_SAMPLES);
    if src.is_empty() || in_rate == 0 {
        for s in out.iter_mut() {
            *s = 0.0;
        }
        return;
    }
    if in_rate == SAMPLE_RATE_HZ {
        for i in 0..FRAME_SAMPLES {
            let s = src.get(i).copied().unwrap_or(0);
            out[i] = s as f32 / 32768.0;
        }
        return;
    }
    let ratio = (in_rate as f64) / (SAMPLE_RATE_HZ as f64);
    for i in 0..FRAME_SAMPLES {
        let x = (i as f64) * ratio;
        let j = x.floor() as usize;
        let frac = (x - (j as f64)) as f32;
        let a_s = src.get(j).copied().unwrap_or(0);
        let b_s = src.get(j + 1).copied().unwrap_or(a_s);
        let a = i16_to_f(a_s);
        let b = i16_to_f(b_s);
        out[i] = a + (b - a) * frac;
    }
}

fn i16_to_f(s: i16) -> f32 {
    s as f32 / 32768.0
}

/// When frame 0 may play on the 80 ms grid.
///
/// `frame_arrived_us` and `first_pcm_us` are both microseconds from the same
/// origin (end of speech). The grid starts at the first PCM byte. If the
/// frame is already complete in that first read, it plays at that instant.
/// Otherwise it plays on the next tick boundary. This does not sleep.
pub fn heartbeat_play_us(frame_arrived_us: u64, first_pcm_us: u64) -> u64 {
    if frame_arrived_us <= first_pcm_us {
        return first_pcm_us;
    }
    let delta = frame_arrived_us - first_pcm_us;
    let steps = (delta + BAR_US - 1) / BAR_US;
    first_pcm_us + steps * BAR_US
}

/// Play times for the first `frames` heartbeat frames, plus late ticks.
///
/// `arrivals` is `(time_us, cumulative_samples)` in nondecreasing time.
/// Each tick plays at most one frame. A tick that is early is a pause.
/// Nothing catches up on the next tick.
pub fn schedule_frames(
    arrivals: &[(u64, u64)],
    frame_samples: u64,
    frames: u64,
) -> (Vec<u64>, u64) {
    if arrivals.is_empty() || frame_samples == 0 || frames == 0 {
        return (Vec::new(), 0);
    }
    let t0 = arrivals[0].0;
    let mut t = t0;
    let mut played = 0u64;
    let mut late = 0u64;
    let mut plays = Vec::new();
    let mut guard = 0u32;
    while played < frames {
        let need = (played + 1) * frame_samples;
        let ready = arrivals
            .iter()
            .find(|(_, cum)| *cum >= need)
            .map(|(at, _)| *at);
        let Some(ready) = ready else {
            break;
        };
        if ready <= t {
            plays.push(t);
            played += 1;
        } else {
            late += 1;
        }
        t = t.saturating_add(BAR_US);
        guard += 1;
        if guard > 100_000 {
            break;
        }
    }
    (plays, late)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_matches_the_heartbeat_crate() {
        assert_eq!(FRAME_MS, 80);
        assert_eq!(FRAME_SAMPLES, 1_920);
        assert_eq!(BAR_US, 80_000);
    }

    #[test]
    fn play_grid_matches_the_scheduler() {
        let (plays, late) = schedule_frames(&[(0, 100)], 10, 3);
        assert_eq!(plays, vec![0, 80_000, 160_000]);
        assert_eq!(late, 0);
        assert_eq!(heartbeat_play_us(0, 0), plays[0]);
    }

    #[test]
    fn a_late_tick_does_not_burst() {
        // One sample at t=0, then four frames arrive together on the next tick.
        let (plays, late) = schedule_frames(&[(0, 1), (BAR_US, 40)], 10, 4);
        assert_eq!(plays, vec![BAR_US, BAR_US * 2, BAR_US * 3, BAR_US * 4]);
        assert!(late >= 1, "the empty first tick is a pause");
        let gaps: Vec<u64> = plays.windows(2).map(|w| w[1] - w[0]).collect();
        assert!(
            gaps.iter().all(|g| *g == BAR_US),
            "one frame per tick, got {plays:?}"
        );
    }

    #[test]
    fn partial_first_read_waits_for_the_next_tick() {
        assert_eq!(heartbeat_play_us(1_001, 1_000), 1_000 + BAR_US);
        assert_eq!(heartbeat_play_us(1_000 + BAR_US, 1_000), 1_000 + BAR_US);
        assert_eq!(
            heartbeat_play_us(1_000 + BAR_US + 1, 1_000),
            1_000 + 2 * BAR_US
        );
        let (plays, _) = schedule_frames(&[(1_000, 1), (1_001, 10)], 10, 1);
        assert_eq!(plays, vec![heartbeat_play_us(1_001, 1_000)]);
    }

    #[test]
    fn resting_mood_stays_put() {
        let pad = advance_toward_rest(crate::Pad::RESTING);
        assert!((pad.arousal - 0.3).abs() < 1e-6);
        assert!((pad.valence - 0.25).abs() < 1e-6);
        assert!((pad.dominance - 0.5).abs() < 1e-6);
    }

    #[test]
    fn piper_frame_need_matches_the_drain_formula() {
        // int(1920 * 22050/24000) + 1 = 1765
        assert_eq!(input_samples_for_one_frame(PIPER_HZ), 1_765);
        assert_eq!(input_samples_for_one_frame(SAMPLE_RATE_HZ), FRAME_SAMPLES);
    }
}
