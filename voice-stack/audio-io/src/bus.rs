// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! The two methods the heartbeat actually calls.
//!
//! `AudioIn` and `AudioOut` match `galion_sam_contract::audio` on the
//! architecture branch (`fill` / `sample_rate` / `name`, and `write_frame` /
//! `stop` / `sample_rate`). That crate is not on this branch, so the types
//! live here with the same fields. `PcmFrame` is PCM the caller already
//! holds. It is not a recording of anyone.
//!
//! `HeartbeatIn::fill` copies one 24 kHz frame out of the capture ring.
//! Shortfalls are silence. `Playback::stop` is `barge_in`.

use crate::playback::Playback;
use crate::ring::Spsc;
use crate::{AudioError, PLAYBACK_HZ};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// One mono PCM16 buffer. Same fields as the contract's `PcmFrame`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PcmFrame {
    pub seq: u64,
    pub t_ms: u32,
    pub sample_rate: u32,
    pub samples: Vec<i16>,
}

/// Pull one heartbeat frame. Same three methods as `MicSource` and the
/// contract's `AudioIn`.
pub trait AudioIn {
    fn fill(&mut self, tick: u64, out: &mut [f32]);
    fn sample_rate(&self) -> u32;
    fn name(&self) -> &str;
}

/// Speaker. `stop` is the barge-in hook.
pub trait AudioOut {
    fn write_frame(&mut self, frame: &PcmFrame) -> Result<(), AudioError>;
    fn stop(&mut self);
    fn sample_rate(&self) -> u32;
}

struct InInner {
    ring: Spsc<f32>,
    name: String,
    overruns: AtomicU64,
}

/// 24 kHz mic frames for the 80 ms bus. The capture callback pushes.
/// `fill` pops. Clone shares the ring.
#[derive(Clone)]
pub struct HeartbeatIn {
    inner: Arc<InInner>,
}

impl HeartbeatIn {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            inner: Arc::new(InInner {
                ring: Spsc::new(32_768),
                name: name.into(),
                overruns: AtomicU64::new(0),
            }),
        }
    }

    /// Device-rate samples already converted to 24 kHz.
    pub fn push(&self, samples: &[f32]) -> usize {
        let n = self.inner.ring.push_slice(samples);
        if n < samples.len() {
            self.inner.overruns.fetch_add(1, Ordering::Relaxed);
        }
        n
    }

    pub fn overruns(&self) -> u64 {
        self.inner.overruns.load(Ordering::Relaxed)
    }

    pub fn queued(&self) -> usize {
        self.inner.ring.queued()
    }
}

impl AudioIn for HeartbeatIn {
    fn fill(&mut self, tick: u64, out: &mut [f32]) {
        let _ = tick;
        let n = self.inner.ring.pop_slice(out);
        if n < out.len() {
            out[n..].fill(0.0);
        }
    }

    fn sample_rate(&self) -> u32 {
        PLAYBACK_HZ
    }

    fn name(&self) -> &str {
        &self.inner.name
    }
}

impl AudioOut for Playback {
    fn write_frame(&mut self, frame: &PcmFrame) -> Result<(), AudioError> {
        if frame.sample_rate == 0 {
            return Err(AudioError::Rate);
        }
        match self.push_pcm16(&frame.samples, frame.sample_rate)? {
            0 if !frame.samples.is_empty() => Err(AudioError::Overrun),
            _ => Ok(()),
        }
    }

    fn stop(&mut self) {
        self.barge_in();
    }

    fn sample_rate(&self) -> u32 {
        PLAYBACK_HZ
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PLAYBACK_FRAME_SAMPLES, PLAYBACK_HZ};

    #[test]
    fn fill_returns_one_heartbeat_frame_at_24k() {
        let mut mic = HeartbeatIn::new("mic");
        assert_eq!(mic.sample_rate(), 24_000);
        assert_eq!(mic.name(), "mic");
        assert_eq!(
            mic.push(&[0.25; PLAYBACK_FRAME_SAMPLES]),
            PLAYBACK_FRAME_SAMPLES
        );
        let mut out = [0.0; PLAYBACK_FRAME_SAMPLES];
        mic.fill(0, &mut out);
        assert!(out.iter().all(|s| (*s - 0.25).abs() < 1e-6));
        mic.fill(1, &mut out);
        assert!(out.iter().all(|s| *s == 0.0));
    }

    #[test]
    fn stop_drops_a_written_frame() {
        let mut speaker = Playback::new(8_192);
        let frame = PcmFrame {
            seq: 1,
            t_ms: 0,
            sample_rate: PLAYBACK_HZ,
            samples: vec![8_000; 320],
        };
        speaker.write_frame(&frame).unwrap();
        assert!(speaker.queued() > 0);
        speaker.stop();
        let mut renderer = crate::Renderer::new(speaker.clone(), PLAYBACK_HZ).unwrap();
        let mut buf = [0.0; 320];
        let rendered = renderer.render(&mut buf);
        assert!(rendered.flushed);
        assert!(buf.iter().all(|s| *s == 0.0));
    }
}
