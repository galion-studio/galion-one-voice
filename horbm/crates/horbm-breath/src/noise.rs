// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Procedural inhale. Same LCG as `crates/galion-heartbeat/src/presence.rs`
//! (seed 13). A parabola window forces the block to start and end at zero.
//! The block is then scaled to [`crate::NEUTRAL_RMS`]. That spectrum is not
//! a measured breath.

use crate::{INHALE_SAMPLES, NEUTRAL_RMS};

pub(crate) fn fill(rng: &mut u64, out: &mut [f32]) -> usize {
    let n = out.len().min(INHALE_SAMPLES);
    if n == 0 {
        return 0;
    }
    let mut energy = 0.0f32;
    for (i, slot) in out.iter_mut().enumerate().take(n) {
        let u = if n == 1 {
            0.0
        } else {
            i as f32 / (n - 1) as f32
        };
        let window = 4.0 * u * (1.0 - u);
        let sample = next_f32(rng) * window;
        *slot = sample;
        energy += sample * sample;
    }
    let mean = energy / n as f32;
    let rms = sqrt_f32(mean);
    if rms > 1.0e-8 {
        let gain = NEUTRAL_RMS / rms;
        for sample in out.iter_mut().take(n) {
            *sample *= gain;
        }
    }
    n
}

fn next_f32(rng: &mut u64) -> f32 {
    *rng = rng
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    let unit = (*rng >> 40) as f32 / 16_777_216.0;
    unit * 2.0 - 1.0
}

/// Newton square root. Eight steps. No libm.
fn sqrt_f32(x: f32) -> f32 {
    if x <= 0.0 || !x.is_finite() {
        return 0.0;
    }
    let mut y = if x >= 1.0 { x } else { 1.0 };
    let mut i = 0;
    while i < 8 {
        y = 0.5 * (y + x / y);
        i += 1;
    }
    if y.is_finite() {
        y
    } else {
        0.0
    }
}
