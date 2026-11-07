// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! Harness for the 80 ms symphony wire.
//! `Heartbeat::step` then `SymphonyWire::step`: no alloc, no block, late = pause.
mod common;
use common::*;
use galion_emotion::{DefaultConsentGate, Purpose};
use galion_heartbeat::symphony::{
    BandView, DotMark, NanoPcmRing, Sector, SymphonyWire, BANDS, NANO_RING_CAP, NANO_RING_FRAMES,
    PERSONA_HOLD_E, PERSONA_HOLD_FRAMES, PERSONA_RELEASE_E, STEP_BUDGET_US,
};
use galion_heartbeat::{
    FrameIn, Heartbeat, HeartbeatConfig, FRAME_MS, FRAME_SAMPLES, RIKA_BITS, SAMPLE_RATE,
};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

struct Counting;
static ALLOCS: AtomicUsize = AtomicUsize::new(0);
thread_local! { static ARMED: Cell<bool> = const { Cell::new(false) }; }

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if ARMED.with(|a| a.get()) {
            ALLOCS.fetch_add(1, Ordering::SeqCst);
        }
        System.alloc(l)
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        System.dealloc(p, l)
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        if ARMED.with(|a| a.get()) {
            ALLOCS.fetch_add(1, Ordering::SeqCst);
        }
        System.realloc(p, l, n)
    }
}
#[global_allocator]
static A: Counting = Counting;

fn heartbeat(rika: bool) -> Heartbeat<DefaultConsentGate> {
    hb(rika)
}

fn hb(rika: bool) -> Heartbeat<DefaultConsentGate> {
    let mut g = DefaultConsentGate::default();
    g.grant(0, "hr", Purpose::PersonalAssistant);
    let cfg = if rika {
        HeartbeatConfig::rika_v0()
    } else {
        HeartbeatConfig::default()
    };
    Heartbeat::new(cfg, g)
}

fn fin<'a>(pcm: &'a [f32], late: bool) -> FrameIn<'a> {
    FrameIn {
        you_pcm: pcm,
        affect: None,
        tension: None,
        consent_bits: RIKA_BITS,
        purpose: Purpose::PersonalAssistant,
        session_open: true,
        me_speaking: false,
        late,
    }
}

fn view<'a>(you_active: bool, feeling_e: f32, sense: bool, me_slot: &'a [f32]) -> BandView<'a> {
    BandView {
        you_active,
        feeling_e,
        sense,
        me_slot,
    }
}

fn source_clean(label: &str, src: &str) {
    let code: String = src
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for bad in [
        "Mutex",
        "RwLock",
        "Condvar",
        "sleep",
        "std::fs",
        "std::net",
        "Instant",
        "SystemTime",
        "mpsc",
        "Vec<",
        "vec!",
        "String",
        "Box::new",
        "format!",
        "println!",
        "thread::",
    ] {
        assert!(!code.contains(bad), "{label} contains `{bad}`");
    }
}

#[test]
fn frame_is_80ms_and_three_bands_and_three_sectors() {
    assert_eq!(FRAME_MS, 80);
    assert_eq!(SAMPLE_RATE, 24_000);
    assert_eq!(FRAME_SAMPLES, 1_920);
    assert_eq!(BANDS, ["you", "me", "feeling"]);
    assert_eq!(BANDS.len(), 3);
    assert_eq!(
        Sector::ALL.map(Sector::name),
        ["clone", "persona", "one-dot"]
    );
    assert_eq!(PERSONA_HOLD_FRAMES, 8);
    assert!((PERSONA_HOLD_E - 0.65).abs() < f32::EPSILON);
    assert!((PERSONA_RELEASE_E - 0.50).abs() < f32::EPSILON);
    assert_eq!(STEP_BUDGET_US, 800);
}

#[test]
fn hot_path_sources_have_no_blocking_or_allocating_calls() {
    source_clean("lib.rs", include_str!("../src/lib.rs"));
    source_clean("symphony.rs", include_str!("../src/symphony.rs"));
}

