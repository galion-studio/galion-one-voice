// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Frame pump. [`EmotionIn`] is [`Copy`]: the rings are scalars (RMS, a voiced
//! bit, F0, ZCR, harmonicity). There is no sample buffer in this type.
//!
//! The caller's PCM is borrowed for the duration of [`EmotionIn::push_frame`]
//! and is not copied into the state. [`EmotionIn::push_gated`] runs the
//! consent gate before the closure that yields the samples is called.

use crate::clock::{FRAME_MS, FRAME_SAMPLES};
use crate::consent::{ConsentGate, Purpose};
use crate::engine::{FakeBrain, MoodTarget};
use crate::error::{EmotionError, EmotionErrorKind};
use crate::map::{self, Mapped};
use crate::pad::Pad;
use crate::pitch::{self, Pitch};
use crate::prosody::{self, FrameRec, Prosody, ENV_LEN, FRAME_RING, SUBS_PER_FRAME};
use crate::report::{self, Report, StageTimes};
use crate::synth;
use crate::wav::resample_frame;
use std::time::Instant;

/// Latest prosody and the PAD target that should be handed to the brain.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reading {
    pub prosody: Prosody,
    pub mapped: Mapped,
    /// Start of the latest accepted frame, ms from the first frame.
    pub t_ms: u32,
    pub frames: u32,
}

impl Reading {
    pub const EMPTY: Reading = Reading {
        prosody: Prosody::EMPTY,
        mapped: Mapped::REST,
        t_ms: 0,
        frames: 0,
    };

    pub fn mood_target(self) -> Pad {
        self.mapped.target
    }

    pub fn retained_audio_samples(self) -> usize {
        0
    }
}

/// Prosody state across the last 2.56 s. Copy, fixed, no audio.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EmotionIn {
    env: [f32; ENV_LEN],
    env_voiced: [bool; ENV_LEN],
    env_pos: usize,
    env_filled: usize,
    frames_ring: [FrameRec; FRAME_RING],
    frame_pos: usize,
    frame_filled: usize,
    frames: u32,
    t_ms: u32,
}

impl Default for EmotionIn {
    fn default() -> Self {
        Self::new()
    }
}

impl EmotionIn {
    pub const fn new() -> Self {
        Self {
            env: [0.0; ENV_LEN],
            env_voiced: [false; ENV_LEN],
            env_pos: 0,
            env_filled: 0,
            frames_ring: [FrameRec::SILENT; FRAME_RING],
            frame_pos: 0,
            frame_filled: 0,
            frames: 0,
            t_ms: 0,
        }
    }

    /// Samples of user audio held inside this state. Always 0.
    pub const fn retained_audio_samples(&self) -> usize {
        0
    }

