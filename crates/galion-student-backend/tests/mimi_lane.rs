// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! W21-02: Mimi passthrough lane runs frames without panic, honest stub output, tone-safe.
use galion_presence::backend::ReplayBackend;
use galion_student_backend::mimi_lane::{tone_frame, IS_STUB};
use galion_student_backend::{KernelMode, MimiLaneConfig, MimiLaneError, MimiPassthrough, Monitor, StudentBackend, FRAME_SAMPLES};

fn run(cfg: MimiLaneConfig, frames: u64) -> MimiPassthrough {
    let mut lane = MimiPassthrough::new(cfg).unwrap();
    let mut out = vec![1.0f32; FRAME_SAMPLES];
    for f in 0..frames {
        let r = lane.step_frame(&tone_frame(f, 0.5), &mut out).unwrap();
        assert!(r.codec_ran && !r.paused && r.is_stub);
        assert_eq!(r.frame, f);
        assert!(!r.codes_nonzero, "zero weights → code 0");
        assert!(!r.bypassed);
        assert_eq!(r.codec_peak, 0.0);
        assert!(out.iter().all(|&s| s == 0.0), "zero weights → silence");
    }
    assert_eq!(lane.frames(), frames);
    lane
}

#[test]
fn stub_flag_unchanged() {
    assert!(IS_STUB);
    assert!(galion_mimi::IS_STUB);
}

#[test]
fn demo_zero_weights_runs_frames() {
    let lane = run(MimiLaneConfig::demo(), 12);
    assert_eq!(lane.codes_per_frame(), 8);
    assert!(lane.last_codes().iter().all(|&c| c == 0));
}

#[test]
fn no_kernels_full_shape_runs_frames() {
    run(MimiLaneConfig::no_kernels(), 25);
}

#[test]
fn dry_bypass_is_labelled_and_clamped() {
    let mut lane = MimiPassthrough::new(MimiLaneConfig::demo().with_monitor(Monitor::DryBypass)).unwrap();
    let mut out = vec![0.0f32; FRAME_SAMPLES];
    let tone = tone_frame(0, 0.5);
    let r = lane.step_frame(&tone, &mut out).unwrap();
    assert!(r.bypassed && r.codec_ran && r.sanitized == 0);
    assert_eq!(out, tone);
    // Hot input: clipped to [-1, 1]; NaN → 0.
    let mut hot = tone_frame(1, 3.0);
    hot[0] = f32::NAN;
    let r = lane.step_frame(&hot, &mut out).unwrap();
    assert!(r.sanitized > 0);
    assert!(out.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
    assert_eq!(out[0], 0.0);
}

#[test]
fn short_frame_is_pause_not_glitch() {
    let mut lane = MimiPassthrough::demo().unwrap();
    let mut out = vec![1.0f32; FRAME_SAMPLES];
    let r = lane.step_frame(&[0.1; 100], &mut out).unwrap();
    assert!(r.paused && !r.codec_ran);
    assert!(out.iter().all(|&s| s == 0.0));
    assert_eq!(lane.frames(), 0);
}

#[test]
fn bad_out_buffer_is_error() {
    let mut lane = MimiPassthrough::new(MimiLaneConfig { kernels: KernelMode::NoKernels, ..MimiLaneConfig::demo() }).unwrap();
    let e = lane.step_frame(&tone_frame(0, 0.5), &mut [0.0; 10]).unwrap_err();
    assert_eq!(e, MimiLaneError::Shape { what: "pcm_out", got: 10, want: FRAME_SAMPLES });
}

#[test]
fn reset_restarts_stream() {
    let mut lane = run(MimiLaneConfig::demo(), 3);
    lane.reset();
    assert_eq!(lane.frames(), 0);
    let mut out = vec![0.0f32; FRAME_SAMPLES];
    assert_eq!(lane.step_frame(&tone_frame(0, 0.5), &mut out).unwrap().frame, 0);
}

#[test]
fn student_backend_step_frame() {
    let mut b = StudentBackend::demo().unwrap();
    assert!(b.mimi().is_some() && !b.is_loaded());
    let mut out = vec![1.0f32; FRAME_SAMPLES];
    for f in 0..5 {
        let r = b.step_frame(&tone_frame(f, 0.5), &mut out).unwrap();
        assert!(r.codec_ran && r.is_stub);
    }
    b.reset(); // ReplayBackend::reset also resets the lane
    assert_eq!(b.mimi().unwrap().frames(), 0);
    // No lane → silence, codec not run.
    let mut s = StudentBackend::stub();
    let r = s.step_frame(&tone_frame(0, 0.5), &mut out).unwrap();
    assert!(!r.codec_ran && out.iter().all(|&x| x == 0.0));
}

/// Full Mimi shapes on ZeroWeights: slow on the scalar path (W20-15: ~1.3 s/frame). Release only.
#[test]
#[ignore]
fn full_size_zero_weights_two_frames() {
    run(MimiLaneConfig::full(), 2);
}
