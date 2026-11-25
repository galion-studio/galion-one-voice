// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! The prosody message on the voice loop. No audio file, no recognizer.

use horbm::{CardHit, CardLookup};
use horbm_voice::{
    EarEvent, FakeEar, FakeMouth, HorbmTurn, TickEvent, TurnMsg, VoiceLoop, BARGE_IN_MAX_MS,
    FRAME_MS,
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
fn a_yield_starts_the_reply_from_the_partial_before_a_late_final() {
    let line = "where is the kettle";
    let mut prosody = voice(vec![
        EarEvent::partial(line),
        EarEvent::partial(line),
        EarEvent::partial(line),
        EarEvent::partial(line),
        EarEvent::quiet(),
        EarEvent::quiet(),
    ]);
    for _ in 0..4 {
        let frame = prosody.tick_turn(&[], TurnMsg::Hold);
        assert!(frame.reply.is_none());
        assert!(frame.audio.is_none());
    }
    // Last word ended at the boundary of this frame (t_ms == 320).
    let decided = prosody.tick_turn(&[], TurnMsg::Yield { question: false });
    assert_eq!(decided.event, TickEvent::EndOfSpeech);
    assert!(decided.audio.is_none(), "the mouth waits one frame");
    assert_eq!(decided.reply.as_deref(), Some("The kettle is on the hob."));
    assert!(!decided.end_question);
    assert_eq!(decided.t_ms, 320);

    let audio = prosody.tick_turn(&[], TurnMsg::Hold);
    assert_eq!(audio.event, TickEvent::FirstAudio);
    assert_eq!(audio.t_ms, 400);
    let eager_to_audio = prosody.first_audio_after_eot_ms().expect("delta");
    assert_eq!(eager_to_audio, u64::from(FRAME_MS));
    let last_word_to_audio = audio.t_ms - 320;
    assert_eq!(last_word_to_audio, u64::from(FRAME_MS));

    // Scripted ASR final three frames after the same boundary. Not a live model.
    let mut late = voice(vec![
        EarEvent::partial(line),
        EarEvent::partial(line),
        EarEvent::partial(line),
        EarEvent::partial(line),
        EarEvent::quiet(),
        EarEvent::quiet(),
        EarEvent::quiet(),
        EarEvent::end(line),
        EarEvent::quiet(),
    ]);
    for _ in 0..7 {
        let frame = late.tick(&[]);
        assert!(frame.reply.is_none());
    }
    let end = late.tick(&[]);
    assert_eq!(end.event, TickEvent::EndOfSpeech);
    assert_eq!(end.t_ms, 560);
    let late_audio = late.tick(&[]);
    assert_eq!(late_audio.event, TickEvent::FirstAudio);
    assert_eq!(late_audio.t_ms, 640);
    let late_from_word = late_audio.t_ms - 320;
    assert_eq!(late_from_word, 320);
    let sooner = late_from_word - last_word_to_audio;
    assert_eq!(sooner, 240);
    println!(
        "eot voice: eager_to_audio_ms={eager_to_audio} last_word_to_audio_prosody_ms={last_word_to_audio} last_word_to_audio_asr_lag_ms={late_from_word} sooner_ms={sooner} asr_lag_ms=240 result=pass"
    );
}

#[test]
fn a_question_is_marked_and_a_filler_final_does_not_speak() {
    let mut question = voice(vec![
        EarEvent::partial("is the kettle ready"),
        EarEvent::quiet(),
        EarEvent::quiet(),
    ]);
    let _ = question.tick_turn(&[], TurnMsg::Hold);
    let yield_q = question.tick_turn(&[], TurnMsg::Yield { question: true });
    assert_eq!(yield_q.event, TickEvent::EndOfSpeech);
    assert!(yield_q.end_question);
    assert_eq!(yield_q.reply.as_deref(), Some("The kettle is on the hob."));

    let mut filler = voice(vec![
        EarEvent::partial("I was going to um"),
        EarEvent::end("I was going to um"),
        EarEvent::quiet(),
    ]);
    let mut cut_ins = 0u32;
    for _ in 0..3 {
        let frame = filler.tick_turn(&[], TurnMsg::Hold);
        if frame.reply.is_some() || frame.audio.is_some() {
            cut_ins += 1;
        }
        assert!(frame.reply.is_none());
        assert!(frame.audio.is_none());
    }
    assert_eq!(cut_ins, 0);
    assert!(filler.mouth().spoken().is_empty());
}

#[test]
fn barge_in_stays_on_the_lock_when_a_yield_is_also_arriving() {
    let mut voice = voice(vec![
        EarEvent::partial("where is the kettle"),
        EarEvent::quiet(),
        EarEvent::partial("stop"),
        EarEvent::partial("stop"),
        EarEvent::partial("stop"),
    ]);
    let epoch = voice.epoch();
    let started = voice.tick_turn(&[], TurnMsg::Yield { question: false });
    assert_eq!(started.event, TickEvent::EndOfSpeech);
    let _ = voice.tick_turn(&[], TurnMsg::Hold);
    assert_eq!(voice.mouth().cancels(), 0);
    let _ = voice.tick_turn(&[], TurnMsg::Yield { question: true });
    let _ = voice.tick_turn(&[], TurnMsg::Hold);
    let yield_frame = voice.tick_turn(&[], TurnMsg::Yield { question: false });
    assert_eq!(yield_frame.event, TickEvent::BargeIn);
    assert!(yield_frame.cancel);
    assert!(yield_frame.me_silent);
    assert!(yield_frame.audio.is_none());
    assert_eq!(yield_frame.epoch, epoch + 1);
    let barge_ms = voice.barge_in_ms().expect("barge");
    assert_eq!(barge_ms, 160);
    assert!(barge_ms <= u64::from(BARGE_IN_MAX_MS));
    assert_eq!(voice.mouth().cancels(), 1);
    println!("eot voice: barge_in_ms={barge_ms} limit_ms={BARGE_IN_MAX_MS} result=pass");
}
