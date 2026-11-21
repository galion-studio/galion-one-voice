// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! 24 kHz playback ring and the barge-in flush.
//!
//! [`Playback`] is cloned into the producer (TTS / heartbeat) and into
//! [`Renderer`] (the output callback). [`Playback::barge_in`] is safe from
//! a third thread. The render that observes it writes silence for the whole
//! period and drops samples that were queued when `barge_in` ran. Samples
//! pushed after it returns play on a later period.
//!
//! [`Playback::set_gain`] is the 80 ms duck. It does not flush.
//! [`Playback::push_pcm16`] resamples to 24 kHz on the caller thread
//! (Piper is 22050). The block is written only when the whole resampled
//! buffer fits.

use crate::convert::i16_to_f32;
use crate::resample::{resample_linear, LinearResampler};
use crate::ring::Spsc;
use crate::{mono_ns, AudioError, PLAYBACK_FRAME_SAMPLES, PLAYBACK_HZ};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

struct PlaybackInner {
    ring: Spsc<f32>,
    gain_bits: AtomicU32,
    underruns: AtomicU64,
    overruns: AtomicU64,
    flushes: AtomicU64,
    flush_req_ns: AtomicU64,
    flush_honor_ns: AtomicU64,
    played: AtomicU64,
}

/// Shared playback handle. One producer thread.
#[derive(Clone)]
pub struct Playback {
    inner: Arc<PlaybackInner>,
}

impl Playback {
    /// `samples` is rounded up to a power of two. 32768 samples is about 1.36 s at 24 kHz.
    pub fn new(samples: usize) -> Self {
        let _ = mono_ns();
        Self {
            inner: Arc::new(PlaybackInner {
                ring: Spsc::new(samples),
                gain_bits: AtomicU32::new(1.0f32.to_bits()),
                underruns: AtomicU64::new(0),
                overruns: AtomicU64::new(0),
                flushes: AtomicU64::new(0),
                flush_req_ns: AtomicU64::new(0),
                flush_honor_ns: AtomicU64::new(0),
                played: AtomicU64::new(0),
            }),
        }
    }

    pub fn queued(&self) -> usize {
        self.inner.ring.queued()
    }

    pub fn free(&self) -> usize {
        self.inner.ring.free()
    }

    pub fn discarded(&self) -> u64 {
        self.inner.ring.discarded()
    }

    pub fn underruns(&self) -> u64 {
        self.inner.underruns.load(Ordering::Relaxed)
    }

    pub fn overruns(&self) -> u64 {
        self.inner.overruns.load(Ordering::Relaxed)
    }

    pub fn flushes(&self) -> u64 {
        self.inner.flushes.load(Ordering::Relaxed)
    }

    pub fn played(&self) -> u64 {
        self.inner.played.load(Ordering::Relaxed)
    }

    /// Nanoseconds from the latest `barge_in` to the render that honored it.
    /// `None` until a render has observed a flush.
    pub fn flush_honor_ns(&self) -> Option<u64> {
        let v = self.inner.flush_honor_ns.load(Ordering::Acquire);
        (v != 0).then_some(v)
    }

