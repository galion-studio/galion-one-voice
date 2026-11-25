// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Prosody probe. Compiled only when `crates/horbm-ear` is on the tree.

fn main() {
    let sense_off = std::env::var("HORBM_E2E_SENSE")
        .ok()
        .map(|value| {
            let value = value.trim().to_ascii_lowercase();
            value == "0" || value == "off" || value == "deny"
        })
        .unwrap_or(false);
    let mut ear = horbm_ear::Ear::from_config(horbm_ear::EarConfig { sense: !sense_off });
    let mut pcm = vec![0.0f32; horbm_ear::FRAME_SAMPLES];
    if sense_off {
        let out = ear.push(&pcm);
        let denied = matches!(out, horbm_ear::FrameOut::Denied);
        if !ear.sensing() && denied {
            println!("prosody=off");
            return;
        }
        eprintln!("prosody off switch did not deny the frame");
        std::process::exit(1);
    }
    for (index, sample) in pcm.iter_mut().enumerate() {
        let t = index as f32 / horbm_ear::SAMPLE_RATE as f32;
        *sample = (t * 200.0 * std::f32::consts::TAU).sin() * 0.2;
    }
    let started = std::time::Instant::now();
    let out = ear.push(&pcm);
    let ns = started.elapsed().as_nanos();
    if matches!(out, horbm_ear::FrameOut::Heard(_)) {
        println!("prosody=real");
        println!("prosody_ns={ns}");
        return;
    }
    eprintln!("prosody frame was not heard");
    std::process::exit(1);
}
