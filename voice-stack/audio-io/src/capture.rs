// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! 16 kHz mono capture frames of 20 ms (320 PCM16 samples).
//!
//! The live device callback owns a [`CaptureAssembler`] and pushes finished
//! frames into a [`CaptureQueue`]. The queue is the same lock-free ring as
//! playback. A full queue drops the new frame and counts an overrun. The
//! callback never blocks.
//!
//! [`FileCapture`] is the headless source. It reads PCM (or a WAV) and emits
//! the same frame type on a virtual 20 ms clock. It does not open a device.

use crate::convert::{downmix_i16, f32_to_i16, i16_to_f32};
use crate::resample::BoxResampler;
use crate::ring::Spsc;
use crate::wav;
use crate::{AudioError, CAPTURE_FRAME_SAMPLES, CAPTURE_HZ};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// One 20 ms frame ready for VAD or the Vosk worker.
#[derive(Clone, Copy, Debug)]
pub struct CaptureFrame {
    pub pcm: [i16; CAPTURE_FRAME_SAMPLES],
    /// Counts from 0 for this assembler.
    pub seq: u64,
    /// Caller clock when the first sample of the frame was accepted.
    pub started_ns: u64,
    /// Caller clock when the 320th sample sealed the frame.
    pub ready_ns: u64,
    /// Host `callback - capture`, microseconds, when the device reported both.
    pub device_latency_us: u32,
    pub device_latency_known: bool,
}

impl Default for CaptureFrame {
    fn default() -> Self {
        Self {
            pcm: [0; CAPTURE_FRAME_SAMPLES],
            seq: 0,
            started_ns: 0,
            ready_ns: 0,
            device_latency_us: 0,
            device_latency_known: false,
        }
    }
}

impl CaptureFrame {
    /// Float view in [-1, 1], the shape the one-dot page's VAD consumes.
    pub fn pcm_f32(&self) -> [f32; CAPTURE_FRAME_SAMPLES] {
        let mut out = [0.0; CAPTURE_FRAME_SAMPLES];
        for (dst, src) in out.iter_mut().zip(self.pcm.iter()) {
            *dst = i16_to_f32(*src);
        }
        out
    }
}

struct CaptureCounters {
    overruns: AtomicU64,
    pushed: AtomicU64,
    callbacks: AtomicU64,
}

/// Finished frames. Clone it into the audio callback and into the consumer.
#[derive(Clone)]
pub struct CaptureQueue {
    ring: Spsc<CaptureFrame>,
    stats: Arc<CaptureCounters>,
}

impl CaptureQueue {
    pub fn new(frames: usize) -> Self {
        Self {
            ring: Spsc::new(frames),
            stats: Arc::new(CaptureCounters {
                overruns: AtomicU64::new(0),
                pushed: AtomicU64::new(0),
                callbacks: AtomicU64::new(0),
            }),
        }
    }

    pub fn push(&self, frame: CaptureFrame) -> bool {
        if self.ring.push(frame) {
            self.stats.pushed.fetch_add(1, Ordering::Relaxed);
            true
        } else {
            self.stats.overruns.fetch_add(1, Ordering::Relaxed);
            false
        }
    }

    pub fn pop(&self) -> Option<CaptureFrame> {
        self.ring.pop()
    }

    pub fn len(&self) -> usize {
        self.ring.queued()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn overruns(&self) -> u64 {
        self.stats.overruns.load(Ordering::Relaxed)
    }

    pub fn pushed(&self) -> u64 {
        self.stats.pushed.load(Ordering::Relaxed)
    }

    pub fn note_callback(&self) {
        self.stats.callbacks.fetch_add(1, Ordering::Relaxed);
    }

    pub fn callbacks(&self) -> u64 {
        self.stats.callbacks.load(Ordering::Relaxed)
    }
}

#[derive(Debug)]
struct FrameSlicer {
    buf: [i16; CAPTURE_FRAME_SAMPLES],
    fill: usize,
    seq: u64,
    started_ns: u64,
}

impl Default for FrameSlicer {
    fn default() -> Self {
        Self {
            buf: [0; CAPTURE_FRAME_SAMPLES],
            fill: 0,
            seq: 0,
            started_ns: 0,
        }
    }
}

impl FrameSlicer {
    fn push(
        &mut self,
        sample: i16,
        now_ns: u64,
        latency_us: u32,
        known: bool,
    ) -> Option<CaptureFrame> {
        if self.fill == 0 {
            self.started_ns = now_ns;
        }
        self.buf[self.fill] = sample;
        self.fill += 1;
        if self.fill < CAPTURE_FRAME_SAMPLES {
            return None;
        }
        self.fill = 0;
        let seq = self.seq;
        self.seq = self.seq.wrapping_add(1);
        Some(CaptureFrame {
            pcm: self.buf,
            seq,
            started_ns: self.started_ns,
            ready_ns: now_ns,
            device_latency_us: latency_us,
            device_latency_known: known,
        })
    }
}

/// Device-rate mono f32 in, 16 kHz frames out.
pub(crate) struct CaptureAssembler {
    boxer: BoxResampler,
    slicer: FrameSlicer,
}

impl CaptureAssembler {
    pub(crate) fn new(in_hz: u32) -> Result<Self, AudioError> {
        Ok(Self {
            boxer: BoxResampler::new(in_hz, CAPTURE_HZ)?,
            slicer: FrameSlicer::default(),
        })
    }

