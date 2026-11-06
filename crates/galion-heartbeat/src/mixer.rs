// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! Dual-band mixer. CPU only. No GPU, no heap, no second clock.
//!
//! One call is one 80 ms frame ([`crate::FRAME_MS`]). The you band and the me
//! band hear each other. While one band is speaking and the other stays
//! voiced:
//!
//! * at 80 ms, the speaking band's gain becomes [`DUCK_GAIN`] (0.35)
//! * the frame that would play past 200 ms is silence ([`PAUSE_MS`],
//!   `LOCKS.toml` `barge_in_max_ms`)
//! * 800 ms of that overlap yields the floor and releases the held queue
//! * 240 ms of quiet before that yield resumes; the floor stays
//!
//! A blip under 150 ms ducks and does not yield. The decision is applied on
//! the next frame, same as the heartbeat. Backchannel PCM is not a lane here.
//! The host keeps that buffer and adds it after a pause.

use crate::{FRAME_MS, FRAME_SAMPLES};

/// Overlap that ducks the speaking band. The ducked frame starts at this age.
pub const DUCK_MS: u32 = 80;
/// Silence starts on the frame that would play past this. `LOCKS.toml` `barge_in_max_ms`.
pub const PAUSE_MS: u32 = 200;
/// Voiced overlap that gives the floor away.
pub const YIELD_MS: u32 = 800;
/// Quiet after a pause that keeps the floor (a short overlap, not a turn).
pub const RESUME_QUIET_MS: u32 = 240;
/// Gain while ducked. Full scale is 1.
pub const DUCK_GAIN: f32 = 0.35;
/// Gain while the band is not ducked.
pub const FULL_GAIN: f32 = 1.0;

const _: () = assert!(DUCK_MS == FRAME_MS);
const _: () = assert!(PAUSE_MS == 200);
const _: () = assert!(YIELD_MS == 800);
const _: () = assert!(RESUME_QUIET_MS == 240);

/// Which band holds the floor after a yield.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Band {
    You,
    Me,
}

#[derive(Clone, Copy, Debug)]
struct Lane {
    overlap_ms: u32,
    pause_voiced_ms: u32,
    pause_quiet_ms: u32,
    gain: f32,
    ducked: bool,
    paused: bool,
}

impl Lane {
    const fn new() -> Self {
        Self {
            overlap_ms: 0,
            pause_voiced_ms: 0,
            pause_quiet_ms: 0,
            gain: FULL_GAIN,
            ducked: false,
            paused: false,
        }
    }

    fn clear_overlap(&mut self) {
        self.overlap_ms = 0;
        self.pause_voiced_ms = 0;
        self.pause_quiet_ms = 0;
        self.ducked = false;
        self.paused = false;
        self.gain = FULL_GAIN;
    }
}

/// Fixed-size mixer. No allocation.
#[derive(Clone, Copy, Debug)]
pub struct DualBandMixer {
    you: Lane,
    me: Lane,
    floor: Option<Band>,
}

impl Default for DualBandMixer {
    fn default() -> Self {
        Self::new()
    }
}

impl DualBandMixer {
    pub const fn new() -> Self {
        Self {
            you: Lane::new(),
            me: Lane::new(),
            floor: None,
        }
    }

    pub const fn floor(&self) -> Option<Band> {
        self.floor
    }

