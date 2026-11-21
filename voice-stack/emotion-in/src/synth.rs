// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Deterministic tones for tests and the bench. These are not recordings.

use crate::clock::{FRAME_SAMPLES, SAMPLE_RATE_HZ};

pub fn sine(hz: f32, amp: f32, n: usize, sr: u32) -> Vec<f32> {
    let sr = sr.max(1) as f32;
    (0..n)
        .map(|i| {
            let t = i as f32 / sr;
            amp * (std::f32::consts::TAU * hz * t).sin()
        })
        .collect()
}

pub fn sine_frame(hz: f32, amp: f32) -> [f32; FRAME_SAMPLES] {
    let mut out = [0.0f32; FRAME_SAMPLES];
    fill_sine(&mut out, hz, amp, 0);
    out
}

pub fn fill_sine(dst: &mut [f32], hz: f32, amp: f32, offset: usize) {
    let sr = SAMPLE_RATE_HZ as f32;
    for (i, s) in dst.iter_mut().enumerate() {
        let t = (offset + i) as f32 / sr;
        *s = amp * (std::f32::consts::TAU * hz * t).sin();
    }
}

/// Tone bursts. `gap` between burst starts is `1 / rate_hz`. Each burst is a
/// sine of `burst_ms` at `carrier_hz`. The rest of the buffer is silence.
pub fn bursts(seconds: f32, rate_hz: f32, burst_ms: f32, carrier_hz: f32, amp: f32) -> Vec<f32> {
    let n = (seconds * SAMPLE_RATE_HZ as f32).round() as usize;
    let mut out = vec![0.0f32; n];
    if rate_hz <= 0.0 || burst_ms <= 0.0 {
        return out;
    }
    let burst_n = (burst_ms * 0.001 * SAMPLE_RATE_HZ as f32).round() as usize;
    let period = (SAMPLE_RATE_HZ as f32 / rate_hz).round() as usize;
    if period == 0 || burst_n == 0 {
        return out;
    }
    let mut start = 0usize;
    while start < n {
        let end = (start + burst_n).min(n);
        fill_sine(&mut out[start..end], carrier_hz, amp, start);
        start += period;
    }
    out
}

/// Steady tone whose frequency switches between `low` and `high` every `every_ms`.
/// Amplitude-modulated sine. `depth` is 0..1. The floor is `(1 - depth) * amp`,
/// so a moderate depth stays above the silence threshold the whole way.
pub fn am_frames(frames: usize, carrier: f32, amp: f32, mod_hz: f32, depth: f32) -> Vec<f32> {
    let n = frames * FRAME_SAMPLES;
    let sr = SAMPLE_RATE_HZ as f32;
    let mut out = vec![0.0f32; n];
    for (i, s) in out.iter_mut().enumerate() {
        let t = i as f32 / sr;
        let env = (1.0 - depth) + depth * (0.5 + 0.5 * (std::f32::consts::TAU * mod_hz * t).sin());
        *s = amp * env * (std::f32::consts::TAU * carrier * t).sin();
    }
    out
}

pub fn steady_frames(frames: usize, hz: f32, amp: f32) -> Vec<f32> {
    sine(hz, amp, frames * FRAME_SAMPLES, SAMPLE_RATE_HZ)
}

pub fn gap_frames(tone_frames: usize, gap_frames: usize, hz: f32, amp: f32) -> Vec<f32> {
    let tone = steady_frames(tone_frames, hz, amp);
    let mut out = Vec::with_capacity(tone.len() * 2 + gap_frames * FRAME_SAMPLES);
    out.extend_from_slice(&tone);
    out.extend(std::iter::repeat(0.0f32).take(gap_frames * FRAME_SAMPLES));
    out.extend_from_slice(&tone);
    out
}

pub fn alternating(seconds: f32, low: f32, high: f32, every_ms: f32, amp: f32) -> Vec<f32> {
    let n = (seconds * SAMPLE_RATE_HZ as f32).round() as usize;
    let mut out = vec![0.0f32; n];
    let hop = (every_ms * 0.001 * SAMPLE_RATE_HZ as f32).round().max(1.0) as usize;
    let mut i = 0;
    let mut which = false;
    while i < n {
        let end = (i + hop).min(n);
        let hz = if which { high } else { low };
        fill_sine(&mut out[i..end], hz, amp, i);
        which = !which;
        i = end;
    }
    out
}