#[test]
fn late_frame_pauses_and_the_next_frame_does_not_catch_up() {
    let pcm = [0.0f32; FRAME_SAMPLES];
    let slot = vec![0.25f32; FRAME_SAMPLES * 2 + 10];
    let mut hb = hb(false);
    let mut wire = SymphonyWire::new();
    let mut me = [7.0f32; FRAME_SAMPLES];

    let frame = hb.step(&fin(&pcm, true));
    assert!(frame.paused);
    assert!(!wire.me_emit(&frame, me.len()));
    let paused = wire.step(&frame, view(true, 0.9, true, &slot), &mut me, None);
    assert!(paused.paused);
    assert_eq!(paused.frame_ms, 80);
    assert_eq!(paused.clone_samples, 0, "pause consumes nothing");
    assert_eq!(me[0], 7.0, "pause does not write the me band");
    assert_eq!(
        paused.persona_hold, 0,
        "pause does not count a feeling hold"
    );
    assert!(!paused.persona_may_flip);

    let frame = hb.step(&fin(&pcm, false));
    assert!(wire.me_emit(&frame, me.len()));
    let one = wire.step(&frame, view(false, 0.0, false, &slot), &mut me, None);
    assert!(!one.paused);
    assert_eq!(
        one.clone_samples, FRAME_SAMPLES as u16,
        "one frame, not the queued tail"
    );
    assert!(me.iter().all(|s| (*s - 0.25).abs() < 1e-6));

    let rest = &slot[one.clone_samples as usize..];
    let mut me2 = [0.0f32; FRAME_SAMPLES];
    let frame = hb.step(&fin(&pcm, false));
    let two = wire.step(&frame, view(false, 0.0, false, rest), &mut me2, None);
    assert_eq!(two.clone_samples, FRAME_SAMPLES as u16);
    assert_eq!(
        rest.len() - two.clone_samples as usize,
        10,
        "the tail waits for a later frame"
    );
}

#[test]
fn short_me_buffer_is_a_pause() {
    let pcm = [0.0f32; FRAME_SAMPLES];
    let slot = [0.5f32; FRAME_SAMPLES];
    let mut hb = hb(false);
    let mut wire = SymphonyWire::new();
    let mut tiny = [3.0f32; 4];
    let frame = hb.step(&fin(&pcm, false));
    assert!(!frame.paused);
    let out = wire.step(&frame, view(false, 0.0, false, &slot), &mut tiny, None);
    assert!(out.paused);
    assert_eq!(out.clone_samples, 0);
    assert_eq!(tiny[0], 3.0);
}

#[test]
fn persona_needs_eight_frames_and_feeling_lands_next_frame() {
    let pcm = [0.0f32; FRAME_SAMPLES];
    let mut hb = hb(false);
    let mut wire = SymphonyWire::new();
    let mut me = [0.0f32; FRAME_SAMPLES];
    let mut prev_applied = 0.0f32;
    for i in 0..PERSONA_HOLD_FRAMES {
        let frame = hb.step(&fin(&pcm, false));
        let out = wire.step(&frame, view(false, 0.9, true, &[]), &mut me, None);
        assert!(!out.paused);
        assert_eq!(out.persona_hold, i + 1);
        assert_eq!(out.persona_may_flip, i + 1 >= PERSONA_HOLD_FRAMES);
        if i == 0 {
            assert_eq!(out.feeling_applied, 0.0, "decision waits one frame");
        } else {
            assert!((out.feeling_applied - 0.9).abs() < 1e-6);
        }
        prev_applied = out.feeling_applied;
    }
    assert!((prev_applied - 0.9).abs() < 1e-6);

    let frame = hb.step(&fin(&pcm, true));
    let held = wire.step(&frame, view(false, 0.1, true, &[]), &mut me, None);
    assert!(held.paused);
    assert_eq!(
        held.persona_hold, PERSONA_HOLD_FRAMES,
        "a late frame does not reset or advance the hold"
    );
    assert!(held.persona_may_flip);

    let frame = hb.step(&fin(&pcm, false));
    let released = wire.step(&frame, view(false, 0.2, true, &[]), &mut me, None);
    assert_eq!(released.persona_hold, 0);
    assert!(!released.persona_may_flip);

    let frame = hb.step(&fin(&pcm, false));
    let _ = wire.step(&frame, view(false, 0.9, true, &[]), &mut me, None);
    let frame = hb.step(&fin(&pcm, false));
    let dropped = wire.step(&frame, view(false, 0.9, false, &[]), &mut me, None);
    assert_eq!(dropped.persona_hold, 0, "no SENSE, no hold");
    assert_eq!(
        dropped.feeling_applied, 0.9,
        "the previous sensed sample still applies once"
    );
    let frame = hb.step(&fin(&pcm, false));
    let next = wire.step(&frame, view(false, 0.9, false, &[]), &mut me, None);
    assert_eq!(next.feeling_applied, 0.0);
}

