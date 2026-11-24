// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Prosody end-of-turn joined to the conductor. Synthetic sines, no audio file.
//!
//! Pause-only still releases the clause on the second quiet frame. A yield
//! releases it on the first. Barge-in stays the 200 ms overlap lock.

use galion_conductor::{
    Conductor, Ear as FloorEar, FakeBrain, Floor, TickEvent, TurnMsg as FloorMsg, BARGE_IN_MAX_MS,
    FRAME_MS,
};
use horbm_ear::{Ear as ProsodyEar, FrameOut, Scorer, TurnMsg, FRAME_SAMPLES, SAMPLE_RATE};

const PI: f32 = std::f32::consts::PI;

fn sine(hz: f32, amp: f32) -> [f32; FRAME_SAMPLES] {
    let mut buf = [0.0f32; FRAME_SAMPLES];
    if hz <= 0.0 || amp <= 0.0 {
        return buf;
    }
    let step = 2.0 * PI * hz / SAMPLE_RATE as f32;
    let mut phase = 0.0f32;
    for sample in &mut buf {
        *sample = amp * phase.sin();
        phase += step;
    }
    buf
}

fn map_msg(msg: TurnMsg) -> FloorMsg {
    match msg {
        TurnMsg::PauseOnly => FloorMsg::PauseOnly,
        TurnMsg::Hold => FloorMsg::Hold,
        TurnMsg::Yield { question } => FloorMsg::Yield { question },
    }
}

struct Taken {
    clause_after_silence_ms: u64,
    eager_to_clause_ms: u64,
    question: bool,
    brain_calls: u32,
}

fn tick_delta(earlier: u64, later: u64) -> u64 {
    later.saturating_sub(earlier) / 1_000
}

fn run(
    partial: &str,
    frames: &[(f32, f32)],
    use_scorer: bool,
) -> (Taken, galion_conductor::TickOut) {
    let mut prosody = ProsodyEar::new();
    let mut scorer = Scorer::bundled();
    let mut conductor = Conductor::new(FakeBrain::new());
    let mut committed = None;
    let mut saw_voiced = false;
    for (index, (hz, amp)) in frames.iter().copied().enumerate() {
        let pcm = sine(hz, amp);
        let heard = match prosody.push(&pcm) {
            FrameOut::Heard(frame) => frame,
            other => panic!("frame {index} was {other:?}"),
        };
        if heard.voiced {
            saw_voiced = true;
        }
        let msg = if use_scorer {
            map_msg(scorer.push(FrameOut::Heard(heard), partial))
        } else {
            FloorMsg::PauseOnly
        };
        let ear = if heard.voiced {
            FloorEar::Speech(partial)
        } else {
            FloorEar::Quiet
        };
        let tick = conductor.tick_turn(ear, None, msg);
        if heard.voiced {
            assert!(tick.clause.is_none(), "cut in on voiced frame {index}");
            assert_ne!(tick.event, TickEvent::Committed);
            assert_ne!(tick.event, TickEvent::EagerEndOfTurn);
        }
        if tick.clause.is_some() && committed.is_none() {
            committed = Some(tick);
        }
    }
    assert!(saw_voiced);
    let committed = committed.expect("clause");
    let silence = committed.stamps.t_vad_silence_start.expect("silence");
    let eou = committed.stamps.t_eou_decided.expect("eou");
    let clause = committed.stamps.t_first_clause_flushed.expect("clause");
    (
        Taken {
            clause_after_silence_ms: tick_delta(silence, clause),
            eager_to_clause_ms: tick_delta(eou, clause),
            question: committed.end_question,
            brain_calls: committed.brain_calls,
        },
        committed,
    )
}

#[test]
fn a_falling_yield_releases_the_clause_one_frame_before_pause_only() {
    let partial = "the kettle is ready";
    let frames = [
        (240.0, 0.30),
        (200.0, 0.24),
        (160.0, 0.18),
        (130.0, 0.12),
        (0.0, 0.0),
        (0.0, 0.0),
    ];
    let (prosody, committed) = run(partial, &frames, true);
    let (pause, _) = run(partial, &frames, false);
    assert_eq!(committed.event, TickEvent::Committed);
    assert_eq!(
        committed.clause.as_ref().map(|c| c.clause.as_str()),
        Some("The kettle is on the hob.")
    );
    assert!(!prosody.question);
    assert_eq!(prosody.brain_calls, 1);
    assert_eq!(prosody.eager_to_clause_ms, 0);
    assert_eq!(prosody.clause_after_silence_ms, u64::from(FRAME_MS));
    assert_eq!(pause.eager_to_clause_ms, u64::from(FRAME_MS));
    assert_eq!(pause.clause_after_silence_ms, u64::from(FRAME_MS) * 2);
    assert!(pause.clause_after_silence_ms > prosody.clause_after_silence_ms);
    let sooner = pause.clause_after_silence_ms - prosody.clause_after_silence_ms;
    println!(
        "eot conductor: eager_to_clause_prosody_ms={} eager_to_clause_pause_only_ms={} clause_after_silence_prosody_ms={} clause_after_silence_pause_only_ms={} sooner_ms={sooner} result=pass",
        prosody.eager_to_clause_ms,
        pause.eager_to_clause_ms,
        prosody.clause_after_silence_ms,
        pause.clause_after_silence_ms
    );
}

