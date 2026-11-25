// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Listener-feeling probe. Compiled only when `crates/horbm-ear/src/listen.rs` is on the tree.

fn main() {
    let named = std::env::var(horbm_ear::listen::DEV_PERSONA_ENV).ok();
    let resolved = horbm_ear::listen::configured_profile(named.as_deref());
    let from_env = horbm_ear::listen::profile_from_env();
    if !same_profile(resolved, from_env) {
        eprintln!("listener profile is not the configured one");
        std::process::exit(1);
    }
    if horbm_ear::listen::configured_profile(None).is_some()
        || horbm_ear::listen::configured_profile(Some("")).is_some()
    {
        eprintln!("listener substituted a profile for a missing id");
        std::process::exit(1);
    }

    let mut ear = horbm_ear::Ear::new();
    let mut pcm = vec![0.0f32; horbm_ear::FRAME_SAMPLES];
    for (index, sample) in pcm.iter_mut().enumerate() {
        let t = index as f32 / horbm_ear::SAMPLE_RATE as f32;
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
    if let Some(profile) = resolved {
        let mut core = horbm_ear::port::EmotionCore::at_rest();
        if horbm_ear::listen::hear_frame(
            &mut ear,
            &mut core,
            &mut (),
            &pcm,
            "where is the kettle",
            profile,
        )
        .is_err()
        {
            eprintln!("listener step failed");
            std::process::exit(1);
        }
    }
    println!("listener_feeling=real");
    println!("listener_feeling_ns={ns}");
}

fn same_profile(
    left: Option<&horbm_ear::listen::PersonaProfile>,
    right: Option<&horbm_ear::listen::PersonaProfile>,
) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => std::ptr::eq(left, right),
        (None, None) => true,
        _ => false,
    }
}
