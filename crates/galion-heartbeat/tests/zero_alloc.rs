// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! Counting allocator: `Heartbeat::step` must allocate nothing (Rika ON, full R1 moment).
mod common;
use common::*;
use galion_emotion::{DefaultConsentGate, EmotionFrame, Purpose, VoicePersona};
use galion_heartbeat::{FrameIn, Heartbeat, HeartbeatConfig, RIKA_BITS};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};

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

#[test]
fn step_allocates_nothing_with_rika_on() {
    let ks = script(&[(K::Room, 13), (K::Breath, 5), (K::Voice, 5), (K::Room, 120), (K::Breath, 4), (K::Room, 60)]);
    let frames = render(&ks);
    let mut g = DefaultConsentGate::default();
    g.grant(0, "hr", Purpose::PersonalAssistant);
    let mut hb = Heartbeat::new(HeartbeatConfig::rika_v0(), g);
    // Self-check: the counter does see allocations.
    ARMED.with(|a| a.set(true));
    let probe = std::hint::black_box(Box::new(7u64));
    ARMED.with(|a| a.set(false));
    drop(probe);
    assert!(ALLOCS.swap(0, Ordering::SeqCst) > 0, "counting allocator is live");
    ARMED.with(|a| a.set(true));
    let mut armed = 0u32;
    for (n, x) in frames.iter().enumerate() {
        let o = hb.step(&FrameIn {
            you_pcm: x,
            affect: None,
            tension: None,
            consent_bits: if n == 100 { 0 } else { RIKA_BITS },
            purpose: Purpose::PersonalAssistant,
            session_open: true,
            me_speaking: n % 7 == 0,
            late: n % 11 == 0,
        });
        armed += o.lean_in_audible() as u32;
    }
    ARMED.with(|a| a.set(false));
    assert_eq!(ALLOCS.load(Ordering::SeqCst), 0, "Heartbeat::step allocated");
    assert!(armed > 0, "the moment actually happened");
}

#[test]
fn feeling_step_allocates_nothing() {
    let mut g = DefaultConsentGate::default();
    g.grant(0, "hr", Purpose::PersonalAssistant);
    let mut hb = Heartbeat::new(HeartbeatConfig::default(), g);
    hb.set_persona(VoicePersona::Jarvis);
    let pcm = vec![0.0f32; galion_heartbeat::FRAME_SAMPLES];
    let affect = EmotionFrame { seq: 1, t_ms: 0, dur_ms: 80, valence: -0.6, arousal: 0.2, dominance: 0.4, confidence: 1.0 };
    ALLOCS.store(0, Ordering::SeqCst);
    ARMED.with(|a| a.set(true));
    for i in 0..32 {
        let _ = hb.step(&FrameIn {
            you_pcm: &pcm,
            affect: Some(affect),
            tension: None,
            consent_bits: if i == 8 { 0 } else { galion_emotion::band::consent_bits::SENSE },
            purpose: Purpose::PersonalAssistant,
            session_open: true,
            me_speaking: false,
            late: i == 4,
        });
    }
    ARMED.with(|a| a.set(false));
    assert_eq!(ALLOCS.load(Ordering::SeqCst), 0, "feeling band inside step allocated");
}
