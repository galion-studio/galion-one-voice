// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Full duplex smoke. The counting allocator is the same shape as
//! `galion-emotion-core` `tests/zero_alloc.rs`.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};

use horbm_e2e::{run, EmotionClaim, Outcome, Sense, EMOTION_FRAME_MS};

struct Counting;

static ALLOCS: AtomicUsize = AtomicUsize::new(0);
thread_local! {
    static ARMED: Cell<bool> = const { Cell::new(false) };
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if ARMED.with(|armed| armed.get()) {
            ALLOCS.fetch_add(1, Ordering::SeqCst);
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if ARMED.with(|armed| armed.get()) {
            ALLOCS.fetch_add(1, Ordering::SeqCst);
        }
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

struct Arm;

impl Arm {
    fn new() -> Self {
        ALLOCS.store(0, Ordering::SeqCst);
        ARMED.with(|armed| armed.set(true));
        Self
    }
}

impl Drop for Arm {
    fn drop(&mut self) {
        ARMED.with(|armed| armed.set(false));
    }
}

fn emotion_allocs() -> usize {
    ARMED.with(|armed| armed.set(true));
    let probe = vec![1u8];
    ARMED.with(|armed| armed.set(false));
    assert!(
        ALLOCS.load(Ordering::SeqCst) > 0,
        "the counter must see a Vec"
    );
    drop(probe);

    let mut claim = EmotionClaim::warmed().expect("emotion warmup");
    let _arm = Arm::new();
    claim.steps(999).expect("emotion claim steps");
    drop(_arm);
    assert_eq!(claim.next_index(), 1000, "warmup plus 999 frames");
    assert_eq!(EMOTION_FRAME_MS, 80);
    ALLOCS.load(Ordering::SeqCst)
}

fn field<'a>(text: &'a str, key: &str) -> &'a str {
    text.lines()
        .find_map(|line| line.strip_prefix(&format!("{key}=")))
        .unwrap_or_else(|| panic!("missing {key} in\n{text}"))
}

fn assert_stub_has_no_time(text: &str, key: &str) {
    let value = field(text, key);
    if value == "stub" || value == "off" {
        let timed = format!("{key}_ns=");
        let timed_ms = format!("{key}_ms=");
        assert!(
            !text
                .lines()
                .any(|line| line.starts_with(&timed) || line.starts_with(&timed_ms)),
            "{key} is {value} and also has a time\n{text}"
        );
    }
}