    /// Gain in `0.0..=1.0`. Values outside that range are clamped.
    /// `0.35` is [`crate::DUCK_GAIN`].
    pub fn set_gain(&self, gain: f32) {
        let gain = if gain.is_finite() {
            gain.clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.inner
            .gain_bits
            .store(gain.to_bits(), Ordering::Relaxed);
    }

    pub fn gain(&self) -> f32 {
        f32::from_bits(self.inner.gain_bits.load(Ordering::Relaxed))
    }

    /// Push 24 kHz f32. Returns how many samples fit. A short write counts
    /// as an overrun of the remainder.
    pub fn push(&self, samples: &[f32]) -> usize {
        let n = self.inner.ring.push_slice(samples);
        if n < samples.len() {
            self.inner.overruns.fetch_add(1, Ordering::Relaxed);
        }
        n
    }

    /// Write the whole slice or nothing.
    pub fn try_push_all(&self, samples: &[f32]) -> bool {
        if samples.is_empty() {
            return true;
        }
        if samples.len() > self.free() {
            self.inner.overruns.fetch_add(1, Ordering::Relaxed);
            return false;
        }
        let n = self.inner.ring.push_slice(samples);
        debug_assert_eq!(n, samples.len());
        n == samples.len()
    }

    /// One heartbeat frame, 1920 samples at 24 kHz. All or nothing.
    pub fn push_heartbeat_frame(&self, frame: &[f32]) -> Result<(), AudioError> {
        if frame.len() != PLAYBACK_FRAME_SAMPLES {
            return Err(AudioError::Frame {
                got: frame.len(),
                expect: PLAYBACK_FRAME_SAMPLES,
            });
        }
        if self.try_push_all(frame) {
            Ok(())
        } else {
            Err(AudioError::Overrun)
        }
    }

    /// PCM16 at `src_hz`, resampled to 24 kHz. Returns source samples
    /// consumed, or 0 when the resampled block does not fit.
    /// `src_hz == 22050` is Piper ([`crate::PIPER_HZ`]).
    pub fn push_pcm16(&self, samples: &[i16], src_hz: u32) -> Result<usize, AudioError> {
        if samples.is_empty() {
            return Ok(0);
        }
        if src_hz == 0 {
            return Err(AudioError::Rate);
        }
        let float: Vec<f32> = samples.iter().copied().map(i16_to_f32).collect();
        let at_play = if src_hz == PLAYBACK_HZ {
            float
        } else {
            resample_linear(&float, src_hz, PLAYBACK_HZ)?
        };
        if self.try_push_all(&at_play) {
            Ok(samples.len())
        } else {
            Ok(0)
        }
    }

    /// Drop queued playback. Lock-free. Does not change gain.
    pub fn barge_in(&self) {
        self.inner.ring.request_skip();
        self.inner.flush_req_ns.store(mono_ns(), Ordering::Release);
        self.inner.flushes.fetch_add(1, Ordering::Relaxed);
    }

    fn skip(&self) -> usize {
        self.inner.ring.skip()
    }

    fn apply_skip(&self) -> u64 {
        self.inner.ring.apply_skip()
    }

    fn pop_slice(&self, dst: &mut [f32]) -> usize {
        let n = self.inner.ring.pop_slice(dst);
        if n > 0 {
            self.inner.played.fetch_add(n as u64, Ordering::Relaxed);
        }
        n
    }

    fn note_underrun(&self) {
        self.inner.underruns.fetch_add(1, Ordering::Relaxed);
    }

    fn note_flush_honored(&self) {
        let req = self.inner.flush_req_ns.load(Ordering::Acquire);
        if req == 0 {
            return;
        }
        let dt = mono_ns().saturating_sub(req);
        // Keep a non-zero value so `flush_honor_ns()` can tell "not yet" from a hit.
        self.inner
            .flush_honor_ns
            .store(dt.max(1), Ordering::Release);
    }
}

impl Default for Playback {
    fn default() -> Self {
        Self::new(32_768)
    }
}

/// What one render period did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderResult {
    pub flushed: bool,
    pub underrun: bool,
    /// Samples taken from the ring or the resampler before silence padding.
    pub produced: usize,
}

/// Output-callback side of [`Playback`]. Not shared. Not the producer.
pub struct Renderer {
    playback: Playback,
    resampler: LinearResampler,
    staging: Vec<f32>,
    applied_skip: usize,
    device_hz: u32,
}

impl Renderer {
    pub fn new(playback: Playback, device_hz: u32) -> Result<Self, AudioError> {
        let reserve =
            ((4_096u64 * u64::from(PLAYBACK_HZ)) / u64::from(device_hz.max(1))) as usize + 32;
        Ok(Self {
            playback,
            resampler: LinearResampler::new(PLAYBACK_HZ, device_hz)?,
            staging: vec![0.0; reserve.max(64)],
            applied_skip: 0,
            device_hz,
        })
    }