#[test]
fn one_dot_speak_wins_and_a_pause_holds_the_mark() {
    let pcm = [0.0f32; FRAME_SAMPLES];
    let slot = [0.4f32; FRAME_SAMPLES];
    let mut hb = hb(false);
    let mut wire = SymphonyWire::new();
    let mut me = [0.0f32; FRAME_SAMPLES];

    let frame = hb.step(&fin(&pcm, false));
    let idle = wire.step(&frame, view(false, 0.0, false, &[]), &mut me, None);
    assert_eq!(idle.dot, DotMark::Idle);

    let frame = hb.step(&fin(&pcm, false));
    let listen = wire.step(&frame, view(true, 0.0, false, &[]), &mut me, None);
    assert_eq!(listen.dot, DotMark::Listening);

    let frame = hb.step(&fin(&pcm, false));
    let speak = wire.step(&frame, view(true, 0.0, false, &slot), &mut me, None);
    assert_eq!(speak.dot, DotMark::Speaking, "me band wins over you");
    assert_eq!(speak.clone_samples, FRAME_SAMPLES as u16);

    let frame = hb.step(&fin(&pcm, true));
    let held = wire.step(&frame, view(true, 0.0, false, &slot), &mut me, None);
    assert!(held.paused);
    assert_eq!(
        held.dot,
        DotMark::Speaking,
        "late frame does not flash the dot"
    );
    assert_eq!(held.clone_samples, 0);
}

#[test]
fn yield_releases_the_slot_and_copies_nothing() {
    let frames = render(&r1());
    let mut hb = hb(true);
    let mut wire = SymphonyWire::new();
    let slot = vec![0.2f32; FRAME_SAMPLES];
    let mut me = [0.0f32; FRAME_SAMPLES];
    let mut saw = false;
    for x in &frames {
        let frame = hb.step(&fin(x, false));
        let emit = wire.me_emit(&frame, me.len());
        let slot_now: &[f32] = if emit { &slot } else { &[] };
        let sector = wire.step(&frame, view(true, 0.0, false, slot_now), &mut me, None);
        if frame.yield_now() {
            saw = true;
            assert!(!emit);
            assert!(sector.release_slot);
            assert_eq!(sector.clone_samples, 0);
            assert_eq!(sector.frame_ms, 80);
        }
    }
    assert!(saw, "R1 yield frame was applied");
}

#[test]
fn steps_allocate_nothing() {
    let ks = script(&[
        (K::Room, 13),
        (K::Breath, 5),
        (K::Voice, 5),
        (K::Room, 120),
        (K::Breath, 4),
        (K::Room, 60),
    ]);
    let frames = render(&ks);
    let slot = vec![0.05f32; FRAME_SAMPLES];
    let mut me = [0.0f32; FRAME_SAMPLES];
    let mut hb = hb(true);
    let mut wire = SymphonyWire::new();
    let mut ring = NanoPcmRing::new();
    assert_eq!(ring.push(&slot), FRAME_SAMPLES);
    ARMED.with(|a| a.set(true));
    let probe = std::hint::black_box(Box::new(7u64));
    ARMED.with(|a| a.set(false));
    drop(probe);
    assert!(
        ALLOCS.swap(0, Ordering::SeqCst) > 0,
        "counting allocator is live"
    );

    ARMED.with(|a| a.set(true));
    let mut copied = 0u32;
    for (n, x) in frames.iter().enumerate() {
        let frame = hb.step(&FrameIn {
            you_pcm: x,
            affect: None,
            tension: None,
            consent_bits: if n == 100 { 0 } else { RIKA_BITS },
            purpose: Purpose::PersonalAssistant,
            session_open: true,
            me_speaking: n % 7 == 0,
            late: n % 11 == 0,
        });
        let emit = wire.me_emit(&frame, me.len());
        let slot_now: &[f32] = if emit { &slot } else { &[] };
        let sector = wire.step(
            &frame,
            view(n % 5 == 0, 0.8, n % 17 != 0, slot_now),
            &mut me,
            Some(&mut ring),
        );
        copied += u32::from(sector.clone_samples > 0);
        std::hint::black_box(sector);
    }
    ARMED.with(|a| a.set(false));
    assert_eq!(
        ALLOCS.load(Ordering::SeqCst),
        0,
        "Heartbeat::step or SymphonyWire::step allocated"
    );
    assert!(copied > 0, "the me slot was actually drained");
    assert!(hb.paused_frames() > 0, "late frames paused");
}

