// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Fixed playback queue. The mouth enqueues a whole utterance and the pull
//! dequeues one frame. The storage does not grow.

use horbm_duplex::FRAME_SAMPLES;

/// Samples waiting to be played. Capacity is fixed at construction.
#[derive(Debug)]
pub struct OutputBuffer {
    data: Vec<i16>,
    cap: usize,
    head: usize,
    len: usize,
    max_depth: usize,
    overflows: u64,
}

impl OutputBuffer {
    #[must_use]
    pub fn with_capacity_samples(cap: usize) -> Self {
        Self {
            data: vec![0; cap],
            cap,
            head: 0,
            len: 0,
            max_depth: 0,
            overflows: 0,
        }
    }

    #[must_use]
    pub fn depth(&self) -> usize {
        self.len
    }

    #[must_use]
    pub fn max_depth(&self) -> usize {
        self.max_depth
    }

    #[must_use]
    pub fn overflows(&self) -> u64 {
        self.overflows
    }

    /// Copy `pcm` onto the tail. A frame that does not fit is counted and
    /// dropped. The buffer is not reallocated.
    pub fn enqueue(&mut self, pcm: &[i16]) {
        if pcm.is_empty() {
            return;
        }
        if self.len.saturating_add(pcm.len()) > self.cap {
            self.overflows = self.overflows.saturating_add(1);
            return;
        }
        for sample in pcm {
            let index = (self.head + self.len) % self.cap;
            self.data[index] = *sample;
            self.len += 1;
        }
        self.max_depth = self.max_depth.max(self.len);
    }

    /// Copy one frame off the head into `out`.
    pub fn dequeue_frame(&mut self, out: &mut [i16]) -> bool {
        if out.len() != FRAME_SAMPLES || self.len < FRAME_SAMPLES {
            return false;
        }
        for sample in out {
            *sample = self.data[self.head];
            self.head = (self.head + 1) % self.cap;
            self.len -= 1;
        }
        true
    }

    pub fn clear_pending(&mut self) {
        self.head = 0;
        self.len = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depth_falls_as_frames_leave_and_a_full_buffer_does_not_grow() {
        let mut buffer = OutputBuffer::with_capacity_samples(FRAME_SAMPLES * 2);
        let frame = vec![3i16; FRAME_SAMPLES];
        buffer.enqueue(&frame);
        buffer.enqueue(&frame);
        assert_eq!(buffer.depth(), FRAME_SAMPLES * 2);
        buffer.enqueue(&frame);
        assert_eq!(buffer.overflows(), 1);
        assert_eq!(buffer.depth(), FRAME_SAMPLES * 2);
        let mut out = [0i16; FRAME_SAMPLES];
        assert!(buffer.dequeue_frame(&mut out));
        assert_eq!(buffer.depth(), FRAME_SAMPLES);
        assert_eq!(buffer.max_depth(), FRAME_SAMPLES * 2);
        assert!(out.iter().all(|sample| *sample == 3));
    }
}