    /// Gate first. On deny the loader is not called and the state is unchanged.
    pub fn push_gated<'a, F>(
        &mut self,
        gate: &dyn ConsentGate,
        load: F,
    ) -> Result<Reading, EmotionError>
    where
        F: FnOnce() -> &'a [f32],
    {
        gate.permit(Purpose::Sense)?;
        self.push_open(load())
    }

    /// Gate first, then read `pcm`. `pcm` must already be one 24 kHz frame.
    pub fn push_frame(
        &mut self,
        gate: &dyn ConsentGate,
        pcm: &[f32],
    ) -> Result<Reading, EmotionError> {
        gate.permit(Purpose::Sense)?;
        self.push_open(pcm)
    }

    pub(crate) fn push_open(&mut self, pcm: &[f32]) -> Result<Reading, EmotionError> {
        if pcm.len() != FRAME_SAMPLES {
            return Err(EmotionError::new(
                EmotionErrorKind::BadFrame,
                "expected 1920 mono samples at 24 kHz (one 80 ms frame)",
            ));
        }
        if pcm.iter().any(|s| !s.is_finite()) {
            self.push_silence();
            return Ok(self.reading());
        }
        let pitch = pitch::estimate(pcm);
        let (_rms, zcr) = pitch::rms_zcr(pcm);
        let mut s = 0;
        while s < SUBS_PER_FRAME {
            let lo = s * (FRAME_SAMPLES / SUBS_PER_FRAME);
            let hi = lo + FRAME_SAMPLES / SUBS_PER_FRAME;
            let (sub_rms, _) = pitch::rms_zcr(&pcm[lo..hi]);
            let voiced = pitch.voiced && sub_rms >= prosody::SILENCE_RMS;
            self.push_sub(sub_rms, voiced);
            s += 1;
        }
        self.push_rec(rec_from(pitch, zcr, pcm));
        Ok(self.reading())
    }

    pub fn reading(&self) -> Reading {
        let mut env = [0.0f32; ENV_LEN];
        let mut voiced = [false; ENV_LEN];
        let n_env = ordered(self.env_filled, self.env_pos, ENV_LEN, |i| {
            env[i] = self.env[i_idx(self.env_filled, self.env_pos, ENV_LEN, i)];
            voiced[i] = self.env_voiced[i_idx(self.env_filled, self.env_pos, ENV_LEN, i)];
        });
        let mut frames = [FrameRec::SILENT; FRAME_RING];
        let n_fr = ordered(self.frame_filled, self.frame_pos, FRAME_RING, |i| {
            frames[i] = self.frames_ring[i_idx(self.frame_filled, self.frame_pos, FRAME_RING, i)];
        });
        let prosody = prosody::summarize(&env[..n_env], &voiced[..n_env], &frames[..n_fr]);
        let mapped = map::map_prosody(&prosody, false);
        Reading {
            prosody,
            mapped,
            t_ms: self.t_ms,
            frames: self.frames,
        }
    }

    /// Write the current mood target into a brain. Does not copy audio.
    pub fn apply_to(&self, brain: &mut impl MoodTarget) {
        brain.set_mood_target(self.reading().mood_target());
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    fn push_silence(&mut self) {
        let mut s = 0;
        while s < SUBS_PER_FRAME {
            self.push_sub(0.0, false);
            s += 1;
        }
        self.push_rec(FrameRec::SILENT);
    }

    fn push_sub(&mut self, rms: f32, voiced: bool) {
        self.env[self.env_pos] = if rms.is_finite() { rms } else { 0.0 };
        self.env_voiced[self.env_pos] = voiced;
        self.env_pos = (self.env_pos + 1) % ENV_LEN;
        if self.env_filled < ENV_LEN {
            self.env_filled += 1;
        }
    }

    fn push_rec(&mut self, rec: FrameRec) {
        self.frames_ring[self.frame_pos] = rec;
        self.frame_pos = (self.frame_pos + 1) % FRAME_RING;
        if self.frame_filled < FRAME_RING {
            self.frame_filled += 1;
        }
        self.t_ms = self.frames.saturating_mul(FRAME_MS);
        self.frames = self.frames.saturating_add(1);
    }
}

fn rec_from(pitch: Pitch, zcr: f32, pcm: &[f32]) -> FrameRec {
    let (rms, _) = pitch::rms_zcr(pcm);
    if pitch.voiced {
        FrameRec {
            f0_hz: pitch.hz,
            voiced: true,
            harmonicity: pitch.harmonicity,
            zcr,
            rms,
        }
    } else {
        FrameRec {
            f0_hz: 0.0,
            voiced: false,
            harmonicity: 0.0,
            zcr,
            rms,
        }
    }
}

fn i_idx(filled: usize, pos: usize, cap: usize, i: usize) -> usize {
    let start = if filled < cap { 0 } else { pos };
    (start + i) % cap
}

fn ordered(filled: usize, _pos: usize, cap: usize, mut body: impl FnMut(usize)) -> usize {
    let n = filled.min(cap);
    let mut i = 0;
    while i < n {
        body(i);
        i += 1;
    }
    n
}

