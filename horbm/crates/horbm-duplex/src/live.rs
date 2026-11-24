// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Assemble device slices into one duplex frame.
//!
//! The duplex clock moves only when [`FramePump::push`] returns a full
//! 80 ms frame. Waiting on a capture queue is the host's job and is not
//! this clock.

use crate::FRAME_SAMPLES;

/// Collects PCM until an 80 ms frame is full. Leftover samples stay here.
#[derive(Clone, Debug)]
pub struct FramePump {
    buf: Vec<i16>,
}

impl Default for FramePump {
    fn default() -> Self {
        Self::new()
    }
}

impl FramePump {
    #[must_use]
    pub fn new() -> Self {
        Self {
            buf: Vec::with_capacity(FRAME_SAMPLES),
        }
    }

    #[must_use]
    pub fn pending(&self) -> usize {
        self.buf.len()
    }

    /// Take every complete frame out of `pcm`. A short slice only appends.
    pub fn push(&mut self, pcm: &[i16]) -> Vec<Vec<i16>> {
        self.buf.extend_from_slice(pcm);
        let mut frames = Vec::new();
        while self.buf.len() >= FRAME_SAMPLES {
            let frame: Vec<i16> = self.buf.drain(..FRAME_SAMPLES).collect();
            frames.push(frame);
        }
        frames
    }
}
