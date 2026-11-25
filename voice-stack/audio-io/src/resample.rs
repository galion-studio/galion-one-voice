// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Resamplers for the two rates this stack already uses.
//!
//! [`resample_linear`] is the batch form of the linear resample in
//! `archive/2026-11-24/crates/galion-v2-live/src/audio.rs` (Piper 22050 → the 24 kHz line).
//! It runs on the producer thread, where a `Vec` is fine.
//!
//! [`BoxResampler`] is the streaming form of the one-dot page's box average
//! (`onCapChunk` in `crates/galion-console/assets/v1-screen.html`): each
//! 16 kHz sample is the mean of the device samples that fall in its span.
//! A remainder is kept so a short HAL period does not drop the tail of the
//! chunk. The page resets that tail; a 128-frame callback cannot.
//!
//! [`LinearResampler`] runs on the playback callback to reach the device
//! rate. It does not allocate after [`LinearResampler::new`].

use crate::AudioError;

/// Linear resample. `from == to` returns a copy. Empty in, empty out.
/// Output length is `len * to / from`, truncated, matching the integer
/// length in `archive/2026-11-24/crates/galion-v2-live/src/audio.rs`.
pub fn resample_linear(input: &[f32], from: u32, to: u32) -> Result<Vec<f32>, AudioError> {
    if from == 0 || to == 0 {
        return Err(AudioError::Rate);
    }
    if from == to || input.is_empty() {
        return Ok(input.to_vec());
    }
    let n = (input.len() as u64 * u64::from(to) / u64::from(from)) as usize;
    let step = f64::from(from) / f64::from(to);
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let pos = i as f64 * step;
        let j = pos as usize;
        let frac = (pos - j as f64) as f32;
        let a = input[j.min(input.len() - 1)];
        let b = input[(j + 1).min(input.len() - 1)];
        out.push(a + (b - a) * frac);
    }
    Ok(out)
}

/// Streaming box average. One output sample per `in_hz / out_hz` input samples.
#[derive(Debug)]
pub struct BoxResampler {
    in_hz: u32,
    out_hz: u32,
    acc: f32,
    count: u32,
    filled: f64,
}

impl BoxResampler {
    pub fn new(in_hz: u32, out_hz: u32) -> Result<Self, AudioError> {
        if in_hz == 0 || out_hz == 0 {
            return Err(AudioError::Rate);
        }
        Ok(Self {
            in_hz,
            out_hz,
            acc: 0.0,
            count: 0,
            filled: 0.0,
        })
    }

    pub fn reset(&mut self) {
        self.acc = 0.0;
        self.count = 0;
        self.filled = 0.0;
    }

    pub fn push(&mut self, input: &[f32], mut emit: impl FnMut(f32)) {
        if self.in_hz == self.out_hz {
            for &sample in input {
                emit(sample);
            }
            return;
        }
        let need = f64::from(self.in_hz) / f64::from(self.out_hz);
        for &sample in input {
            self.acc += sample;
            self.count += 1;
            self.filled += 1.0;
            if self.filled + 1e-6 >= need {
                let n = self.count.max(1) as f32;
                emit(self.acc / n);
                self.acc = 0.0;
                self.count = 0;
                self.filled -= need;
                if self.filled < 1e-6 {
                    self.filled = 0.0;
                }
            }
        }
    }
}

/// Streaming linear resampler. `pos` is an index into [`Self::buf`].
#[derive(Debug)]
pub(crate) struct LinearResampler {
    from_hz: u32,
    to_hz: u32,
    pos: f64,
    buf: Vec<f32>,
}

impl LinearResampler {
    pub(crate) fn new(from_hz: u32, to_hz: u32) -> Result<Self, AudioError> {
        if from_hz == 0 || to_hz == 0 {
            return Err(AudioError::Rate);
        }
        Ok(Self {
            from_hz,
            to_hz,
            pos: 0.0,
            buf: Vec::with_capacity(16_384),
        })
    }