    /// Mix one frame into `you_out` and `me_out`.
    ///
    /// A band whose input or output is not [`FRAME_SAMPLES`] long is not
    /// written and does not advance. The other band still runs.
    pub fn step(&mut self, input: MixIn<'_>, you_out: &mut [f32], me_out: &mut [f32]) -> MixOut {
        let you_ok = input.you.len() == FRAME_SAMPLES && you_out.len() == FRAME_SAMPLES;
        let me_ok = input.me.len() == FRAME_SAMPLES && me_out.len() == FRAME_SAMPLES;
        let you_gain = self.you.gain;
        let me_gain = self.me.gain;
        let you_paused = self.you.paused;
        let me_paused = self.me.paused;
        let you_played = if you_ok {
            emit(&self.you, input.you, you_out);
            true
        } else {
            false
        };
        let me_played = if me_ok {
            emit(&self.me, input.me, me_out);
            true
        } else {
            false
        };
        let you_release = if you_ok {
            update(&mut self.you, input.you_speaking, input.me_voiced)
        } else {
            false
        };
        let me_release = if me_ok {
            update(&mut self.me, input.me_speaking, input.you_voiced)
        } else {
            false
        };
        if you_release && !me_release {
            self.floor = Some(Band::Me);
        } else if me_release && !you_release {
            self.floor = Some(Band::You);
        }
        MixOut {
            you_gain,
            me_gain,
            you_paused,
            me_paused,
            you_played,
            me_played,
            you_release,
            me_release,
            floor: self.floor,
        }
    }
}

/// One frame of both bands. The caller owns the samples.
#[derive(Clone, Copy, Debug)]
pub struct MixIn<'a> {
    pub you: &'a [f32],
    pub me: &'a [f32],
    /// You has a turn in progress.
    pub you_speaking: bool,
    /// Me has a turn in progress.
    pub me_speaking: bool,
    /// You was voiced this frame (the me band hears this).
    pub you_voiced: bool,
    /// Me was voiced this frame (the you band hears this).
    pub me_voiced: bool,
}

/// What this frame played. Gains and pause flags are the values just written
/// into the output buffers. `you_release` / `me_release` are decided at the
/// end of the frame and take effect on the next one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MixOut {
    pub you_gain: f32,
    pub me_gain: f32,
    pub you_paused: bool,
    pub me_paused: bool,
    /// Input length matched a frame, so this band was mixed.
    pub you_played: bool,
    pub me_played: bool,
    /// Host should drop the rest of this band's queue.
    pub you_release: bool,
    pub me_release: bool,
    pub floor: Option<Band>,
}

fn emit(lane: &Lane, src: &[f32], dst: &mut [f32]) {
    if lane.paused {
        let mut i = 0;
        while i < FRAME_SAMPLES {
            dst[i] = 0.0;
            i += 1;
        }
        return;
    }
    let g = lane.gain;
    let mut i = 0;
    while i < FRAME_SAMPLES {
        let s = src[i] * g;
        dst[i] = if s > 1.0 {
            1.0
        } else if s < -1.0 {
            -1.0
        } else {
            s
        };
        i += 1;
    }
}

