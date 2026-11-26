// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! Wall-clock cost of one 80 ms frame. The timed region is
//! `set_controls` plus `process`. The scratch buffer is allocated before
//! the clock starts.

use std::hint::black_box;
use std::time::Instant;

use crate::{frame_samples, MouthControls, SourceFilter, FRAME_MS};

/// processing time / audio duration. Below 1 the frame finishes inside its
/// own 80 ms. `ns_per_frame` is the mean wall time of one frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameReport {
    pub sample_rate: u32,
    pub samples_per_frame: usize,
    pub frames: u32,
    pub ns_per_frame: u64,
    /// 80 ms, in nanoseconds.
    pub audio_ns: u64,
    pub rtf: f64,
}

pub fn measure_frame_cost(sample_rate: u32, frames: u32, controls: MouthControls) -> FrameReport {
    let frames = frames.max(1);
    let samples_per_frame = frame_samples(sample_rate).max(1);
    let mut filter = SourceFilter::rest(sample_rate);
    let mut buf = vec![0.0f32; samples_per_frame];
    for (i, sample) in buf.iter_mut().enumerate() {
        let step = (i % 40) as f32;
        *sample = (step / 20.0) - 1.0;
        *sample *= 0.15;
    }
    filter.set_controls(controls);
    for _ in 0..4 {
        filter.process(&mut buf);
        black_box(buf[0]);
    }
    let started = Instant::now();
    for _ in 0..frames {
        // `black_box` keeps the retune inside the frame. A constant control
        // would otherwise be hoisted out of the loop.
        filter.set_controls(black_box(controls));
        filter.process(&mut buf);
        black_box(buf[0]);
    }
    let total_ns = u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
    let ns_per_frame = total_ns / u64::from(frames);
    let audio_ns = u64::from(FRAME_MS) * 1_000_000;
    let rtf = if audio_ns == 0 {
        f64::INFINITY
    } else {
        ns_per_frame as f64 / audio_ns as f64
    };
    FrameReport {
        sample_rate,
        samples_per_frame,
        frames,
        ns_per_frame,
        audio_ns,
        rtf,
    }
}