/// Analyse a buffer already in memory. The samples are not retained in the
/// returned [`Report`]. `rate` is the buffer's rate; each 80 ms hop is
/// resampled to 24 kHz.
pub fn analyse_pcm(
    samples: &[f32],
    rate: u32,
    gate: &dyn ConsentGate,
) -> Result<Report, EmotionError> {
    gate.permit(Purpose::Sense)?;
    let hop = crate::clock::samples_per_frame(rate).ok_or(EmotionError::new(
        EmotionErrorKind::BadRate,
        "rate must be 1..=48000 Hz and divide 80 ms into a whole number of samples",
    ))?;
    let mut engine = EmotionIn::new();
    let mut times = Vec::new();
    let mut frame = [0.0f32; FRAME_SAMPLES];
    let mut off = 0usize;
    let mut frames = 0u32;
    while off + hop <= samples.len() {
        resample_frame(&samples[off..off + hop], rate, &mut frame);
        let started = Instant::now();
        engine.push_open(&frame)?;
        times.push(report::elapsed_us(started));
        frames += 1;
        off += hop;
    }
    frame = [0.0; FRAME_SAMPLES];
    let _ = frame;
    Ok(Report::from_reading(
        engine.reading(),
        frames,
        (samples.len() - off) as u32,
        &times,
    ))
}

pub fn measure_stages(frames: usize) -> StageTimes {
    let frame = synth::sine_frame(180.0, 0.2);
    let mut pitch_us = Vec::with_capacity(frames);
    let mut energy_us = Vec::with_capacity(frames);
    let mut push_us = Vec::with_capacity(frames);
    let mut tick_us = Vec::with_capacity(frames);
    let pitch_batch = Instant::now();
    for _ in 0..frames {
        let t = Instant::now();
        std::hint::black_box(pitch::estimate(std::hint::black_box(&frame)));
        pitch_us.push(report::elapsed_us(t));
    }
    let pitch_batch_us = report::elapsed_us(pitch_batch);
    let energy_batch = Instant::now();
    for _ in 0..frames {
        let t = Instant::now();
        std::hint::black_box(pitch::rms_zcr(std::hint::black_box(&frame)));
        energy_us.push(report::elapsed_us(t));
    }
    let energy_batch_us = report::elapsed_us(energy_batch);
    let gate = crate::consent::Session::grant_sense();
    let mut engine = EmotionIn::new();
    let push_batch = Instant::now();
    for _ in 0..frames {
        let t = Instant::now();
        let reading = engine.push_frame(&gate, &frame).expect("granted");
        std::hint::black_box(reading.prosody.rms);
        push_us.push(report::elapsed_us(t));
    }
    let push_batch_us = report::elapsed_us(push_batch);
    let mut brain = FakeBrain::open("", "", "");
    brain.set_mood_target(Pad {
        arousal: 0.8,
        valence: 0.1,
        dominance: 0.4,
    });
    let tick_batch = Instant::now();
    for _ in 0..frames {
        let t = Instant::now();
        brain.tick();
        std::hint::black_box(brain.mood().arousal);
        tick_us.push(report::elapsed_us(t));
    }
    let tick_batch_us = report::elapsed_us(tick_batch);
    StageTimes {
        frames: frames as u32,
        pitch_median_us: report::percentile(&pitch_us, 50),
        pitch_p95_us: report::percentile(&pitch_us, 95),
        pitch_sum_us: sum_us(&pitch_us),
        pitch_batch_us,
        energy_median_us: report::percentile(&energy_us, 50),
        energy_p95_us: report::percentile(&energy_us, 95),
        energy_sum_us: sum_us(&energy_us),
        energy_batch_us,
        push_median_us: report::percentile(&push_us, 50),
        push_p95_us: report::percentile(&push_us, 95),
        push_sum_us: sum_us(&push_us),
        push_batch_us,
        tick_median_us: report::percentile(&tick_us, 50),
        tick_p95_us: report::percentile(&tick_us, 95),
        tick_sum_us: sum_us(&tick_us),
        tick_batch_us,
    }
}

