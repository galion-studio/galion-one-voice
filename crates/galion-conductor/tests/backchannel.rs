// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Listening nods from generated prosody cues. No audio file.
//!
//! A long hold with three mild clause falls nods twice: the middle fall is
//! inside the 4.8 s gap. A question rise, a filler, a yield, a denied ear,
//! and the off switch nod zero times. None of those frames is a barge-in.

use galion_conductor::{
    listener_on_bus, Affect, BackchannelConfig, BackchannelKind, Conductor, Ear,
    FakeBrain, Floor, ListenCue, ListenMood, TickEvent, TurnMsg, BACKCHANNEL_GAP_MS,
    BACKCHANNEL_PEAK_MAX, FRAME_MS, SPEECH_PEAK_MIN,
};
use horbm_ear::{FrameOut, ListenCue as EarCue, ProsodyCue, Scorer, TurnMsg as EarMsg};

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

/// Three clause falls. The gaps are 2.0 s and then 5.2 s more, so the
/// 4.8 s limit keeps the middle nod and allows the third.
fn story_steps() -> Vec<Step> {
    let mut steps = Vec::new();
    push_fall(&mut steps, "I walked down to the market");
    push_flat(&mut steps, 20, "and the stall was still open");
    push_fall(&mut steps, "and the stall was still open");
    push_flat(&mut steps, 60, "so I bought the bread");
    push_fall(&mut steps, "so I bought the bread");
    steps
}

struct Nod {
    t_ms: u64,
    frames_after_fall: u64,
    kind: BackchannelKind,
    energy: f32,
}

struct Report {
    nods: Vec<Nod>,
    falls: Vec<u64>,
    barge_in: u32,
    brain_calls: u32,
    epoch: u64,
}

fn run(steps: &[Step], enabled: bool, mood: ListenMood) -> Report {
    let mut scorer = Scorer::bundled();
    let mut conductor = Conductor::new(FakeBrain::new());
    if !enabled {
        conductor.set_backchannel_config(BackchannelConfig::from_env(Some("0")));
    }
    let epoch = conductor.epoch();
    let mut nods = Vec::new();
    let mut falls = Vec::new();
    let mut barge_in = 0u32;
    let mut last_voiced = None;
    let mut brain_calls = 0u32;
    for step in steps {
        let msg = scorer.push_cue(step.cue, step.partial);
        let cue = scorer.listen();
        let ear = if step.cue.voiced {
            Ear::Speech(step.partial)
        } else {
            Ear::Quiet
        };
        let tick = conductor.tick_listen(ear, None, map_msg(msg), map_cue(cue), mood);
        if tick.event == TickEvent::BargeIn || tick.cancel {
            barge_in = barge_in.saturating_add(1);
        }
        assert_eq!(tick.epoch, epoch, "a nod must not bump the epoch");
        brain_calls = tick.brain_calls;
        if step.cue.voiced {
            last_voiced = Some(tick.t_ms);
            assert!(tick.backchannel.is_none(), "mid-word at {}", tick.t_ms);
        }
        if cue == EarCue::ClauseFall {
            falls.push(tick.t_ms);
            assert!(matches!(msg, EarMsg::Hold), "a clause fall is still a hold");
        }
        if let Some(nod) = tick.backchannel {
            let frames =
                tick.t_ms.saturating_sub(last_voiced.expect("voiced")) / u64::from(FRAME_MS);
            assert_eq!(frames, u64::from(nod.frames_after_fall));
            assert_eq!(tick.event, TickEvent::ListenBackchannel);
            assert_eq!(tick.floor, Floor::Listening);
            assert!(tick.clause.is_none());
            assert!(!tick.me_silent);
            assert!(nod.energy < 1.0);
            assert!(nod.energy <= 0.55);
            nods.push(Nod {
                t_ms: tick.t_ms,
                frames_after_fall: frames,
                kind: nod.kind,
                energy: nod.energy,
            });
        }
    }
    Report {
        nods,
        falls,
        barge_in,
        brain_calls,
        epoch,
    }
}

