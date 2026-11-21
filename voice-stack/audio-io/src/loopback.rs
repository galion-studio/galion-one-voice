// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! In-process speaker → microphone loop.
//!
//! One [`Loopback::tick`] is one HAL period: render the playback ring at the
//! device rate, then feed those samples to the 16 kHz capture assembler.
//! No device is opened. The clock is virtual. `period` samples at
//! `device_hz` advance time by that many samples.
//!
//! This is the path the tests and `galion-sam-audio-io measure` use when
//! `/dev/snd` is missing. It is not an acoustic round trip.

use crate::capture::{CaptureAssembler, CaptureFrame, CaptureQueue};
use crate::playback::{Playback, Renderer};
use crate::AudioError;

pub struct Loopback {
    pub playback: Playback,
    renderer: Renderer,
    assembler: CaptureAssembler,
    capture: CaptureQueue,
    scratch: Vec<f32>,
    now_ns: u64,
    period_ns: u64,
}

impl Loopback {
    /// `device_hz` is the fake sound-card rate. `period` is one callback,
    /// in device samples. 48000 and 480 is a 10 ms period.
    pub fn new(device_hz: u32, period: usize) -> Result<Self, AudioError> {
        if device_hz == 0 || period == 0 {
            return Err(AudioError::Rate);
        }
        let playback = Playback::default();
        let renderer = Renderer::new(playback.clone(), device_hz)?;
        let period_ns = (u128::from(period as u64) * 1_000_000_000) / u128::from(device_hz);
        Ok(Self {
            playback,
            renderer,
            assembler: CaptureAssembler::new(device_hz)?,
            capture: CaptureQueue::new(64),
            scratch: vec![0.0; period],
            now_ns: 0,
            period_ns: period_ns as u64,
        })
    }

    pub fn capture(&self) -> CaptureQueue {
        self.capture.clone()
    }

    pub fn now_ns(&self) -> u64 {
        self.now_ns
    }

    /// Render one period and return the 20 ms frames sealed during it.
    pub fn tick(&mut self) -> Vec<CaptureFrame> {
        self.scratch.fill(0.0);
        let _ = self.renderer.render(&mut self.scratch);
        self.now_ns = self.now_ns.saturating_add(self.period_ns);
        self.capture.note_callback();
        self.assembler
            .push_mono(&self.scratch, self.now_ns, None, &self.capture);
        let mut frames = Vec::new();
        while let Some(frame) = self.capture.pop() {
            frames.push(frame);
        }
        frames
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CAPTURE_HZ, PLAYBACK_HZ};

    #[test]
    fn impulse_lands_in_the_first_frame_within_one_hop() {
        let mut loopback = Loopback::new(48_000, 480).unwrap();
        let mut burst = vec![0.0f32; PLAYBACK_HZ as usize];
        for sample in burst.iter_mut().take(480) {
            *sample = 0.8;
        }
        assert!(loopback.playback.try_push_all(&burst));
        let mut frames = Vec::new();
        for _ in 0..8 {
            frames.extend(loopback.tick());
            if !frames.is_empty() {
                break;
            }
        }
        assert!(!frames.is_empty(), "no capture frame");
        let frame = &frames[0];
        assert_eq!(
            frame.ready_ns, 20_000_000,
            "virtual ready {}",
            frame.ready_ns
        );
        let hot = frame
            .pcm
            .iter()
            .position(|s| *s > 8_000)
            .expect("impulse missing from the first frame");
        assert!(hot < 32, "impulse at capture sample {hot}");
        let _ = CAPTURE_HZ;
    }
}