/// Odd, so the median is one of the passes and not an average of two.
const BUDGET_TRIALS: usize = 5;

struct BudgetPass {
    p99_us: f64,
    max_us: f64,
    n_warm: usize,
    nano_frames: u32,
    dropped: u32,
}

fn median_us(samples: &mut [f64]) -> f64 {
    assert!(!samples.is_empty());
    assert!(
        samples.len() % 2 == 1,
        "median pass index needs an odd count"
    );
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    samples[samples.len() / 2]
}

/// One warm pass: 64 frames dropped, then the rest ranked. Same timed region
/// as the original single pass (ring fill stays outside `Instant`).
fn budget_pass(pcm: &[f32], slot: &[f32]) -> BudgetPass {
    let mut me = [0.0f32; FRAME_SAMPLES];
    let mut hb = hb(true);
    let mut wire = SymphonyWire::new();
    let mut ring = NanoPcmRing::new();
    let n = 4096usize;
    let mut times = Vec::with_capacity(n);
    let mut nano_frames = 0u32;
    for i in 0..n {
        if ring.queued() < FRAME_SAMPLES {
            ring.push(slot);
        }
        let t0 = Instant::now();
        let frame = hb.step(&fin(pcm, i % 64 == 63));
        let emit = wire.me_emit(&frame, me.len());
        let slot_now: &[f32] = if emit { slot } else { &[] };
        let sector = wire.step(
            &frame,
            view(i % 3 == 0, 0.4, true, slot_now),
            &mut me,
            Some(&mut ring),
        );
        nano_frames += u32::from(sector.nano_frame);
        let us = t0.elapsed().as_secs_f64() * 1e6;
        std::hint::black_box(sector);
        times.push(us);
    }
    let warm = &mut times[64..];
    warm.sort_by(|a, b| a.partial_cmp(b).unwrap());
    BudgetPass {
        p99_us: warm[warm.len() * 99 / 100],
        max_us: *warm.last().unwrap(),
        n_warm: warm.len(),
        nano_frames,
        dropped: ring.dropped(),
    }
}

#[test]
fn budget_median_ignores_one_spike_and_still_fails_a_slow_step() {
    // The CI flake was one pass at 849 µs, and a loaded box has recorded a
    // single 4 ms preemption. Those are outliers. A step that is actually
    // at or over the ceiling is slow on every pass, so the median fails.
    let mut p99 = [100.0, 110.0, 849.0, 105.0, 108.0];
    let mut max = [120.0, 130.0, 4_000.0, 140.0, 125.0];
    assert!((median_us(&mut p99) - 108.0).abs() < 1e-9);
    assert!((median_us(&mut max) - 130.0).abs() < 1e-9);
    assert!(median_us(&mut max) < f64::from(STEP_BUDGET_US));
    let mut slow = [849.0, 860.0, 900.0, 910.0, 920.0];
    assert!(median_us(&mut slow) >= f64::from(STEP_BUDGET_US));
}