#[test]
fn a_long_story_nods_on_two_of_three_clause_falls() {
    let report = run(&story_steps(), true, ListenMood::resting());
    assert_eq!(report.falls.len(), 3, "three clause falls");
    assert_eq!(report.nods.len(), 2, "the middle fall is inside the gap");
    assert!(report.nods.iter().all(|nod| nod.frames_after_fall == 1));
    assert!(report
        .nods
        .iter()
        .all(|nod| nod.kind == BackchannelKind::MmHm));
    assert_eq!(report.brain_calls, 0, "a nod is not a reply");
    assert_eq!(report.barge_in, 0);
    assert_eq!(report.epoch, 1);
    let skipped = report.falls[1] - report.nods[0].t_ms;
    let kept = report.nods[1].t_ms - report.nods[0].t_ms;
    assert!(skipped < u64::from(BACKCHANNEL_GAP_MS));
    assert!(kept >= u64::from(BACKCHANNEL_GAP_MS));
    assert_eq!(report.falls[0], report.nods[0].t_ms);
    assert_eq!(report.falls[2], report.nods[1].t_ms);
    println!(
        "backchannel story: falls=3 nods=2 frames_after=[{}, {}] skipped_gap_ms={skipped} kept_gap_ms={kept} limit_ms={BACKCHANNEL_GAP_MS} false_barge_in=0 result=pass",
        report.nods[0].frames_after_fall, report.nods[1].frames_after_fall
    );
}

#[test]
fn a_question_rise_does_not_nod() {
    let question = "is the kettle ready";
    let mut early = Vec::new();
    for (f0, energy) in [(140.0, 0.22), (165.0, 0.22), (195.0, 0.22), (230.0, 0.22)] {
        early.push(Step {
            cue: heard(f0, energy),
            partial: question,
        });
    }
    early.push(Step {
        cue: quiet(40),
        partial: question,
    });
    let held = run(&early, true, ListenMood::resting());
    assert!(held.nods.is_empty());
    assert!(held.falls.is_empty());
    assert_eq!(held.brain_calls, 0);
    assert_eq!(held.barge_in, 0);

    let mut yielded = early;
    yielded.push(Step {
        cue: quiet(80),
        partial: question,
    });
    let mut scorer = Scorer::bundled();
    let mut conductor = Conductor::new(FakeBrain::new());
    let mut nods = 0;
    let mut barge = 0;
    for step in &yielded {
        let msg = scorer.push_cue(step.cue, step.partial);
        let cue = scorer.listen();
        let ear = if step.cue.voiced {
            Ear::Speech(step.partial)
        } else {
            Ear::Quiet
        };
        let tick =
            conductor.tick_listen(ear, None, map_msg(msg), map_cue(cue), ListenMood::resting());
        if tick.backchannel.is_some() {
            nods += 1;
        }
        if tick.event == TickEvent::BargeIn || tick.cancel {
            barge += 1;
        }
    }
    assert_eq!(nods, 0);
    assert_eq!(barge, 0);
    assert_eq!(conductor.floor(), Floor::Speaking);
    println!("backchannel question: nods=0 false_barge_in=0 result=pass");
}

#[test]
fn a_filler_does_not_nod() {
    let partial = "I was going to um...";
    let mut steps = Vec::new();
    for (f0, energy) in [(180.0, 0.20), (160.0, 0.12), (140.0, 0.06), (120.0, 0.04)] {
        steps.push(Step {
            cue: heard(f0, energy),
            partial,
        });
    }
    for pause in [80, 160, 400, 640] {
        steps.push(Step {
            cue: quiet(pause),
            partial,
        });
    }
    let report = run(&steps, true, ListenMood::resting());
    assert!(report.nods.is_empty());
    assert!(report.falls.is_empty());
    assert_eq!(report.brain_calls, 0);
    assert_eq!(report.barge_in, 0);
    println!("backchannel filler: nods=0 false_barge_in=0 result=pass");
}