fn assert_common(outcome: &Outcome, text: &str, sense: Sense) {
    assert!(outcome.wav_removed, "the input wav was left on disk");
    assert_eq!(field(text, "persona"), "samantha");
    assert_eq!(
        field(text, "prosody_sense"),
        if sense == Sense::On { "on" } else { "off" }
    );
    assert_eq!(field(text, "vad"), "energy");
    assert!(outcome.voiced_frames > 0);
    assert!(field(text, "vad_voiced_frames").parse::<u32>().unwrap() > 0);
    assert!(field(text, "vad_quiet_frames").parse::<u32>().unwrap() > 0);
    assert_eq!(field(text, "emotion"), "real");
    assert_eq!(field(text, "emotion_allocs"), "0");
    assert_eq!(field(text, "brain"), "real");
    assert_eq!(field(text, "breath"), "real");
    let breath_p50: f64 = field(text, "breath_p50_us").parse().expect("breath p50");
    let breath_p99: f64 = field(text, "breath_p99_us").parse().expect("breath p99");
    assert!(
        breath_p50 > 0.0 && breath_p50 <= breath_p99,
        "breath p50 {breath_p50} p99 {breath_p99}"
    );
    assert!(breath_p99 < 80_000.0, "breath p99 {breath_p99} us");
    let breath_ns: u64 = field(text, "breath_ns").parse().expect("breath ns");
    assert!(breath_ns > 0);
    let group_ms: u32 = field(text, "breath_group_ms")
        .parse()
        .expect("breath group");
    assert!(
        (480..=4_000).contains(&group_ms),
        "breath group {group_ms} ms"
    );
    let _pauses: u32 = field(text, "breath_pauses").parse().expect("breath pauses");
    for key in [
        "dialogue",
        "person_memory",
        "listener_feeling",
        "mood_into_voice",
        "end_of_turn",
        "piper_duplex",
        "prosody",
    ] {
        assert_stub_has_no_time(text, key);
    }
    if sense == Sense::Off {
        assert_eq!(field(text, "prosody"), "off");
        assert!(!text.lines().any(|line| line.starts_with("prosody_ns=")));
    }
    if outcome.asr_real {
        assert_eq!(field(text, "asr"), "vosk");
        if field(text, "wer") != "stub" {
            let wer: f64 = field(text, "wer").parse().expect("wer");
            assert!(wer.is_finite());
        }
    } else {
        assert_eq!(field(text, "asr"), "stub");
        assert_eq!(field(text, "wer"), "stub");
        assert!(
            outcome.spoke_kettle,
            "the scripted kettle question had no kettle reply"
        );
        let barge = outcome.conductor_barge_ms.expect("conductor barge");
        assert!(barge > 0 && barge <= 200, "conductor barge {barge} ms");
        assert_eq!(field(text, "conductor_barge_ms"), barge.to_string());
        assert_eq!(field(text, "conductor_barge_clock"), "virtual");
    }
    if outcome.mouth_real {
        assert_ne!(field(text, "mouth"), "stub");
        assert_eq!(field(text, "output_buffer"), "loopback");
        let p50 = outcome.first_audio_p50_us.expect("p50");
        let p95 = outcome.first_audio_p95_us.expect("p95");
        assert!(p50 > 0 && p50 <= p95, "first audio p50 {p50} p95 {p95}");
        assert_eq!(
            field(text, "first_audio_p50_ms"),
            format!("{:.3}", p50 as f64 / 1000.0)
        );
        assert_eq!(
            field(text, "first_audio_p95_ms"),
            format!("{:.3}", p95 as f64 / 1000.0)
        );
        let stop = outcome.barge_stop_ns.expect("barge stop");
        assert_eq!(field(text, "barge_in_stop_ns"), stop.to_string());
        let rtf = outcome.rtf.expect("rtf");
        assert!(rtf.is_finite() && rtf > 0.0, "rtf {rtf}");
        assert_eq!(field(text, "rtf_def"), "wall/audio");
        assert!(
            field(text, "output_buffer_samples")
                .parse::<usize>()
                .unwrap()
                > 0
        );
    } else {
        assert_eq!(field(text, "mouth"), "stub");
        assert_eq!(field(text, "output_buffer"), "stub");
        assert_eq!(field(text, "first_audio_p50_ms"), "stub");
        assert_eq!(field(text, "first_audio_p95_ms"), "stub");
        assert_eq!(field(text, "barge_in_stop_ns"), "stub");
        assert_eq!(field(text, "rtf"), "stub");
        assert!(outcome.first_audio_p50_us.is_none());
        assert!(outcome.barge_stop_ns.is_none());
        assert!(outcome.rtf.is_none());
    }
}

#[test]
fn emotion_tick_allocates_nothing() {
    assert_eq!(emotion_allocs(), 0, "EmotionCore::step allocated");
}

#[test]
fn live_loop_prosody_on() {
    let allocs = emotion_allocs();
    assert_eq!(allocs, 0, "EmotionCore::step allocated");
    let outcome = run(Sense::On).expect("smoke");
    let text = outcome.report.render(allocs);
    println!("{text}");
    assert_common(&outcome, &text, Sense::On);
}

#[test]
fn live_loop_prosody_off() {
    let allocs = emotion_allocs();
    assert_eq!(allocs, 0, "EmotionCore::step allocated");
    let outcome = run(Sense::Off).expect("smoke");
    let text = outcome.report.render(allocs);
    println!("{text}");
    assert_common(&outcome, &text, Sense::Off);
}

#[test]
fn a_persona_other_than_samantha_is_refused() {
    horbm_e2e::refuse_other_persona().expect("aria refused");
}