#[test]
fn a_rising_question_yields_and_a_filler_does_not() {
    let question = [
        (140.0, 0.22),
        (165.0, 0.22),
        (195.0, 0.22),
        (230.0, 0.22),
        (0.0, 0.0),
    ];
    let (taken, committed) = run("is the kettle ready", &question, true);
    assert!(taken.question);
    assert_eq!(committed.event, TickEvent::Committed);
    assert_eq!(taken.eager_to_clause_ms, 0);
    assert_eq!(
        committed.clause.as_ref().map(|c| c.clause.as_str()),
        Some("The kettle is on the hob.")
    );

    let filler = [
        (150.0, 0.18),
        (156.0, 0.18),
        (148.0, 0.18),
        (154.0, 0.18),
        (0.0, 0.0),
        (0.0, 0.0),
        (0.0, 0.0),
        (0.0, 0.0),
        (0.0, 0.0),
    ];
    let mut prosody = ProsodyEar::new();
    let mut scorer = Scorer::bundled();
    let mut conductor = Conductor::new(FakeBrain::new());
    let partial = "I was going to um...";
    for (hz, amp) in filler {
        let pcm = sine(hz, amp);
        let heard = match prosody.push(&pcm) {
            FrameOut::Heard(frame) => frame,
            other => panic!("{other:?}"),
        };
        let msg = map_msg(scorer.push(FrameOut::Heard(heard), partial));
        assert_ne!(msg, FloorMsg::Yield { question: false });
        assert_ne!(msg, FloorMsg::Yield { question: true });
        let ear = if heard.voiced {
            FloorEar::Speech(partial)
        } else {
            FloorEar::Quiet
        };
        let tick = conductor.tick_turn(ear, None, msg);
        assert_eq!(tick.brain_calls, 0);
        assert!(tick.clause.is_none());
        assert_eq!(tick.event, TickEvent::None);
    }
}

#[test]
fn sensing_off_falls_back_to_the_pause_and_barge_in_is_unchanged() {
    let mut denied = ProsodyEar::from_env(Some("0"));
    let mut closed = ProsodyEar::new();
    closed.deny();
    let mut scorer = Scorer::bundled();
    let tone = sine(150.0, 0.2);
    for ear in [&mut denied, &mut closed] {
        let frame = ear.push(&tone);
        assert!(matches!(frame, FrameOut::Denied));
        assert_eq!(
            scorer.push(frame, "I was going to um..."),
            TurnMsg::PauseOnly
        );
    }

    let mut conductor = Conductor::new(FakeBrain::new());
    let partial = "I was going to um...";
    let _ = conductor.tick_turn(FloorEar::Speech(partial), None, FloorMsg::PauseOnly);
    let _ = conductor.tick_turn(FloorEar::Speech(partial), None, FloorMsg::PauseOnly);
    let eager = conductor.tick_turn(FloorEar::Quiet, None, FloorMsg::PauseOnly);
    assert_eq!(eager.event, TickEvent::EagerEndOfTurn);
    assert!(eager.clause.is_none(), "pause-only still gates the clause");
    let commit = conductor.tick_turn(FloorEar::Quiet, None, FloorMsg::PauseOnly);
    assert_eq!(commit.event, TickEvent::Committed);
    assert!(commit.clause.is_some());

    let mut held = Conductor::new(FakeBrain::new());
    let _ = held.tick_turn(FloorEar::Speech(partial), None, FloorMsg::Hold);
    let _ = held.tick_turn(FloorEar::Speech(partial), None, FloorMsg::Hold);
    for _ in 0..5 {
        let tick = held.tick_turn(FloorEar::Quiet, None, FloorMsg::Hold);
        assert_eq!(tick.event, TickEvent::None);
        assert_eq!(tick.brain_calls, 0);
        assert_eq!(tick.floor, Floor::Listening);
    }

    let frames = [
        (240.0, 0.30),
        (200.0, 0.24),
        (160.0, 0.18),
        (130.0, 0.12),
        (0.0, 0.0),
    ];
    let (_, committed) = run("where is the kettle", &frames, true);
    assert_eq!(committed.floor, Floor::Speaking);
    let mut conductor = Conductor::new(FakeBrain::new());
    // Replay up to the commit, then overlap. The message on the overlap
    // frames must not move the 160 ms yield.
    let mut prosody = ProsodyEar::new();
    let mut scorer = Scorer::bundled();
    let partial = "where is the kettle";
    for (hz, amp) in frames {
        let pcm = sine(hz, amp);
        let heard = match prosody.push(&pcm) {
            FrameOut::Heard(frame) => frame,
            other => panic!("{other:?}"),
        };
        let msg = map_msg(scorer.push(FrameOut::Heard(heard), partial));
        let ear = if heard.voiced {
            FloorEar::Speech(partial)
        } else {
            FloorEar::Quiet
        };
        let tick = conductor.tick_turn(ear, None, msg);
        if tick.clause.is_some() {
            break;
        }
    }
    assert_eq!(conductor.floor(), Floor::Speaking);
    let onset = conductor.t_ms();
    let first = conductor.tick_turn(
        FloorEar::Speech("stop talking now"),
        None,
        FloorMsg::Yield { question: false },
    );
    assert!(!first.cancel);
    let mut silence_at = None;
    let mut last = first;
    for _ in 0..4 {
        if last.me_silent {
            silence_at = Some(last.t_ms);
            break;
        }
        last = conductor.tick_turn(FloorEar::Speech("stop talking now"), None, FloorMsg::Hold);
    }
    let silence_at = silence_at.expect("barge");
    let barge_ms = silence_at - onset;
    assert_eq!(barge_ms, 160);
    assert!(barge_ms <= u64::from(BARGE_IN_MAX_MS));
    assert_eq!(last.event, TickEvent::BargeIn);
    assert!(last.cancel);
    assert!(last.me_silent);
    println!(
        "eot conductor: barge_in_ms={barge_ms} limit_ms={BARGE_IN_MAX_MS} sensing_off=pause_only result=pass"
    );
}
