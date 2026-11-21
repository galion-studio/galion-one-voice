// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Wall-clock budget for one mood update. The numbers printed here are the
//! measurement. The only constant is the 80 ms ceiling. This file does not
//! record a duration from a previous run.

use std::hint::black_box;
use std::time::{Duration, Instant};

use galion_sam_empathy::{Empathy, InboundSignal, TickIn, MOOD_BUDGET_US};

const N: usize = 2_000;
const WARMUP: usize = 64;

struct Stats {
    min: Duration,
    p50: Duration,
    p95: Duration,
    max: Duration,
}

fn measure(mut body: impl FnMut(usize)) -> Stats {
    for i in 0..WARMUP {
        body(i);
    }
    let mut samples = Vec::with_capacity(N);
    for i in 0..N {
        let started = Instant::now();
        body(i);
        samples.push(started.elapsed());
    }
    samples.sort();
    Stats {
        min: samples[0],
        p50: rank(&samples, 0.50),
        p95: rank(&samples, 0.95),
        max: samples[samples.len() - 1],
    }
}

fn rank(sorted: &[Duration], p: f64) -> Duration {
    let n = sorted.len();
    let index = ((p * n as f64).ceil() as usize).clamp(1, n) - 1;
    sorted[index]
}

fn report(kind: &str, stats: &Stats) {
    eprintln!(
        "MEASURE empathy_{kind} n={N} min_us={} p50_us={} p95_us={} max_us={} ceiling_us={MOOD_BUDGET_US}",
        stats.min.as_micros(),
        stats.p50.as_micros(),
        stats.p95.as_micros(),
        stats.max.as_micros(),
    );
}

fn under_ceiling(stats: &Stats) {
    let ceiling = Duration::from_micros(u64::from(MOOD_BUDGET_US));
    assert!(
        stats.p95 < ceiling,
        "p95 {} us is over the 80 ms ceiling",
        stats.p95.as_micros()
    );
    assert!(
        stats.max < ceiling,
        "max {} us is over the 80 ms ceiling",
        stats.max.as_micros()
    );
}

#[test]
fn mood_update_fits_in_one_80ms_frame() {
    let mut layer = Empathy::new();
    let signal = InboundSignal {
        arousal: 0.82,
        valence: -0.55,
        dominance: 0.66,
        confidence: 0.62,
    };
    let texts = [
        "I am angry about the gate",
        "I am so tired of this",
        "I am happy",
        "what is the gate",
    ];
    let stats = measure(|i| {
        let text = texts[i % texts.len()];
        let inbound = if i % 2 == 0 { Some(signal) } else { None };
        black_box(layer.update_mood(inbound, text, 80));
    });
    report("mood", &stats);
    under_ceiling(&stats);
}

#[test]
fn a_full_tick_fits_in_one_80ms_frame() {
    let mut layer = Empathy::new();
    let signal = InboundSignal {
        arousal: 0.7,
        valence: -0.4,
        dominance: 0.45,
        confidence: 0.55,
    };
    let stats = measure(|i| {
        let out = layer.tick(TickIn {
            inbound: Some(signal),
            asr: "I am anxious and I want the notes",
            asr_final: i == 0,
            invite_name: i % 5 == 0,
            frame_ms: 80,
        });
        black_box(out.e);
        black_box(out.mood.valence);
    });
    report("tick", &stats);
    under_ceiling(&stats);
}
