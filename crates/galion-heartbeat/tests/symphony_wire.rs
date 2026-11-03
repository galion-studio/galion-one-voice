// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! Harness for the 80 ms symphony wire.
//! `Heartbeat::step` then `SymphonyWire::step`: no alloc, no block, late = pause.
mod common;
use common::*;
use galion_emotion::{DefaultConsentGate, Purpose};
use galion_heartbeat::symphony::{
    BandView, DotMark, Sector, SymphonyWire, BANDS, PERSONA_HOLD_E, PERSONA_HOLD_FRAMES,
    PERSONA_RELEASE_E, STEP_BUDGET_US,
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
    let paused = wire.step(&frame, view(true, 0.9, true, &slot), &mut me);
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
    let one = wire.step(&frame, view(false, 0.0, false, &slot), &mut me);
    assert!(!one.paused);
    assert_eq!(
        one.clone_samples, FRAME_SAMPLES as u16,
        "one frame, not the queued tail"
    );
    assert!(me.iter().all(|s| (*s - 0.25).abs() < 1e-6));

    let rest = &slot[one.clone_samples as usize..];
    let mut me2 = [0.0f32; FRAME_SAMPLES];
    let frame = hb.step(&fin(&pcm, false));
    let two = wire.step(&frame, view(false, 0.0, false, rest), &mut me2);
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
    let out = wire.step(&frame, view(false, 0.0, false, &slot), &mut tiny);
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
        let out = wire.step(&frame, view(false, 0.9, true, &[]), &mut me);
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
    let held = wire.step(&frame, view(false, 0.1, true, &[]), &mut me);
    assert!(held.paused);
    assert_eq!(
        held.persona_hold, PERSONA_HOLD_FRAMES,
        "a late frame does not reset or advance the hold"
    );
    assert!(held.persona_may_flip);

    let frame = hb.step(&fin(&pcm, false));
    let released = wire.step(&frame, view(false, 0.2, true, &[]), &mut me);
    assert_eq!(released.persona_hold, 0);
    assert!(!released.persona_may_flip);

    let frame = hb.step(&fin(&pcm, false));
    let _ = wire.step(&frame, view(false, 0.9, true, &[]), &mut me);
    let frame = hb.step(&fin(&pcm, false));
    let dropped = wire.step(&frame, view(false, 0.9, false, &[]), &mut me);
    assert_eq!(dropped.persona_hold, 0, "no SENSE, no hold");
    assert_eq!(
        dropped.feeling_applied, 0.9,
        "the previous sensed sample still applies once"
    );
    let frame = hb.step(&fin(&pcm, false));
    let next = wire.step(&frame, view(false, 0.9, false, &[]), &mut me);
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
    let idle = wire.step(&frame, view(false, 0.0, false, &[]), &mut me);
    assert_eq!(idle.dot, DotMark::Idle);

    let frame = hb.step(&fin(&pcm, false));
    let listen = wire.step(&frame, view(true, 0.0, false, &[]), &mut me);
    assert_eq!(listen.dot, DotMark::Listening);

    let frame = hb.step(&fin(&pcm, false));
    let speak = wire.step(&frame, view(true, 0.0, false, &slot), &mut me);
    assert_eq!(speak.dot, DotMark::Speaking, "me band wins over you");
    assert_eq!(speak.clone_samples, FRAME_SAMPLES as u16);

    let frame = hb.step(&fin(&pcm, true));
    let held = wire.step(&frame, view(true, 0.0, false, &slot), &mut me);
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
        let sector = wire.step(&frame, view(true, 0.0, false, slot_now), &mut me);
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

#[test]
fn harness_budget_and_verdict() {
    let pcm = [0.1f32; FRAME_SAMPLES];
    let slot = [0.2f32; FRAME_SAMPLES];
    let mut me = [0.0f32; FRAME_SAMPLES];
    let mut hb = hb(true);
    let mut wire = SymphonyWire::new();
    let n = 4096usize;
    let mut times = Vec::with_capacity(n);
    for i in 0..n {
        let t0 = Instant::now();
        let frame = hb.step(&fin(&pcm, i % 64 == 63));
        let emit = wire.me_emit(&frame, me.len());
        let slot_now: &[f32] = if emit { &slot } else { &[] };
        let sector = wire.step(&frame, view(i % 3 == 0, 0.4, true, slot_now), &mut me);
        let us = t0.elapsed().as_secs_f64() * 1e6;
        std::hint::black_box(sector);
        times.push(us);
    }
    let warm = &mut times[64..];
    warm.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p99 = warm[warm.len() * 99 / 100];
    let max = *warm.last().unwrap();
    assert!(
        p99 < f64::from(STEP_BUDGET_US),
        "p99 {p99:.2} us >= {STEP_BUDGET_US}"
    );
    assert!(
        max < f64::from(STEP_BUDGET_US),
        "max {max:.2} us >= {STEP_BUDGET_US}"
    );
    assert_eq!(FRAME_MS, 80);
    println!(
        "verdict=PASS frame_ms={FRAME_MS} bands=3 sectors=3 allocs=0 late=pause p99_us={p99:.3} max_us={max:.3} budget_us={STEP_BUDGET_US} n={}",
        warm.len()
    );
}
