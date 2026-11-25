// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! One full turn through horbm-voice, then the barge-in and backchannel gates.
//! Fake ear, fake mouth, the same brain the 7881 process opens. No audio file.

use samantha_local_server::{
    EarEvent, NativeSession, TickEvent, BARGE_IN_MAX_MS, BOOT_BRAIN, BOOT_MOUTH_DEVICE, FRAME_MS,
};

#[test]
fn one_full_turn_through_horbm_voice() {
    let mut session =
        NativeSession::scripted([EarEvent::end("where is the kettle"), EarEvent::quiet()]);
    assert_eq!(session.brain(), BOOT_BRAIN);
    assert_eq!(session.brain(), "horbm");
    assert_eq!(session.mouth_device(), BOOT_MOUTH_DEVICE);
    assert_eq!(session.mouth_device(), "cpu");
    assert_eq!(session.mouth_id(), "fake");

    let eot = session.tick();
    assert_eq!(eot.event, TickEvent::EndOfSpeech);
    assert!(eot.audio.is_none());
    assert_eq!(eot.reply.as_deref(), Some("The kettle is on the stove."));
    assert_eq!(eot.t_ms, 0);

    let audio = session.tick();
    assert_eq!(audio.event, TickEvent::FirstAudio);
    let frame = audio.audio.expect("first mouth frame");
    assert!(frame.first);
    assert!(frame.samples.iter().any(|sample| *sample != 0));
    let waited = u64::from(FRAME_MS);
    assert_eq!(audio.t_ms, waited);
    assert!(waited <= 160);
    println!(
        "voice gate: first_audio_ms={waited} budget_ms=160 clock=virtual frame_ms={FRAME_MS} result=pass"
    );
}

#[test]
fn barge_in_stays_inside_200_ms_end_to_end() {
    let mut session = NativeSession::scripted([
        EarEvent::end("where is the kettle"),
        EarEvent::quiet(),
        EarEvent::partial("stop"),
        EarEvent::partial("stop"),
        EarEvent::partial("stop"),
    ]);
    let epoch = session.epoch();
    let _ = session.tick();
    let _ = session.tick();
    assert_eq!(session.cancels(), 0);
    let _ = session.tick();
    let _ = session.tick();
    let yield_frame = session.tick();
    assert_eq!(yield_frame.event, TickEvent::BargeIn);
    assert!(yield_frame.cancel);
    assert!(yield_frame.me_silent);
    assert!(yield_frame.audio.is_none());
    assert_eq!(yield_frame.epoch, epoch + 1);
    let barge_ms = session.barge_in_ms().expect("barge");
    assert!(barge_ms <= u64::from(BARGE_IN_MAX_MS));
    assert_eq!(barge_ms, 160);
    assert_eq!(session.cancels(), 1);
    println!(
        "voice gate: barge_in_ms={barge_ms} limit_ms={BARGE_IN_MAX_MS} clock=virtual frame_ms={FRAME_MS} result=pass"
    );
}

#[test]
fn a_backchannel_does_not_cancel_end_to_end() {
    let mut script = vec![EarEvent::end("where is the kettle"), EarEvent::quiet()];
    for _ in 0..6 {
        script.push(EarEvent::partial("yeah"));
    }
    let mut session = NativeSession::scripted(script);
    let epoch = session.epoch();
    let _ = session.tick();
    let _ = session.tick();
    for index in 0..6 {
        let frame = session.tick();
        assert_eq!(frame.event, TickEvent::Backchannel, "frame {index}");
        assert!(!frame.cancel);
        assert_eq!(frame.epoch, epoch);
        assert!(frame.audio.is_some());
    }
    assert_eq!(session.cancels(), 0);
    assert_eq!(session.epoch(), epoch);
    assert!(session.barge_in_ms().is_none());
    println!(
        "voice gate: backchannel=yeah frames=6 frame_ms={FRAME_MS} span_ms=480 cancel=false epoch_unchanged=true result=pass"
    );
}
