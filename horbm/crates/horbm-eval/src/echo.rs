// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Synthetic room for the echo probe.
//!
//! Delay, gain, and the two taps are the room the open echo-cancellation
//! change measures (160 samples, gain 0.08, taps at 48 and 96). The far end
//! is a generated tone or noise, loud enough that the room output sits above
//! the ear floor. No recording.

use horbm_duplex::{FRAME_SAMPLES, SAMPLE_RATE};
use horbm_voice::{AudioFrame, Mouth, MouthPull, SpokenLine};

const DELAY: usize = 160;
const GAIN: f32 = 0.08;
const TAPS: [(usize, f32); 2] = [(48, 0.30), (96, 0.12)];
pub const GRACE_FRAMES: usize = 8;
pub const SCORE_FRAMES: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Far {
    Tone,
    Noise,
}

pub struct Room {
    hist: Vec<f32>,
}

impl Room {
    pub fn new() -> Self {
        Self { hist: Vec::new() }
    }

    pub fn render(&mut self, far: &[i16]) -> Vec<i16> {
        let start = self.hist.len();
        for sample in far {
            self.hist.push(f32::from(*sample) / 32_768.0);
        }
        let mut mic = vec![0i16; far.len()];
        for (index, sample) in mic.iter_mut().enumerate() {
            let at = start + index;
            let mut echo = GAIN * tap(&self.hist, at, DELAY);
            for (lag, scale) in TAPS {
                echo += GAIN * scale * tap(&self.hist, at, DELAY + lag);
            }
            *sample = unit_to_pcm(echo);
        }
        mic
    }
}

pub struct FarMouth {
    kind: Far,
    left: u32,
    index: usize,
    pending_first: bool,
    last: Vec<i16>,
}

impl FarMouth {
    pub fn new(kind: Far) -> Self {
        Self {
            kind,
            left: 0,
            index: 0,
            pending_first: false,
            last: Vec::new(),
        }
    }

    pub fn last(&self) -> &[i16] {
        &self.last
    }
}

impl Mouth for FarMouth {
    fn id(&self) -> &str {
        "probe"
    }

    fn start(&mut self, line: &SpokenLine) {
        self.last.clear();
        self.index = 0;
        if line.text.trim().is_empty() {
            self.left = 0;
            self.pending_first = false;
            return;
        }
        self.left = 64;
        self.pending_first = true;
    }

    fn pull(&mut self) -> MouthPull {
        if self.left == 0 {
            return MouthPull::Done;
        }
        self.left -= 1;
        let first = self.pending_first;
        self.pending_first = false;
        let origin = self.index.saturating_mul(FRAME_SAMPLES);
        self.index = self.index.saturating_add(1);
        let samples = (0..FRAME_SAMPLES)
            .map(|index| unit_to_pcm(far_unit(self.kind, origin + index)))
            .collect::<Vec<_>>();
        self.last.clone_from(&samples);
        MouthPull::Frame(AudioFrame {
            samples,
            sample_rate: SAMPLE_RATE,
            first,
        })
    }

    fn cancel(&mut self) {
        self.left = 0;
        self.pending_first = false;
        self.last.clear();
    }

    fn speaking(&self) -> bool {
        self.left > 0
    }
}

pub fn user_tone() -> Vec<i16> {
    (0..FRAME_SAMPLES)
        .map(|index| unit_to_pcm(0.25 * unit_tone(index, 200.0)))
        .collect()
}

fn far_unit(kind: Far, index: usize) -> f32 {
    match kind {
        Far::Tone => 0.5 * unit_tone(index, 440.0),
        Far::Noise => 0.4 * unit_noise(index),
    }
}

fn unit_tone(index: usize, hz: f32) -> f32 {
    (std::f32::consts::TAU * hz * index as f32 / SAMPLE_RATE as f32).sin()
}

fn unit_noise(index: usize) -> f32 {
    let mut x = (index as u32)
        .wrapping_mul(0x9E37_79B9)
        .wrapping_add(0xA5A5_1234);
    x ^= x >> 16;
    f32::from((x >> 16) as i16) / 32_768.0
}

fn unit_to_pcm(sample: f32) -> i16 {
    let scaled = f64::from(sample) * 32_768.0;
    if scaled >= 32_767.0 {
        32_767
    } else if scaled <= -32_768.0 {
        -32_768
    } else {
        // The product is inside i16 by the branches above.
        #[allow(clippy::cast_possible_truncation)]
        {
            scaled.round() as i16
        }
    }
}

fn tap(hist: &[f32], index: usize, lag: usize) -> f32 {
    if index >= lag {
        hist[index - lag]
    } else {
        0.0
    }
}