    pub(crate) fn push_mono(
        &mut self,
        mono: &[f32],
        now_ns: u64,
        latency_us: Option<u32>,
        queue: &CaptureQueue,
    ) {
        let known = latency_us.is_some();
        let latency_us = latency_us.unwrap_or(0);
        let slicer = &mut self.slicer;
        self.boxer.push(mono, |sample| {
            if let Some(frame) = slicer.push(f32_to_i16(sample), now_ns, latency_us, known) {
                queue.push(frame);
            }
        });
    }
}

/// Headless 16 kHz source. Frames use a virtual clock: frame `n` is ready at
/// `n * 20 ms`. That is not a wall-clock capture latency.
pub struct FileCapture {
    pcm: Vec<i16>,
    pos: usize,
    seq: u64,
}

impl FileCapture {
    pub fn from_pcm(samples: &[i16], channels: u16, rate: u32) -> Result<Self, AudioError> {
        if rate == 0 || channels == 0 {
            return Err(AudioError::Rate);
        }
        let mono = downmix_i16(samples, channels as usize);
        let float: Vec<f32> = mono.iter().copied().map(i16_to_f32).collect();
        let mut boxer = BoxResampler::new(rate, CAPTURE_HZ)?;
        let mut pcm = Vec::new();
        boxer.push(&float, |sample| pcm.push(f32_to_i16(sample)));
        Ok(Self {
            pcm,
            pos: 0,
            seq: 0,
        })
    }

    pub fn from_wav(path: &Path) -> Result<Self, AudioError> {
        let wav = wav::read_wav(path)?;
        Self::from_pcm(&wav.samples, wav.channels, wav.rate)
    }

    pub fn remaining(&self) -> usize {
        self.pcm.len().saturating_sub(self.pos)
    }

    pub fn next_frame(&mut self) -> Option<CaptureFrame> {
        if self.pos + CAPTURE_FRAME_SAMPLES > self.pcm.len() {
            return None;
        }
        let mut pcm = [0; CAPTURE_FRAME_SAMPLES];
        pcm.copy_from_slice(&self.pcm[self.pos..self.pos + CAPTURE_FRAME_SAMPLES]);
        self.pos += CAPTURE_FRAME_SAMPLES;
        let seq = self.seq;
        self.seq += 1;
        let ready_ns = seq.saturating_mul(20_000_000);
        Some(CaptureFrame {
            pcm,
            seq,
            started_ns: ready_ns,
            ready_ns: ready_ns.saturating_add(20_000_000),
            device_latency_us: 0,
            device_latency_known: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_second_of_48k_becomes_fifty_frames() {
        let queue = CaptureQueue::new(64);
        let mut asm = CaptureAssembler::new(48_000).unwrap();
        let block = [0.25f32; 4_800];
        for i in 0..10 {
            asm.push_mono(&block, (i + 1) * 100_000_000, Some(1_500), &queue);
        }
        assert_eq!(queue.pushed(), 50);
        let frame = queue.pop().unwrap();
        assert_eq!(frame.seq, 0);
        assert!(frame.device_latency_known);
        assert_eq!(frame.device_latency_us, 1_500);
        let sample = frame.pcm[0];
        assert!((sample - 8_192).abs() <= 1, "sample {sample}");
        assert_eq!(frame.pcm_f32().len(), 320);
    }

    #[test]
    fn a_short_tail_does_not_invent_a_frame() {
        let mut file = FileCapture::from_pcm(&[1000; 400], 1, 16_000).unwrap();
        let frame = file.next_frame().unwrap();
        assert_eq!(frame.pcm[0], 1000);
        assert_eq!(frame.ready_ns, 20_000_000);
        assert!(file.next_frame().is_none());
        assert_eq!(file.remaining(), 80);
    }

    #[test]
    fn overrun_drops_the_new_frame() {
        let queue = CaptureQueue::new(2);
        let frame = CaptureFrame::default();
        assert!(queue.push(frame));
        assert!(queue.push(frame));
        assert!(!queue.push(frame));
        assert_eq!(queue.overruns(), 1);
        assert_eq!(queue.len(), 2);
    }
}