    pub(crate) fn reset(&mut self) {
        self.pos = 0.0;
        self.buf.clear();
    }

    pub(crate) fn pending(&self) -> usize {
        self.buf.len().saturating_sub(self.pos.floor() as usize)
    }

    pub(crate) fn push(&mut self, samples: &[f32]) {
        self.buf.extend_from_slice(samples);
    }

    /// Write into `dst`. Returns how many output samples were produced.
    /// The unfilled tail of `dst` is left as it was.
    pub(crate) fn pull(&mut self, dst: &mut [f32]) -> usize {
        if dst.is_empty() {
            return 0;
        }
        if self.from_hz == self.to_hz {
            let n = dst.len().min(self.buf.len());
            dst[..n].copy_from_slice(&self.buf[..n]);
            self.buf.drain(..n);
            return n;
        }
        let step = f64::from(self.from_hz) / f64::from(self.to_hz);
        let mut n = 0;
        while n < dst.len() {
            let i = self.pos.floor() as usize;
            if i + 1 >= self.buf.len() {
                break;
            }
            let frac = (self.pos - i as f64) as f32;
            let a = self.buf[i];
            let b = self.buf[i + 1];
            dst[n] = a + (b - a) * frac;
            n += 1;
            self.pos += step;
        }
        let drop = (self.pos.floor() as usize).min(self.buf.len().saturating_sub(1));
        if drop > 0 {
            self.buf.drain(..drop);
            self.pos -= drop as f64;
            if self.pos < 0.0 {
                self.pos = 0.0;
            }
        }
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_length_matches_the_v2_live_ratio() {
        let input = vec![0.0; 22_050];
        let out = resample_linear(&input, 22_050, 24_000).unwrap();
        assert_eq!(out.len(), 24_000);
        let same = resample_linear(&[0.25, -0.25], 16_000, 16_000).unwrap();
        assert_eq!(same, vec![0.25, -0.25]);
    }

    #[test]
    fn linear_ramp_stays_between_the_endpoints() {
        let input: Vec<f32> = (0..100).map(|i| i as f32 / 100.0).collect();
        let out = resample_linear(&input, 16_000, 48_000).unwrap();
        assert!(out.len() > input.len());
        let peak = out.iter().copied().fold(0.0f32, f32::max);
        let low = out.iter().copied().fold(0.0f32, f32::min);
        assert!(peak <= 1.0);
        assert!(low >= 0.0);
    }

    #[test]
    fn box_integer_ratio_preserves_a_constant() {
        let mut boxer = BoxResampler::new(48_000, 16_000).unwrap();
        let mut out = Vec::new();
        boxer.push(&[0.5; 48], |s| out.push(s));
        assert_eq!(out.len(), 16);
        assert!(out.iter().all(|s| (*s - 0.5).abs() < 1e-6));
    }

    #[test]
    fn box_streams_across_short_chunks() {
        let mut boxer = BoxResampler::new(48_000, 16_000).unwrap();
        let mut out = Vec::new();
        for _ in 0..3 {
            boxer.push(&[1.0], |s| out.push(s));
        }
        assert_eq!(out, vec![1.0]);
        boxer.push(&[1.0; 3], |s| out.push(s));
        assert_eq!(out, vec![1.0, 1.0]);
    }

    #[test]
    fn streaming_linear_matches_a_constant_and_resets() {
        let mut rs = LinearResampler::new(24_000, 48_000).unwrap();
        rs.push(&[0.25; 64]);
        let mut dst = vec![0.0; 100];
        let n = rs.pull(&mut dst);
        assert!(n >= 90, "produced {n}");
        assert!(dst[..n].iter().all(|s| (*s - 0.25).abs() < 1e-5));
        rs.reset();
        assert_eq!(rs.pending(), 0);
        rs.push(&[1.0, 0.0]);
        let mut one = [0.0; 1];
        assert_eq!(rs.pull(&mut one), 1);
        assert!((one[0] - 1.0).abs() < 1e-5);
    }
}
