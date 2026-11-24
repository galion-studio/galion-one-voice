// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Listener-feeling probe. Compiled only when `crates/horbm-ear/src/listen.rs` is on the tree.

fn main() {
    if horbm_ear::listen::PERSONA != "samantha" {
        eprintln!("listener persona is not samantha");
        std::process::exit(1);
    }
    let mut ear = horbm_ear::Ear::new();
    let mut pcm = vec![0.0f32; horbm_ear::FRAME_SAMPLES];
    for (index, sample) in pcm.iter_mut().enumerate() {
        let t = index as f32 / f32::from(horbm_ear::SAMPLE_RATE);
        *sample = (t * 190.0 * std::f32::consts::TAU).sin() * 0.2;
    }
    let horbm_ear::FrameOut::Heard(frame) = ear.push(&pcm) else {
        eprintln!("listener had no prosody frame");
        std::process::exit(1);
    };
    let started = std::time::Instant::now();
    let feel = horbm_ear::listen::listener_feel(&frame, "where is the kettle");
    let ns = started.elapsed().as_nanos();
    std::hint::black_box(feel.unclaimed());
    println!("listener_feeling=real");
    println!("listener_feeling_ns={ns}");
}
