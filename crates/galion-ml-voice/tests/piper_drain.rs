// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! Real Piper PCM feeds the symphony ring when `GALION_SPEECH=a21`.
//! The step still allocates nothing. A late frame is a pause.
//! `GALION_CLONE_STUB_FRAMES` is not this feed.
#![cfg(feature = "speech")]

use galion_emotion::{DefaultConsentGate, Purpose};
use galion_heartbeat::symphony::{BandView, NanoPcmRing, SymphonyWire, STEP_BUDGET_US};
use galion_heartbeat::{FrameIn, Heartbeat, HeartbeatConfig, FRAME_SAMPLES, RIKA_BITS};
use galion_ml_voice::speech::{A21RingFeed, SPEECH_RATE};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

struct Counting;
static ALLOCS: AtomicUsize = AtomicUsize::new(0);
thread_local! { static ARMED: Cell<bool> = const { Cell::new(false) }; }

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if ARMED.with(|a| a.get()) {
            ALLOCS.fetch_add(1, Ordering::SeqCst);
        }
        System.alloc(layout)
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if ARMED.with(|a| a.get()) {
            ALLOCS.fetch_add(1, Ordering::SeqCst);
        }
        System.realloc(ptr, layout, new_size)
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

#[test]
fn a21_piper_pcm_feeds_one_heartbeat_frame() {
    let prev = std::env::var("GALION_SPEECH").ok();
    std::env::set_var("GALION_SPEECH", "a21");
    let opened = A21RingFeed::open();
    match prev {
        Some(v) => std::env::set_var("GALION_SPEECH", v),
        None => std::env::remove_var("GALION_SPEECH"),
    }
    let Ok(mut feed) = opened else {
        eprintln!("piper_measured=false reason=pack-absent");
        return;
    };
    assert_eq!(SPEECH_RATE, 24_000);
    assert!(feed.native_hz() > 0);

    let t_submit = Instant::now();
    feed.submit("Hello Galion.").expect("piper submit");
    let mut ring = NanoPcmRing::new();
    let need = FRAME_SAMPLES * 2 + 10;
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut first_frame_ms = None;
    while ring.queued() < need {
        if Instant::now() >= deadline {
            panic!("piper did not fill two frames: queued {}", ring.queued());
        }
        feed.poll(&mut ring).expect("piper poll");
        if first_frame_ms.is_none() && ring.queued() >= FRAME_SAMPLES {
            first_frame_ms = Some(t_submit.elapsed().as_millis());
        }
        if ring.queued() < need {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    let first_frame_ms = first_frame_ms.expect("first frame");
    let native_hz = feed.native_hz();
    let voice = feed.voice().to_string();

    let mut gate = DefaultConsentGate::default();
    gate.grant(0, "hr", Purpose::PersonalAssistant);
    let mut hb = Heartbeat::new(HeartbeatConfig::default(), gate);
    let you = [0.0f32; FRAME_SAMPLES];
    let mut wire = SymphonyWire::new();
    let mut me = [7.0f32; FRAME_SAMPLES];
    let input = |late: bool| FrameIn {
        you_pcm: &you,
        affect: None,
        tension: None,
        consent_bits: RIKA_BITS,
        purpose: Purpose::PersonalAssistant,
        session_open: true,
        me_speaking: false,
        late,
    };
    let bands = BandView { you_active: false, feeling_e: 0.0, sense: false, me_slot: &[] };
    let queued = ring.queued();

    ARMED.with(|a| a.set(true));
    let probe = std::hint::black_box(Box::new(1u8));
    ARMED.with(|a| a.set(false));
    drop(probe);
    assert!(ALLOCS.swap(0, Ordering::SeqCst) > 0, "counting allocator is live");

    ARMED.with(|a| a.set(true));
    let t_late = Instant::now();
    let late_frame = hb.step(&input(true));
    let paused = wire.step(&late_frame, bands, &mut me, Some(&mut ring));
    let late_us = t_late.elapsed().as_secs_f64() * 1e6;
    assert!(late_frame.paused);
    assert_eq!(paused.clone_samples, 0);
    assert!(!paused.nano_frame);
    assert_eq!(me[0], 7.0, "a late frame does not write Piper PCM");
    assert_eq!(ring.queued(), queued, "a late frame does not pop the ring");
    let t_drain = Instant::now();
    let frame = hb.step(&input(false));
    let one = wire.step(&frame, bands, &mut me, Some(&mut ring));
    let drain_us = t_drain.elapsed().as_secs_f64() * 1e6;
    ARMED.with(|a| a.set(false));

    assert_eq!(ring.queued(), queued - FRAME_SAMPLES, "one frame, no catch-up");
    assert!(one.nano_frame);
    assert_eq!(one.clone_samples, FRAME_SAMPLES as u16);
    assert_eq!(ALLOCS.load(Ordering::SeqCst), 0, "step allocated");
    assert!(drain_us < f64::from(STEP_BUDGET_US) * 20.0, "drain_us {drain_us}");
    let mean = me.iter().sum::<f32>() / me.len() as f32;
    let var = me.iter().map(|s| (s - mean) * (s - mean)).sum::<f32>() / me.len() as f32;
    assert!(var > 1e-6, "Piper frame is not a stub tone");
    assert!(me.iter().all(|s| s.is_finite()));

    println!(
        "piper_measured=true engine=piper-cpu voice={voice} native_hz={native_hz} rate_hz={SPEECH_RATE} frame={FRAME_SAMPLES} first_frame_ms={first_frame_ms} drain_us={drain_us:.3} late_us={late_us:.3} allocs=0 late=pause"
    );
}