    pub fn playback(&self) -> Playback {
        self.playback.clone()
    }

    /// Fill `mono_out` at [`Self`]'s device rate. A period that observes a
    /// flush is all zeros, including samples that were already copied.
    pub fn render(&mut self, mono_out: &mut [f32]) -> RenderResult {
        if mono_out.is_empty() {
            return RenderResult {
                flushed: false,
                underrun: false,
                produced: 0,
            };
        }
        if self.honor_flush(mono_out) {
            return RenderResult {
                flushed: true,
                underrun: false,
                produced: 0,
            };
        }
        let had_audio = self.playback.queued() > 0 || self.resampler.pending() >= 2;
        let (produced, underrun) = if self.device_hz == PLAYBACK_HZ {
            self.render_native(mono_out)
        } else {
            self.render_converted(mono_out)
        };
        self.apply_gain(mono_out);
        if self.honor_flush(mono_out) {
            return RenderResult {
                flushed: true,
                underrun: false,
                produced: 0,
            };
        }
        for sample in mono_out.iter_mut() {
            *sample = clamp_audio(*sample);
        }
        if underrun && had_audio {
            self.playback.note_underrun();
        }
        RenderResult {
            flushed: false,
            underrun: underrun && had_audio,
            produced,
        }
    }

    fn honor_flush(&mut self, mono_out: &mut [f32]) -> bool {
        let skip = self.playback.skip();
        if skip == self.applied_skip {
            return false;
        }
        self.playback.apply_skip();
        self.applied_skip = self.playback.skip();
        self.resampler.reset();
        mono_out.fill(0.0);
        self.playback.note_flush_honored();
        true
    }

    fn render_native(&mut self, mono_out: &mut [f32]) -> (usize, bool) {
        let n = self.playback.pop_slice(mono_out);
        if n < mono_out.len() {
            mono_out[n..].fill(0.0);
            return (n, true);
        }
        (n, false)
    }

    fn render_converted(&mut self, mono_out: &mut [f32]) -> (usize, bool) {
        let mut filled = 0;
        let mut underrun = false;
        while filled < mono_out.len() {
            if self.resampler.pending() < 2 {
                let got = self.playback.pop_slice(&mut self.staging);
                if got == 0 {
                    underrun = true;
                    break;
                }
                self.resampler.push(&self.staging[..got]);
            }
            let n = self.resampler.pull(&mut mono_out[filled..]);
            if n == 0 {
                let got = self.playback.pop_slice(&mut self.staging);
                if got == 0 {
                    underrun = true;
                    break;
                }
                self.resampler.push(&self.staging[..got]);
                let n = self.resampler.pull(&mut mono_out[filled..]);
                if n == 0 {
                    underrun = true;
                    break;
                }
                filled += n;
            } else {
                filled += n;
            }
        }
        if filled < mono_out.len() {
            mono_out[filled..].fill(0.0);
        }
        (filled, underrun)
    }

    fn apply_gain(&self, mono: &mut [f32]) {
        let gain = self.playback.gain();
        if (gain - 1.0).abs() < 1e-6 {
            return;
        }
        for sample in mono {
            *sample *= gain;
        }
    }
}

