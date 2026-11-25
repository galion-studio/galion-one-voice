// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! End-of-turn probe. Compiled only when `crates/horbm-ear/src/eot.rs` is on the tree.

fn main() {
    let mut ear = horbm_ear::Ear::new();
    let mut pcm = vec![0.0f32; horbm_ear::FRAME_SAMPLES];
    for (index, sample) in pcm.iter_mut().enumerate() {
        let t = index as f32 / horbm_ear::SAMPLE_RATE as f32;
        *sample = (t * 180.0 * std::f32::consts::TAU).sin() * 0.2;
    }
    let heard = ear.push(&pcm);
    let mut scorer = horbm_ear::Scorer::bundled();
    let started = std::time::Instant::now();
    let message = scorer.push(heard, "where is the kettle");
    let ns = started.elapsed().as_nanos();
    std::hint::black_box(message);
    println!("end_of_turn=real");
    println!("end_of_turn_ns={ns}");
}
