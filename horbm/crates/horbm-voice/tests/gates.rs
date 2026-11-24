// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Virtual-clock gates. No wall clock, no audio file, no Piper process.

use horbm::{CardHit, CardLookup};
use horbm_voice::{
    EarEvent, FakeEar, FakeMouth, HorbmTurn, TickEvent, VoiceLoop, BARGE_IN_MAX_MS,
    FIRST_AUDIO_BUDGET_MS, FRAME_MS,
};

struct Kettle;

impl CardLookup for Kettle {
    fn lookup(&self, query: &str) -> Option<CardHit> {
        if query.to_ascii_lowercase().contains("kettle") {
            Some(CardHit {
                id: "kettle".into(),
                gist: "The kettle is on the hob.".into(),
            })
        } else {
            None
        }
    }
}

fn voice(script: Vec<EarEvent>) -> VoiceLoop<FakeEar, FakeMouth, HorbmTurn> {
    VoiceLoop::new(
        FakeEar::script(script),
        FakeMouth::new(),
        HorbmTurn::with_cards(Kettle),
    )
}

#[test]
fn first_audio_frame_after_end_of_speech_is_within_budget() {
    let mut voice = voice(vec![
        EarEvent::end("where is the kettle"),
        EarEvent::quiet(),
    ]);
    let eot = voice.tick(&[]);
    assert_eq!(eot.event, TickEvent::EndOfSpeech);
    assert!(eot.audio.is_none());
    assert_eq!(eot.reply.as_deref(), Some("The kettle is on the hob."));
    assert_eq!(eot.t_ms, 0);

    let audio = voice.tick(&[]);
    assert_eq!(audio.event, TickEvent::FirstAudio);
    let frame = audio.audio.expect("first frame");
    assert!(frame.first);
    assert_eq!(frame.sample_rate, 16_000);
    assert!(frame.samples.iter().any(|sample| *sample != 0));
    let waited = voice.first_audio_after_eot_ms().expect("delta");
    assert_eq!(waited, u64::from(FRAME_MS));
    assert!(waited <= u64::from(FIRST_AUDIO_BUDGET_MS));
    println!(
        "voice gate: first_audio_ms={waited} budget_ms={FIRST_AUDIO_BUDGET_MS} clock=virtual frame_ms={FRAME_MS} result=pass"
    );
}

#[test]
fn barge_in_yields_inside_the_lock() {
    let mut voice = voice(vec![
        EarEvent::end("where is the kettle"),
        EarEvent::quiet(),
        EarEvent::partial("stop"),
        EarEvent::partial("stop"),
        EarEvent::partial("stop"),
    ]);
    let epoch = voice.epoch();
    let _ = voice.tick(&[]);
    let _ = voice.tick(&[]);
    assert_eq!(voice.mouth().cancels(), 0);
    let _ = voice.tick(&[]);
    let _ = voice.tick(&[]);
    let yield_frame = voice.tick(&[]);
    assert_eq!(yield_frame.event, TickEvent::BargeIn);
    assert!(yield_frame.cancel);
    assert!(yield_frame.me_silent);
    assert!(yield_frame.audio.is_none());
    assert_eq!(yield_frame.epoch, epoch + 1);
    let barge_ms = voice.barge_in_ms().expect("barge");
    assert_eq!(barge_ms, 160);
    assert!(barge_ms <= u64::from(BARGE_IN_MAX_MS));
    assert_eq!(voice.mouth().cancels(), 1);
    println!(
        "voice gate: barge_in_ms={barge_ms} limit_ms={BARGE_IN_MAX_MS} clock=virtual frame_ms={FRAME_MS} result=pass"
    );
}

#[test]
fn a_backchannel_does_not_cancel() {
    let mut script = vec![EarEvent::end("where is the kettle"), EarEvent::quiet()];
    for _ in 0..6 {
        script.push(EarEvent::partial("yeah"));
    }
    let mut voice = voice(script);
    let epoch = voice.epoch();
    let _ = voice.tick(&[]);
    let _ = voice.tick(&[]);
    for index in 0..6 {
        let frame = voice.tick(&[]);
        assert_eq!(frame.event, TickEvent::Backchannel, "frame {index}");
        assert!(!frame.cancel);
        assert_eq!(frame.epoch, epoch);
        assert!(frame.audio.is_some());
    }
    assert_eq!(voice.mouth().cancels(), 0);
    assert_eq!(voice.epoch(), epoch);
    assert!(voice.barge_in_ms().is_none());
    println!(
        "voice gate: backchannel=yeah frames=6 frame_ms={FRAME_MS} span_ms=480 cancel=false epoch_unchanged=true result=pass"
    );
}

#[test]
fn a_miss_is_the_honest_line() {
    let mut voice = voice(vec![EarEvent::end("what is the weather")]);
    let eot = voice.tick(&[]);
    assert_eq!(eot.reply.as_deref(), Some(horbm::UNKNOWN_REPLY));
    assert_eq!(voice.mouth().spoken(), &[horbm::UNKNOWN_REPLY.to_string()]);
}

#[test]
fn a_fault_does_not_speak() {
    let mut voice = voice(vec![EarEvent::Fault("recognizer down".into())]);
    let frame = voice.tick(&[]);
    assert!(frame.reply.is_none());
    assert!(frame.audio.is_none());
    assert!(voice.mouth().spoken().is_empty());
    assert_eq!(voice.mouth().cancels(), 0);
}