fn clamp_audio(sample: f32) -> f32 {
    if sample.is_finite() {
        sample.clamp(-1.0, 1.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DUCK_GAIN;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn heartbeat_frame_plays_back_at_24k() {
        let pb = Playback::new(8_192);
        let frame = vec![0.25f32; PLAYBACK_FRAME_SAMPLES];
        pb.push_heartbeat_frame(&frame).unwrap();
        let mut renderer = Renderer::new(pb.clone(), PLAYBACK_HZ).unwrap();
        let mut buf = vec![0.0; PLAYBACK_FRAME_SAMPLES];
        let result = renderer.render(&mut buf);
        assert!(!result.flushed);
        assert!(!result.underrun);
        assert!(buf.iter().all(|s| (*s - 0.25).abs() < 1e-6));
    }

    #[test]
    fn barge_in_silences_this_period_and_keeps_the_next_push() {
        let pb = Playback::new(4_096);
        pb.push(&vec![0.2; 1_000]);
        pb.barge_in();
        assert!(pb.push_pcm16(&[16_384; 200], PLAYBACK_HZ).unwrap() > 0);
        let mut renderer = Renderer::new(pb.clone(), PLAYBACK_HZ).unwrap();
        let mut buf = vec![0.0; 500];
        let first = renderer.render(&mut buf);
        assert!(first.flushed);
        assert!(buf.iter().all(|s| *s == 0.0));
        let honor = pb.flush_honor_ns().unwrap();
        assert!(honor < 50_000_000, "flush honor {honor} ns");
        let second = renderer.render(&mut buf);
        assert!(!second.flushed);
        assert!((buf[0] - 0.5).abs() < 1e-4, "buf0 {}", buf[0]);
        assert!((buf[199] - 0.5).abs() < 1e-4);
        assert_eq!(buf[200], 0.0);
        assert!(pb.discarded() >= 1_000);
    }

    #[test]
    fn duck_gain_scales_without_flushing() {
        let pb = Playback::new(1_024);
        pb.set_gain(DUCK_GAIN);
        pb.push(&[1.0; 32]);
        let mut renderer = Renderer::new(pb, PLAYBACK_HZ).unwrap();
        let mut buf = [0.0; 32];
        let result = renderer.render(&mut buf);
        assert!(!result.flushed);
        assert!(buf.iter().all(|s| (*s - DUCK_GAIN).abs() < 1e-5));
    }

    #[test]
    fn idle_render_is_not_an_underrun() {
        let pb = Playback::new(1_024);
        let mut renderer = Renderer::new(pb.clone(), PLAYBACK_HZ).unwrap();
        let mut buf = [0.0; 64];
        let result = renderer.render(&mut buf);
        assert!(!result.underrun);
        assert_eq!(pb.underruns(), 0);
        assert!(buf.iter().all(|s| *s == 0.0));
    }

    #[test]
    fn converted_render_reaches_48k() {
        let pb = Playback::new(8_192);
        pb.push(&[0.5; 2_400]);
        let mut renderer = Renderer::new(pb, 48_000).unwrap();
        let mut buf = vec![0.0; 4_800];
        let result = renderer.render(&mut buf);
        assert!(!result.flushed, "{result:?}");
        assert!(result.produced > 4_000, "{result:?}");
        assert!(buf[..100].iter().all(|s| (*s - 0.5).abs() < 1e-4));
    }

    #[test]
    fn piper_rate_block_is_accepted() {
        let pb = Playback::new(32_768);
        let n = pb.push_pcm16(&[1_000; 2_205], 22_050).unwrap();
        assert_eq!(n, 2_205);
        assert!(pb.queued() >= 2_300);
        assert!(pb.queued() <= 2_450);
    }

    #[test]
    fn flush_from_another_thread_is_honored() {
        let pb = Playback::new(8_192);
        pb.push(&vec![0.3; 4_000]);
        let other = pb.clone();
        let handle = thread::spawn(move || {
            thread::sleep(Duration::from_millis(1));
            other.barge_in();
        });
        let mut renderer = Renderer::new(pb.clone(), PLAYBACK_HZ).unwrap();
        let mut buf = vec![0.0; 256];
        let mut silenced = false;
        for _ in 0..100 {
            if renderer.render(&mut buf).flushed {
                silenced = true;
                assert!(buf.iter().all(|s| *s == 0.0));
                break;
            }
            thread::sleep(Duration::from_millis(1));
        }
        handle.join().unwrap();
        if !silenced {
            silenced = renderer.render(&mut buf).flushed;
        }
        assert!(silenced);
        assert!(pb.flush_honor_ns().unwrap() < 50_000_000);
    }
}