#[test]
fn the_rate_limit_holds() {
    let mut steps = Vec::new();
    push_fall(&mut steps, "I walked down to the market");
    push_flat(&mut steps, 10, "and the stall was still open");
    push_fall(&mut steps, "and the stall was still open");
    let report = run(&steps, true, ListenMood::resting());
    assert_eq!(report.falls.len(), 2);
    assert_eq!(report.nods.len(), 1);
    let gap = report.falls[1] - report.falls[0];
    assert!(gap < u64::from(BACKCHANNEL_GAP_MS));
    assert_eq!(report.barge_in, 0);
    println!(
        "backchannel rate: falls=2 nods=1 gap_ms={gap} limit_ms={BACKCHANNEL_GAP_MS} false_barge_in=0 result=pass"
    );
}

#[test]
fn a_yield_takes_the_turn_without_a_nod() {
    let partial = "the kettle is ready";
    let mut scorer = Scorer::bundled();
    let mut conductor = Conductor::new(FakeBrain::new());
    let frames = [
        heard(220.0, 0.30),
        heard(190.0, 0.22),
        heard(160.0, 0.14),
        heard(130.0, 0.08),
        quiet(80),
    ];
    let mut nods = 0;
    let mut last = None;
    for cue in frames {
        let msg = scorer.push_cue(cue, partial);
        let listen = scorer.listen();
        let ear = if cue.voiced {
            Ear::Speech(partial)
        } else {
            Ear::Quiet
        };
        last = Some(conductor.tick_listen(
            ear,
            None,
            map_msg(msg),
            map_cue(listen),
            ListenMood::resting(),
        ));
        if last.as_ref().unwrap().backchannel.is_some() {
            nods += 1;
        }
    }
    let last = last.expect("frame");
    assert_eq!(nods, 0);
    assert!(last.backchannel.is_none());
    assert_eq!(last.event, TickEvent::Committed);
    assert_ne!(last.event, TickEvent::BargeIn);
    assert!(!last.cancel);
    assert!(last.brain_calls >= 1);
    assert_eq!(last.floor, Floor::Speaking);

    let mut forced = Conductor::new(FakeBrain::new());
    let _ = forced.tick_listen(
        Ear::Speech(partial),
        None,
        TurnMsg::Hold,
        ListenCue::None,
        ListenMood::resting(),
    );
    let yield_frame = forced.tick_listen(
        Ear::Quiet,
        None,
        TurnMsg::Yield { question: false },
        ListenCue::ClauseFall,
        ListenMood::resting(),
    );
    assert!(yield_frame.backchannel.is_none());
    assert_ne!(yield_frame.event, TickEvent::ListenBackchannel);
    assert_ne!(yield_frame.event, TickEvent::BargeIn);
    assert!(!yield_frame.cancel);
    println!("backchannel yield: nods=0 false_barge_in=0 result=pass");
}

#[test]
fn the_off_switch_and_a_denied_ear_stay_quiet() {
    let off = run(&story_steps(), false, ListenMood::resting());
    assert!(off.nods.is_empty());
    assert_eq!(off.falls.len(), 3, "the ear still hears the falls");
    assert_eq!(off.barge_in, 0);
    assert!(!BackchannelConfig::from_env(Some("off")).enabled);
    assert!(BackchannelConfig::from_env(Some("on")).enabled);

    let mut denied = Scorer::bundled();
    denied.push_cue(heard(200.0, 0.2), "market");
    assert_eq!(denied.push(FrameOut::Denied, "market"), EarMsg::PauseOnly);
    assert_eq!(denied.listen(), EarCue::None);

    let mut conductor = Conductor::new(FakeBrain::new());
    let _ = conductor.tick_listen(
        Ear::Speech("I walked down to the market"),
        None,
        TurnMsg::Hold,
        ListenCue::None,
        ListenMood::resting(),
    );
    let quiet = conductor.tick_listen(
        Ear::Quiet,
        None,
        TurnMsg::PauseOnly,
        ListenCue::ClauseFall,
        ListenMood::resting(),
    );
    assert!(quiet.backchannel.is_none());
    assert_ne!(quiet.event, TickEvent::BargeIn);
    assert!(!quiet.cancel);
    println!(
        "backchannel off: env=0 nods=0 denied_ear=pause_only nods_on_pause_only=0 reason=no_pitch_contour false_barge_in=0 result=pass"
    );
}

