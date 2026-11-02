// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! W21-02 latency probe for the Mimi passthrough lane (SYNTHETIC: ZeroWeights, IS_STUB = true).
//! `cargo run --release -p galion-student-backend --example mimi_passthrough [demo|no-kernels|full] [frames]`
use galion_student_backend::mimi_lane::{tone_frame, IS_STUB};
use galion_student_backend::{MimiLaneConfig, MimiPassthrough, FRAME_SAMPLES};
use std::time::Instant;

fn main() {
    let mut args = std::env::args().skip(1);
    let which = args.next().unwrap_or_else(|| "demo".into());
    let frames: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(if which == "full" { 2 } else { 50 });
    let cfg = match which.as_str() {
        "full" => MimiLaneConfig::full(),
        "no-kernels" => MimiLaneConfig::no_kernels(),
        _ => MimiLaneConfig::demo(),
    };
    let t0 = Instant::now();
    let mut lane = MimiPassthrough::new(cfg).expect("build");
    let build = t0.elapsed();
    let input: Vec<Vec<f32>> = (0..frames).map(|f| tone_frame(f, 0.5)).collect();
    let mut out = vec![0.0f32; FRAME_SAMPLES];
    let (mut worst, mut total) = (0f64, 0f64);
    for f in &input {
        let t = Instant::now();
        lane.step_frame(f, &mut out).expect("step");
        let ms = t.elapsed().as_secs_f64() * 1e3;
        worst = worst.max(ms);
        total += ms;
    }
    let mean = total / frames as f64;
    println!("mimi-lane {which}: IS_STUB={IS_STUB} build={:.1} ms frames={frames} mean={mean:.3} ms worst={worst:.3} ms budget=80 ms real_time={}",
        build.as_secs_f64() * 1e3, worst < 80.0);
}