#[test]
fn harness_budget_and_verdict() {
    // Median of independent passes, still compared with STEP_BUDGET_US (800).
    // A CI multiplier is the wrong fix: the recorded 4 ms spike is 5× the
    // ceiling, and a multiplier that absorbs it would also pass a real
    // regression. A 1.1× multiplier would let a true 849 µs step through,
    // which is the regression this ceiling exists to catch.
    // An opt-in bench would drop the check from `cargo test`.
    assert_eq!(BUDGET_TRIALS % 2, 1);
    let pcm = [0.1f32; FRAME_SAMPLES];
    let slot = [0.2f32; FRAME_SAMPLES];
    let mut passes = Vec::with_capacity(BUDGET_TRIALS);
    for _ in 0..BUDGET_TRIALS {
        let pass = budget_pass(&pcm, &slot);
        assert_eq!(FRAME_MS, 80);
        assert!(pass.nano_frames > 0, "the timed step drained the Nano ring");
        assert_eq!(pass.dropped, 0, "the harness must not overflow the ring");
        assert_eq!(pass.n_warm, 4032);
        passes.push(pass);
    }
    let mut p99s: Vec<f64> = passes.iter().map(|p| p.p99_us).collect();
    let mut maxes: Vec<f64> = passes.iter().map(|p| p.max_us).collect();
    let worst_p99 = p99s.iter().copied().fold(0.0_f64, f64::max);
    let worst_max = maxes.iter().copied().fold(0.0_f64, f64::max);
    let p99 = median_us(&mut p99s);
    let max = median_us(&mut maxes);
    let budget = f64::from(STEP_BUDGET_US);
    assert!(
        p99 < budget,
        "median p99 {p99:.2} us >= {STEP_BUDGET_US} (worst {worst_p99:.2})"
    );
    assert!(
        max < budget,
        "median max {max:.2} us >= {STEP_BUDGET_US} (worst {worst_max:.2})"
    );
    let trial_max: Vec<String> = passes.iter().map(|p| format!("{:.3}", p.max_us)).collect();
    println!(
        "verdict=PASS frame_ms={FRAME_MS} bands=3 sectors=3 allocs=0 late=pause p99_us={p99:.3} max_us={max:.3} budget_us={STEP_BUDGET_US} n={} trials={BUDGET_TRIALS} gate=median worst_p99_us={worst_p99:.3} worst_max_us={worst_max:.3} trial_max_us={}",
        passes[0].n_warm,
        trial_max.join(",")
    );
}

