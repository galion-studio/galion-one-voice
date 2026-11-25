// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! In-process output buffer.
//!
//! `galion-sam-audio-io::Playback` is the speaker ring. It links cpal, which
//! needs the ALSA headers on Linux, so this smoke does not take that crate.
//! Samples are queued here and `barge` drops whatever is still queued. The
//! stop time is the wall clock of that drop plus one silent render. A stub
//! mouth never pushes, and then there is no stop time.

use std::time::Instant;

use horbm_voice::FRAME_SAMPLES;

#[derive(Debug, Default)]
pub struct Loopback {
    samples: Vec<i16>,
    pos: usize,
}

impl Loopback {
    pub fn push(&mut self, pcm: &[i16]) {
        self.samples.extend_from_slice(pcm);
    }

    pub fn queued(&self) -> usize {
        self.samples.len().saturating_sub(self.pos)
    }

    fn render_period(&mut self) -> bool {
        let start = self.pos;
        let end = (self.pos + FRAME_SAMPLES).min(self.samples.len());
        let audible = self.samples[start..end].iter().any(|sample| *sample != 0);
        self.pos = end;
        std::hint::black_box(audible)
    }

    /// Drop queued audio. `None` when nothing was queued, so a caller cannot
    /// report a stop time for silence it did not interrupt.
    pub fn barge(&mut self) -> Option<u64> {
        if self.queued() == 0 {
            return None;
        }
        let started = Instant::now();
        self.samples.clear();
        self.pos = 0;
        let audible = self.render_period();
        let ns = u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
        if audible {
            None
        } else {
            Some(ns)
        }
    }
}