fn sum_us(v: &[u32]) -> u32 {
    v.iter().fold(0u32, |a, b| a.saturating_add(*b))
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scenario {
    pub name: &'static str,
    pub report: Report,
}

/// Synthetic scenes used by the bench and the tests. Not recordings.
pub fn synthetic_scenarios() -> Result<Vec<Scenario>, EmotionError> {
    let gate = crate::consent::Session::grant_sense();
    let mut out = Vec::new();
    let silence = vec![0.0f32; 25 * FRAME_SAMPLES];
    out.push(Scenario {
        name: "silence",
        report: analyse_pcm(&silence, 24_000, &gate)?,
    });
    let quiet = synth::steady_frames(12, 120.0, 0.05);
    out.push(Scenario {
        name: "quiet-low-120hz",
        report: analyse_pcm(&quiet, 24_000, &gate)?,
    });
    let loud = synth::steady_frames(12, 120.0, 0.25);
    out.push(Scenario {
        name: "loud-low-120hz",
        report: analyse_pcm(&loud, 24_000, &gate)?,
    });
    let high = synth::steady_frames(12, 240.0, 0.15);
    out.push(Scenario {
        name: "mid-high-240hz",
        report: analyse_pcm(&high, 24_000, &gate)?,
    });
    let low = synth::steady_frames(12, 120.0, 0.15);
    out.push(Scenario {
        name: "mid-low-120hz",
        report: analyse_pcm(&low, 24_000, &gate)?,
    });
    let slow = synth::am_frames(25, 180.0, 0.16, 2.0, 0.75);
    out.push(Scenario {
        name: "am-2hz",
        report: analyse_pcm(&slow, 24_000, &gate)?,
    });
    let fast = synth::am_frames(25, 180.0, 0.16, 5.0, 0.75);
    out.push(Scenario {
        name: "am-5hz",
        report: analyse_pcm(&fast, 24_000, &gate)?,
    });
    let gapped = synth::gap_frames(5, 5, 180.0, 0.2);
    out.push(Scenario {
        name: "gap-400ms",
        report: analyse_pcm(&gapped, 24_000, &gate)?,
    });
    let flat = synth::steady_frames(25, 240.0, 0.35);
    out.push(Scenario {
        name: "loud-flat-240hz",
        report: analyse_pcm(&flat, 24_000, &gate)?,
    });
    let melodic = synth::alternating(2.0, 140.0, 220.0, 180.0, 0.12);
    // 2.0 s at 24 kHz is exactly 25 frames.
    out.push(Scenario {
        name: "melodic-140-220",
        report: analyse_pcm(&melodic, 24_000, &gate)?,
    });
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consent::Session;
    use crate::pad::Pad;

    fn assert_copy<T: Copy>() {}

    #[test]
    fn state_holds_no_audio_buffer() {
        assert_copy::<EmotionIn>();
        assert_copy::<Reading>();
        assert_copy::<Prosody>();
        assert_copy::<Pad>();
        assert_copy::<Report>();
        assert!(std::mem::size_of::<EmotionIn>() < 8 * 1024);
        let engine = EmotionIn::new();
        assert_eq!(engine.retained_audio_samples(), 0);
        assert_eq!(engine.reading().retained_audio_samples(), 0);
    }

    #[test]
    fn deny_does_not_call_the_loader() {
        let mut engine = EmotionIn::new();
        let mut called = false;
        let pcm = [0.2f32; FRAME_SAMPLES];
        let err = engine
            .push_gated(&Session::DENY, || {
                called = true;
                &pcm[..]
            })
            .unwrap_err();
        assert_eq!(err.kind, EmotionErrorKind::Denied);
        assert!(!called);
        assert_eq!(engine.reading().frames, 0);
    }

    #[test]
    fn wrong_length_is_rejected() {
        let mut engine = EmotionIn::new();
        let err = engine
            .push_frame(&Session::grant_sense(), &[0.0; 10])
            .unwrap_err();
        assert_eq!(err.kind, EmotionErrorKind::BadFrame);
        assert_eq!(engine.reading().frames, 0);
    }

    #[test]
    fn silence_target_is_resting() {
        let gate = Session::grant_sense();
        let samples = vec![0.0f32; 25 * FRAME_SAMPLES];
        let report = analyse_pcm(&samples, 24_000, &gate).unwrap();
        assert_eq!(report.audio_retained_samples, 0);
        assert_eq!(report.reading.mood_target(), Pad::RESTING);
        assert_eq!(report.reading.mapped.confidence, 0.0);
        assert!(report.reading.prosody.f0_hz == 0.0);
    }

    #[test]
    fn reset_clears_frames() {
        let mut engine = EmotionIn::new();
        let frame = synth::sine_frame(180.0, 0.2);
        engine.push_frame(&Session::grant_sense(), &frame).unwrap();
        assert_eq!(engine.reading().frames, 1);
        engine.reset();
        assert_eq!(engine.reading().frames, 0);
        assert_eq!(engine.retained_audio_samples(), 0);
    }
}