#[test]
fn nano_ring_drains_one_frame_and_a_late_tick_does_not_catch_up() {
    assert_eq!(FRAME_SAMPLES, 1_920);
    assert_eq!(SAMPLE_RATE, 24_000);
    assert_eq!(NANO_RING_FRAMES, 4);
    assert_eq!(NANO_RING_CAP, FRAME_SAMPLES * 4);

    let mut ring = NanoPcmRing::new();
    let mut carry = None;
    let tone = 1_000i16;
    let mut bytes = Vec::with_capacity((FRAME_SAMPLES * 2 + 10) * 2);
    for i in 0..(FRAME_SAMPLES * 2 + 10) {
        let sample = if i < FRAME_SAMPLES { tone } else { 2_000 };
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    let n1 = ring.push_s16le(&bytes[..3], &mut carry);
    assert_eq!(n1, 1, "one full s16 and one leftover byte");
    assert_eq!(carry, Some(bytes[2]));
    let n2 = ring.push_s16le(&bytes[3..], &mut carry);
    assert!(carry.is_none());
    assert_eq!(n1 + n2, FRAME_SAMPLES * 2 + 10);

    let pcm = [0.0f32; FRAME_SAMPLES];
    let mut hb = hb(false);
    let mut wire = SymphonyWire::new();
    let mut me = [7.0f32; FRAME_SAMPLES];
    let queued = ring.queued();

    let frame = hb.step(&fin(&pcm, true));
    assert!(frame.paused);
    let paused = wire.step(
        &frame,
        view(true, 0.9, true, &[0.8; FRAME_SAMPLES]),
        &mut me,
        Some(&mut ring),
    );
    assert!(paused.paused);
    assert_eq!(paused.clone_samples, 0);
    assert!(!paused.nano_frame);
    assert_eq!(ring.queued(), queued, "a late frame does not pop the ring");
    assert_eq!(me[0], 7.0);

    let frame = hb.step(&fin(&pcm, false));
    let one = wire.step(
        &frame,
        view(false, 0.0, false, &[0.8; FRAME_SAMPLES]),
        &mut me,
        Some(&mut ring),
    );
    assert!(one.nano_frame);
    assert_eq!(one.clone_samples, FRAME_SAMPLES as u16);
    assert_eq!(one.dot, DotMark::Speaking);
    assert!(me
        .iter()
        .all(|s| (*s - (tone as f32 / 32768.0)).abs() < 1e-6));
    assert_eq!(
        ring.queued(),
        FRAME_SAMPLES + 10,
        "the second frame stays queued"
    );

    let frame = hb.step(&fin(&pcm, false));
    let two = wire.step(
        &frame,
        view(false, 0.0, false, &[]),
        &mut me,
        Some(&mut ring),
    );
    assert!(two.nano_frame);
    assert_eq!(two.clone_samples, FRAME_SAMPLES as u16);
    assert!(me.iter().all(|s| (*s - (2_000.0 / 32768.0)).abs() < 1e-6));
    assert_eq!(ring.queued(), 10, "no catch-up: the tail waits");

    let fallback = [0.8f32; FRAME_SAMPLES];
    let frame = hb.step(&fin(&pcm, false));
    let third = wire.step(
        &frame,
        view(false, 0.0, false, &fallback),
        &mut me,
        Some(&mut ring),
    );
    assert!(!third.nano_frame, "ten samples are not a frame");
    assert_eq!(third.clone_samples, FRAME_SAMPLES as u16);
    assert!(
        me.iter().all(|s| (*s - 0.8).abs() < 1e-6),
        "me slot is the fallback"
    );
    assert_eq!(ring.queued(), 10);

    let mut wrapped = NanoPcmRing::new();
    let mut wire = SymphonyWire::new();
    for i in 1..=6 {
        let sample = [i as f32 * 0.01; FRAME_SAMPLES];
        assert_eq!(wrapped.push(&sample), FRAME_SAMPLES);
        let frame = hb.step(&fin(&pcm, false));
        let out = wire.step(
            &frame,
            view(false, 0.0, false, &[]),
            &mut me,
            Some(&mut wrapped),
        );
        assert!(out.nano_frame);
        assert!(me.iter().all(|s| (*s - i as f32 * 0.01).abs() < 1e-6));
        assert_eq!(wrapped.queued(), 0);
    }

    let mut full = NanoPcmRing::new();
    let block = [0.1f32; FRAME_SAMPLES];
    for _ in 0..NANO_RING_FRAMES {
        assert_eq!(full.push(&block), FRAME_SAMPLES);
    }
    assert_eq!(full.push(&block), 0);
    assert_eq!(full.dropped(), FRAME_SAMPLES as u32);
    assert_eq!(full.queued(), NANO_RING_CAP);
    let frame = hb.step(&fin(&pcm, false));
    let kept = wire.step(
        &frame,
        view(false, 0.0, false, &[]),
        &mut me,
        Some(&mut full),
    );
    assert_eq!(kept.clone_samples, FRAME_SAMPLES as u16);
    assert_eq!(full.queued(), NANO_RING_CAP - FRAME_SAMPLES);

    let frames = render(&r1());
    let mut clock = heartbeat(true);
    let mut wire = SymphonyWire::new();
    let mut held = NanoPcmRing::new();
    let burst = [0.3f32; FRAME_SAMPLES];
    let mut saw = false;
    for x in &frames {
        let frame = clock.step(&fin(x, false));
        if frame.yield_now() {
            assert_eq!(held.push(&burst), FRAME_SAMPLES);
            let before = held.queued();
            let sector = wire.step(
                &frame,
                view(true, 0.0, false, &burst),
                &mut me,
                Some(&mut held),
            );
            assert!(sector.release_slot);
            assert_eq!(sector.clone_samples, 0);
            assert!(!sector.nano_frame);
            assert_eq!(held.queued(), before, "step does not drop the ring");
            held.release();
            assert_eq!(held.queued(), 0);
            saw = true;
        }
    }
    assert!(saw, "R1 yield frame was applied");
}