/// Returns whether this lane yielded the floor.
fn update(lane: &mut Lane, speaking: bool, other_voiced: bool) -> bool {
    if lane.paused {
        if other_voiced {
            lane.pause_voiced_ms = lane.pause_voiced_ms.saturating_add(FRAME_MS);
            lane.pause_quiet_ms = 0;
        } else {
            lane.pause_quiet_ms = lane.pause_quiet_ms.saturating_add(FRAME_MS);
        }
        if lane.pause_quiet_ms >= RESUME_QUIET_MS && lane.pause_voiced_ms < YIELD_MS {
            lane.clear_overlap();
            return false;
        }
        if lane.pause_voiced_ms >= YIELD_MS {
            lane.clear_overlap();
            return true;
        }
        return false;
    }
    if speaking && other_voiced {
        lane.overlap_ms = lane.overlap_ms.saturating_add(FRAME_MS);
        if lane.overlap_ms >= DUCK_MS {
            lane.ducked = true;
            lane.gain = DUCK_GAIN;
        }
        if lane.overlap_ms.saturating_add(FRAME_MS) > PAUSE_MS {
            lane.paused = true;
            lane.pause_voiced_ms = lane.overlap_ms;
            lane.pause_quiet_ms = 0;
        }
        return false;
    }
    lane.overlap_ms = 0;
    if lane.ducked {
        lane.ducked = false;
        lane.gain = FULL_GAIN;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(x: f32) -> [f32; FRAME_SAMPLES] {
        [x; FRAME_SAMPLES]
    }

    fn drive(
        m: &mut DualBandMixer,
        n: usize,
        you_speaking: bool,
        me_speaking: bool,
        you_voiced: bool,
        me_voiced: bool,
    ) -> MixOut {
        let you = tone(0.5);
        let me = tone(0.25);
        let mut you_out = [0.0; FRAME_SAMPLES];
        let mut me_out = [0.0; FRAME_SAMPLES];
        let mut last = MixOut {
            you_gain: 0.0,
            me_gain: 0.0,
            you_paused: false,
            me_paused: false,
            you_played: false,
            me_played: false,
            you_release: false,
            me_release: false,
            floor: None,
        };
        for _ in 0..n {
            last = m.step(
                MixIn {
                    you: &you,
                    me: &me,
                    you_speaking,
                    me_speaking,
                    you_voiced,
                    me_voiced,
                },
                &mut you_out,
                &mut me_out,
            );
        }
        last
    }

    fn sample_after(
        frames_already: usize,
        you_speaking: bool,
        me_speaking: bool,
        you_voiced: bool,
        me_voiced: bool,
    ) -> (f32, f32, MixOut) {
        let mut m = DualBandMixer::new();
        if frames_already > 0 {
            drive(
                &mut m,
                frames_already,
                you_speaking,
                me_speaking,
                you_voiced,
                me_voiced,
            );
        }
        let you = tone(0.5);
        let me = tone(0.25);
        let mut you_out = [0.0; FRAME_SAMPLES];
        let mut me_out = [0.0; FRAME_SAMPLES];
        let out = m.step(
            MixIn {
                you: &you,
                me: &me,
                you_speaking,
                me_speaking,
                you_voiced,
                me_voiced,
            },
            &mut you_out,
            &mut me_out,
        );
        (you_out[0], me_out[0], out)
    }

    #[test]
    fn passes_both_bands_when_nobody_talks_over_anyone() {
        let (y, m, out) = sample_after(0, true, true, false, false);
        assert!((y - 0.5).abs() < 1e-6);
        assert!((m - 0.25).abs() < 1e-6);
        assert!(!out.you_paused && !out.me_paused);
        assert!((out.you_gain - 1.0).abs() < 1e-6);
        assert!((out.me_gain - 1.0).abs() < 1e-6);
        assert!(out.floor.is_none());
    }

    #[test]
    fn ducks_the_speaking_band_on_the_frame_that_starts_at_80ms() {
        let (y, me0, _) = sample_after(0, false, true, true, false);
        assert!((y - 0.5).abs() < 1e-6, "you is not speaking");
        assert!(
            (me0 - 0.25).abs() < 1e-6,
            "first overlap frame is still full gain"
        );
        let (_, me1, out) = sample_after(1, false, true, true, false);
        assert!((me1 - 0.25 * DUCK_GAIN).abs() < 1e-6);
        assert!((out.me_gain - DUCK_GAIN).abs() < 1e-6);
        assert!(!out.me_paused);
        assert!(!out.me_release);
    }

    #[test]
    fn silence_starts_on_the_frame_that_would_pass_200ms() {
        // Frames 0 and 1 play. Frame 2 (t = 160 ms) is the one that contains 200 ms.
        let (_, me1, _) = sample_after(1, false, true, true, false);
        assert!(me1.abs() > 0.0);
        let (_, me2, out) = sample_after(2, false, true, true, false);
        assert_eq!(me2, 0.0);
        assert!(out.me_paused);
        assert!(!out.me_release);
        assert!(out.floor.is_none());
    }

    #[test]
    fn short_overlap_resumes_and_keeps_the_floor() {
        let mut m = DualBandMixer::new();
        drive(&mut m, 3, false, true, true, false);
        assert!(m.me.paused);
        let out = drive(&mut m, 3, false, true, false, false);
        assert!(
            out.me_paused,
            "the frame that completes the quiet is still silence"
        );
        assert!(!out.me_release);
        assert!(out.floor.is_none());
        assert!(!m.me.paused);
        assert!((m.me.gain - 1.0).abs() < 1e-6);
        let (_, me, _) = {
            let me_pcm = tone(0.25);
            let you_pcm = tone(0.0);
            let mut you_out = [0.0; FRAME_SAMPLES];
            let mut me_out = [0.0; FRAME_SAMPLES];
            let o = m.step(
                MixIn {
                    you: &you_pcm,
                    me: &me_pcm,
                    you_speaking: false,
                    me_speaking: true,
                    you_voiced: false,
                    me_voiced: false,
                },
                &mut you_out,
                &mut me_out,
            );
            (you_out[0], me_out[0], o)
        };
        assert!((me - 0.25).abs() < 1e-6);
    }

    #[test]
    fn eight_hundred_ms_of_overlap_yields_the_floor() {
        let mut m = DualBandMixer::new();
        let mut released = None;
        for i in 0..12 {
            let out = drive(&mut m, 1, false, true, true, false);
            if out.me_release {
                released = Some(i);
                assert_eq!(out.floor, Some(Band::You));
                assert!(out.me_paused, "the yielding frame itself stays silent");
                assert!((m.me.gain - FULL_GAIN).abs() < 1e-6);
                break;
            }
        }
        assert_eq!(
            released,
            Some(9),
            "yield at the frame that completes 800 ms"
        );
    }

    #[test]
    fn a_blip_under_150ms_ducks_once_and_does_not_pause() {
        let mut m = DualBandMixer::new();
        let you = tone(0.5);
        let me = tone(0.25);
        let mut you_out = [0.0; FRAME_SAMPLES];
        let mut me_out = [0.0; FRAME_SAMPLES];
        let first = m.step(
            MixIn {
                you: &you,
                me: &me,
                you_speaking: false,
                me_speaking: true,
                you_voiced: true,
                me_voiced: false,
            },
            &mut you_out,
            &mut me_out,
        );
        assert!((me_out[0] - 0.25).abs() < 1e-6);
        assert!(!first.me_paused);
        assert!(!m.me.paused);
        let second = m.step(
            MixIn {
                you: &you,
                me: &me,
                you_speaking: false,
                me_speaking: true,
                you_voiced: false,
                me_voiced: false,
            },
            &mut you_out,
            &mut me_out,
        );
        assert!((me_out[0] - 0.25 * DUCK_GAIN).abs() < 1e-6);
        assert!(!second.me_paused);
        assert!(!second.me_release);
        assert!(!m.me.paused);
        assert!((m.me.gain - FULL_GAIN).abs() < 1e-6);
    }

    #[test]
    fn each_band_hears_the_other() {
        let (_, _, out) = sample_after(1, true, true, true, true);
        assert!((out.you_gain - DUCK_GAIN).abs() < 1e-6);
        assert!((out.me_gain - DUCK_GAIN).abs() < 1e-6);
    }

    #[test]
    fn a_short_buffer_does_not_advance_that_band() {
        let mut m = DualBandMixer::new();
        let you = [0.2f32; 4];
        let me = tone(0.25);
        let mut you_out = [7.0; 4];
        let mut me_out = [0.0; FRAME_SAMPLES];
        let out = m.step(
            MixIn {
                you: &you,
                me: &me,
                you_speaking: true,
                me_speaking: true,
                you_voiced: true,
                me_voiced: true,
            },
            &mut you_out,
            &mut me_out,
        );
        assert!(!out.you_played);
        assert_eq!(you_out[0], 7.0);
        assert!(out.me_played);
        assert!((me_out[0] - 0.25).abs() < 1e-6);
        assert_eq!(m.you.overlap_ms, 0);
    }

    #[test]
    fn gain_stays_inside_minus_one_to_one() {
        let you = tone(4.0);
        let me = tone(-4.0);
        let mut you_out = [0.0; FRAME_SAMPLES];
        let mut me_out = [0.0; FRAME_SAMPLES];
        DualBandMixer::new().step(
            MixIn {
                you: &you,
                me: &me,
                you_speaking: false,
                me_speaking: false,
                you_voiced: false,
                me_voiced: false,
            },
            &mut you_out,
            &mut me_out,
        );
        assert_eq!(you_out[0], 1.0);
        assert_eq!(me_out[0], -1.0);
    }
}
