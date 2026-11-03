// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! Stub-path timing for one heartbeat step.
//!
//! This measures `Heartbeat::step` with the RMS/ZCR stub and Rika v0.
//! It is not a model, codec, clone, or host-CPU result, and it is not an
//! EPYC / CX23 / CX43 benchmark. The 1% ceiling (800 µs) is enforced by
//! `scripts/symphony/check-g-latency.sh`, which reads the printed line.
//! The test itself rejects a step that runs past the 80 ms frame.
use galion_emotion::{DefaultConsentGate, Purpose};
use galion_heartbeat::{FrameIn, Heartbeat, HeartbeatConfig, FRAME_MS, FRAME_SAMPLES, RIKA_BITS};
use std::hint::black_box;

fn granted() -> DefaultConsentGate {
    let mut g = DefaultConsentGate::default();
    g.grant(0, "hr", Purpose::PersonalAssistant);
    g
}

fn frame<'a>(pcm: &'a [f32], late: bool, me_speaking: bool) -> galion_heartbeat::FrameIn<'a> {
    FrameIn {
        you_pcm: pcm,
        affect: None,
        tension: None,
        consent_bits: RIKA_BITS,
        purpose: Purpose::PersonalAssistant,
        session_open: true,
        me_speaking,
        late,
    }
}

/// ceil(n * numer/denom) - 1, clamped. Rank index into an ascending sample.
fn rank(n: usize, numer: usize, denom: usize) -> usize {
    let ceil = (n * numer + denom - 1) / denom;
    ceil.saturating_sub(1).min(n - 1)
}

#[test]
fn step_p99_line_is_measured_stub_and_stays_inside_the_frame() {
    assert_eq!(FRAME_MS, 80);
    assert_eq!(FRAME_SAMPLES, 1920);
    // 1% of an 80 ms tick, in microseconds: frame_ms * 1000 / 100 = frame_ms * 10.
    let budget_us: u128 = u128::from(FRAME_MS) * 10;
    assert_eq!(budget_us, 800, "non-inference ceiling is 1% of the 80 ms tick");

    let pcm = vec![0.0f32; FRAME_SAMPLES];
    let mut hb = Heartbeat::new(HeartbeatConfig::rika_v0(), granted());
    for i in 0..32u32 {
        black_box(hb.step(&frame(&pcm, i % 16 == 0, i % 2 == 0)));
    }

    const N: usize = 256;
    let mut samples = Vec::with_capacity(N);
    for i in 0..N {
        let t0 = std::time::Instant::now();
        let out = hb.step(&frame(&pcm, i % 16 == 0, i % 2 == 0));
        let us = t0.elapsed().as_micros();
        black_box(out.index);
        samples.push(us);
    }
    samples.sort_unstable();
    let p50 = samples[rank(N, 50, 100)];
    let p95 = samples[rank(N, 95, 100)];
    let p99 = samples[rank(N, 99, 100)];
    let max_us = *samples.last().unwrap();
    let frame_us = u128::from(FRAME_MS) * 1000;
    let step_duty_ppm = p99 * 1000 / u128::from(FRAME_MS);

    let late = hb.step(&frame(&pcm, true, false));
    assert!(late.paused, "late frame is a pause");
    assert!(!late.me_onset_allowed, "a late frame starts nothing new");
    let short = vec![0.0f32; FRAME_SAMPLES / 2];
    let short_out = hb.step(&frame(&short, false, false));
    assert!(short_out.paused && !short_out.me_onset_allowed);

    let profile = if cfg!(debug_assertions) { "debug" } else { "release" };
    println!(
        "GALION_FRAME_BUDGET kind=measured-stub profile={profile} frame_ms={FRAME_MS} budget_us={budget_us} n={N} p50_us={p50} p95_us={p95} p99_us={p99} max_us={max_us} step_duty_ppm={step_duty_ppm} late_pause=1"
    );
    assert!(
        max_us < frame_us,
        "stub step max {max_us} µs ran past the {frame_us} µs frame"
    );
}
