// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! The voice loop plays the nod. Generated cues, generated tone, no file.
//!
//! The me band carries one quiet frame. The you band keeps the turn: the
//! mouth is not speaking, the epoch does not move, and a later user word
//! is not a barge-in.

use horbm::{CardHit, CardLookup};
use horbm_ear::{ListenCue as EarCue, ProsodyCue, Scorer, TurnMsg as EarMsg};
use horbm_voice::{
    BackchannelConfig, BackchannelKind, EarEvent, FakeEar, FakeMouth, HorbmTurn, ListenCue,
    ListenMood, TickEvent, TurnMsg, VoiceLoop, BACKCHANNEL_GAP_MS, BACKCHANNEL_PEAK_MAX, FRAME_MS,
    SPEECH_PEAK_MIN,
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

fn heard(f0: f32, energy: f32) -> ProsodyCue {
    ProsodyCue {
        voiced: true,
        f0_hz: f0,
        energy,
        speaking_rate_hz: 3.0,
        pause_ms: 0,
    }
}

fn quiet(pause_ms: u32) -> ProsodyCue {
    ProsodyCue {
        voiced: false,
        f0_hz: 0.0,
        energy: 0.0,
        speaking_rate_hz: 0.0,
        pause_ms,
    }
}

fn map_msg(msg: EarMsg) -> TurnMsg {
    match msg {
        EarMsg::PauseOnly => TurnMsg::PauseOnly,
        EarMsg::Hold => TurnMsg::Hold,
        EarMsg::Yield { question } => TurnMsg::Yield { question },
    }
}

fn map_cue(cue: EarCue) -> ListenCue {
    match cue {
        EarCue::None => ListenCue::None,
        EarCue::ClauseFall => ListenCue::ClauseFall,
        EarCue::QuestionRise => ListenCue::QuestionRise,
        EarCue::Filler => ListenCue::Filler,
    }
}

struct Step {
    cue: ProsodyCue,
    partial: &'static str,
}

fn push_fall(steps: &mut Vec<Step>, partial: &'static str) {
    for (f0, energy) in [(200.0, 0.20), (193.0, 0.16), (186.0, 0.12), (180.0, 0.08)] {
        steps.push(Step {
            cue: heard(f0, energy),
            partial,
        });
    }
    steps.push(Step {
        cue: quiet(80),
        partial,
    });
}

fn push_flat(steps: &mut Vec<Step>, frames: usize, partial: &'static str) {
    for _ in 0..frames {
        steps.push(Step {
            cue: heard(200.0, 0.20),
            partial,
        });
    }
}

fn story_steps() -> Vec<Step> {
    let mut steps = Vec::new();
    push_fall(&mut steps, "I walked down to the market");
    push_flat(&mut steps, 20, "and the stall was still open");
    push_fall(&mut steps, "and the stall was still open");
    push_flat(&mut steps, 60, "so I bought the bread");
    push_fall(&mut steps, "so I bought the bread");
    steps
}

struct Plan {
    turn: TurnMsg,
    cue: ListenCue,
    voiced: bool,
}

fn plans_for(steps: &[Step]) -> (Vec<EarEvent>, Vec<Plan>) {
    let mut scorer = Scorer::bundled();
    let mut events = Vec::new();
    let mut plans = Vec::new();
    for step in steps {
        let turn = map_msg(scorer.push_cue(step.cue, step.partial));
        let cue = map_cue(scorer.listen());
        events.push(if step.cue.voiced {
            EarEvent::partial(step.partial)
        } else {
            EarEvent::quiet()
        });
        plans.push(Plan {
            turn,
            cue,
            voiced: step.cue.voiced,
        });
    }
    (events, plans)
}

struct Nod {
    t_ms: u64,
    frames_after_fall: u64,
    kind: BackchannelKind,
    peak: i16,
}

fn play(steps: &[Step], enabled: bool, mood: ListenMood) -> (Vec<Nod>, u32) {
    let (events, plans) = plans_for(steps);
    let mut voice = VoiceLoop::new(
        FakeEar::script(events),
        FakeMouth::new(),
        HorbmTurn::with_cards(Kettle),
    );
    if !enabled {
        voice.set_backchannel_config(BackchannelConfig::from_env(Some("off")));
    }
    let epoch = voice.epoch();
    let mut nods = Vec::new();
    let mut barge = 0u32;
    let mut last_voiced = None;
    for plan in plans {
        let tick = voice.tick_listen(&[], plan.turn, plan.cue, mood);
        if tick.event == TickEvent::BargeIn || tick.cancel {
            barge = barge.saturating_add(1);
        }
        assert_eq!(tick.epoch, epoch);
        assert!(!tick.speaking, "a nod must not take the mouth");
        assert!(tick.reply.is_none());
        if plan.voiced {
            last_voiced = Some(tick.t_ms);
            assert!(tick.audio.is_none(), "mid-word");
            assert!(tick.backchannel.is_none());
        }
        match tick.backchannel {
            None => assert!(tick.audio.is_none() || tick.event != TickEvent::ListenBackchannel),
            Some(nod) => {
                assert_eq!(tick.event, TickEvent::ListenBackchannel);
                let frame = tick.audio.expect("nod audio");
                let peak = frame
                    .samples
                    .iter()
                    .fold(0i16, |acc, sample| acc.max(sample.abs()));
                assert!(peak > 0);
                assert!(peak <= BACKCHANNEL_PEAK_MAX);
                assert!(peak < SPEECH_PEAK_MIN);
                let frames =
                    tick.t_ms.saturating_sub(last_voiced.expect("voiced")) / u64::from(FRAME_MS);
                assert_eq!(frames, 1);
                assert_eq!(nod.frames_after_fall, 1);
                assert!(nod.energy <= 0.55);
                nods.push(Nod {
                    t_ms: tick.t_ms,
                    frames_after_fall: frames,
                    kind: nod.kind,
                    peak,
                });
            }
        }
    }
    assert!(voice.mouth().spoken().is_empty(), "no reply was started");
    (nods, barge)
}

#[test]
fn a_long_story_plays_two_quiet_nods_and_never_barges() {
    let (nods, barge) = play(&story_steps(), true, ListenMood::resting());
    assert_eq!(nods.len(), 2);
    assert!(nods.iter().all(|nod| nod.frames_after_fall == 1));
    assert!(nods.iter().all(|nod| nod.kind == BackchannelKind::MmHm));
    assert_eq!(barge, 0);
    let kept = nods[1].t_ms - nods[0].t_ms;
    assert!(kept >= u64::from(BACKCHANNEL_GAP_MS));
    println!(
        "backchannel voice: nods=2 frames_after=[{}, {}] kept_gap_ms={kept} peaks=[{}, {}] speech_floor={SPEECH_PEAK_MIN} false_barge_in=0 result=pass",
        nods[0].frames_after_fall, nods[1].frames_after_fall, nods[0].peak, nods[1].peak
    );
}

#[test]
fn a_question_a_filler_and_a_yield_stay_silent() {
    let question = "is the kettle ready";
    let mut rise = Vec::new();
    for (f0, energy) in [(140.0, 0.22), (165.0, 0.22), (195.0, 0.22), (230.0, 0.22)] {
        rise.push(Step {
            cue: heard(f0, energy),
            partial: question,
        });
    }
    rise.push(Step {
        cue: quiet(40),
        partial: question,
    });
    let (nods, barge) = play(&rise, true, ListenMood::resting());
    assert!(nods.is_empty());
    assert_eq!(barge, 0);

    let partial = "I was going to um...";
    let mut filler = Vec::new();
    for (f0, energy) in [(180.0, 0.2), (160.0, 0.12), (140.0, 0.06), (120.0, 0.04)] {
        filler.push(Step {
            cue: heard(f0, energy),
            partial,
        });
    }
    filler.push(Step {
        cue: quiet(160),
        partial,
    });
    let (nods, barge) = play(&filler, true, ListenMood::resting());
    assert!(nods.is_empty());
    assert_eq!(barge, 0);

    let mut voice = VoiceLoop::new(
        FakeEar::script([EarEvent::partial("where is the kettle"), EarEvent::quiet()]),
        FakeMouth::new(),
        HorbmTurn::with_cards(Kettle),
    );
    let _ = voice.tick_listen(&[], TurnMsg::Hold, ListenCue::None, ListenMood::resting());
    let yielded = voice.tick_listen(
        &[],
        TurnMsg::Yield { question: false },
        ListenCue::ClauseFall,
        ListenMood::resting(),
    );
    assert!(yielded.backchannel.is_none());
    assert_ne!(yielded.event, TickEvent::ListenBackchannel);
    assert_ne!(yielded.event, TickEvent::BargeIn);
    assert!(!yielded.cancel);
    assert_eq!(yielded.event, TickEvent::EndOfSpeech);
    println!(
        "backchannel voice refusals: question=0 filler=0 yield=0 false_barge_in=0 result=pass"
    );
}

#[test]
fn the_off_switch_and_other_personas_play_nothing() {
    let (nods, barge) = play(&story_steps(), false, ListenMood::resting());
    assert!(nods.is_empty());
    assert_eq!(barge, 0);

    let mut steps = Vec::new();
    push_fall(&mut steps, "I walked down to the market");
    let (events, plans) = plans_for(&steps);
    let mut voice = VoiceLoop::new(
        FakeEar::script(events),
        FakeMouth::new(),
        HorbmTurn::with_cards(Kettle),
    );
    voice.set_persona("aria");
    let mut nods = 0;
    for plan in plans {
        let tick = voice.tick_listen(&[], plan.turn, plan.cue, ListenMood::resting());
        if tick.backchannel.is_some() {
            nods += 1;
        }
        assert_ne!(tick.event, TickEvent::BargeIn);
    }
    assert_eq!(nods, 0);
    println!("backchannel voice off: nods=0 aria=0 false_barge_in=0 result=pass");
}

#[test]
fn two_close_falls_play_one_nod() {
    let mut steps = Vec::new();
    push_fall(&mut steps, "I walked down to the market");
    push_flat(&mut steps, 10, "and the stall was still open");
    push_fall(&mut steps, "and the stall was still open");
    let (nods, barge) = play(&steps, true, ListenMood::resting());
    assert_eq!(nods.len(), 1);
    assert_eq!(nods[0].frames_after_fall, 1);
    assert_eq!(barge, 0);
    println!(
        "backchannel voice rate: nods=1 frames_after=1 limit_ms={BACKCHANNEL_GAP_MS} false_barge_in=0 result=pass"
    );
}