#[test]
fn parked_config_silences_nods_for_any_id() {
    for persona in ["northwind", "parked-fixture", "configured"] {
        let mut conductor = Conductor::new(FakeBrain::new());
        conductor.set_persona(persona);
        conductor.set_backchannel_config(BackchannelConfig {
            enabled: true,
            parked: true,
        });
        let report = {
            let mut scorer = Scorer::bundled();
            let steps = {
                let mut steps = Vec::new();
                push_fall(&mut steps, "I walked down to the market");
                steps
            };
            let mut nods = 0;
            for step in &steps {
                let msg = scorer.push_cue(step.cue, step.partial);
                let cue = scorer.listen();
                let ear = if step.cue.voiced {
                    Ear::Speech(step.partial)
                } else {
                    Ear::Quiet
                };
                let tick = conductor.tick_listen(
                    ear,
                    None,
                    map_msg(msg),
                    map_cue(cue),
                    ListenMood::resting(),
                );
                if tick.backchannel.is_some() {
                    nods += 1;
                }
                assert_ne!(tick.event, TickEvent::BargeIn);
            }
            nods
        };
        assert_eq!(report, 0, "{persona}");
    }
}

#[test]
fn installed_persona_nods_without_a_name_check() {
    for persona in ["northwind", "parked-fixture", "configured"] {
        let mut conductor = Conductor::new(FakeBrain::new());
        conductor.set_persona(persona);
        let mut scorer = Scorer::bundled();
        let mut steps = Vec::new();
        push_fall(&mut steps, "I walked down to the market");
        let mut nods = 0;
        for step in &steps {
            let msg = scorer.push_cue(step.cue, step.partial);
            let cue = scorer.listen();
            let ear = if step.cue.voiced {
                Ear::Speech(step.partial)
            } else {
                Ear::Quiet
            };
            let tick = conductor.tick_listen(ear, None, map_msg(msg), map_cue(cue), ListenMood::resting());
            if tick.backchannel.is_some() {
                nods += 1;
            }
            assert_ne!(tick.event, TickEvent::BargeIn);
        }
        assert_eq!(nods, 1, "{persona}");
    }
}

#[test]
fn the_choice_follows_the_moment_and_the_listener() {
    let warm = ListenMood {
        moment: Affect {
            valence: 0.70,
            arousal: 0.55,
        },
        listener: None,
    };
    let laughed = run(
        &{
            let mut steps = Vec::new();
            push_fall(&mut steps, "I walked down to the market");
            steps
        },
        true,
        warm,
    );
    assert_eq!(laughed.nods.len(), 1);
    assert_eq!(laughed.nods[0].kind, BackchannelKind::BreathLaugh);
    assert!(laughed.nods[0].energy <= 0.55);
    assert!(f32::from(BACKCHANNEL_PEAK_MAX) < f32::from(SPEECH_PEAK_MIN));

    let mut wire = [0u8; 8];
    wire[2] = 0b1;
    wire[3] = (-0.8_f32 * 127.0).round() as i8 as u8;
    wire[4] = (0.2_f32 * 255.0).round() as u8;
    let sad = listener_on_bus(&wire).expect("listener");
    let tugged = ListenMood {
        moment: warm.moment,
        listener: Some(sad),
    };
    let quiet = run(
        &{
            let mut steps = Vec::new();
            push_fall(&mut steps, "I walked down to the market");
            steps
        },
        true,
        tugged,
    );
    assert_eq!(quiet.nods.len(), 1);
    assert_eq!(quiet.nods[0].kind, BackchannelKind::MmHm);
    assert!(quiet.nods[0].energy <= 0.55);
    assert!(laughed.nods[0].energy <= 0.55);
    println!(
        "backchannel mood: rest=mm-hm warm=breath-laugh listener_sad=mm-hm energy_warm={:.3} energy_sad={:.3} result=pass",
        laughed.nods[0].energy, quiet.nods[0].energy
    );
}
