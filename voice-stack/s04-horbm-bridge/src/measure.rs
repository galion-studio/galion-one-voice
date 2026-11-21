// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Wall-clock samples for a closure. The bin prints these. Tests only check
//! that a mood tick stays inside one 80 ms frame.

use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SampleNs {
    pub n: u32,
    pub min_ns: u128,
    pub p50_ns: u128,
    pub p95_ns: u128,
    pub max_ns: u128,
}

/// Time `n` calls after 64 warm-up calls. `n` must be at least 1.
/// The closure should [`std::hint::black_box`] its result so the compiler
/// keeps the work.
pub fn time_ns(n: usize, mut op: impl FnMut()) -> SampleNs {
    assert!(n >= 1, "time_ns needs at least one sample");
    for _ in 0..64 {
        op();
    }
    let mut xs = Vec::with_capacity(n);
    for _ in 0..n {
        let started = Instant::now();
        op();
        xs.push(started.elapsed().as_nanos());
    }
    xs.sort_unstable();
    let at = |p: f64| {
        let idx = ((xs.len() as f64 - 1.0) * p).round() as usize;
        xs[idx]
    };
    SampleNs {
        n: n as u32,
        min_ns: xs[0],
        p50_ns: at(0.50),
        p95_ns: at(0.95),
        max_ns: xs[xs.len() - 1],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pad::Pad;

    #[test]
    fn one_mood_tick_stays_inside_the_80ms_frame() {
        let sample = time_ns(200, || {
            let pad = std::hint::black_box(Pad::new(0.8, 0.6, 0.7));
            let next = std::hint::black_box(pad.advance_frame());
            std::hint::black_box(next);
        });
        eprintln!(
            "pad.advance_frame n={} min_ns={} p50_ns={} p95_ns={} max_ns={}",
            sample.n, sample.min_ns, sample.p50_ns, sample.p95_ns, sample.max_ns
        );
        assert!(
            sample.max_ns < 80_000_000,
            "mood tick max {} ns exceeds one 80 ms frame",
            sample.max_ns
        );
    }
}
