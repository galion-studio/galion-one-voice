// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! A text turn leaves the emotion moment where it was.
//!
//! `Brain::turn` may speak a card, a miss, or nothing. The moment position
//! (`pos`, `tick`, `t_ms`) and the voice baseline stay on the bits they had
//! before the call. `Brain::step_moment` is the call that moves the position.
//! The same checks run for an in-test persona and for [`Persona::standin`].

use horbm::{
    Brain, CardHit, CardLookup, IntentMsg, MomentState, Persona, VoiceBaseline, UNKNOWN_REPLY,
};

/// Schema-valid sample. The id is replaced in [`persona_with_id`].
const VALID: &str = r#"
schema = "galion-horbm-persona/1"
version = 1
id = "sample"
voice = "sample"

[traits]
warmth = 0.61

[honesty]
never_fabricate = true
unknown = "I don't know."

[boundaries]
forbid = ["fake-sympathy", "unsourced-fact"]

[style]
brevity = 0.5
warmth = 0.62
max_sentences = 2

[baseline]
valence = 0.1
arousal = 0.31
dominance = 0.4
approach = 0.0
pitch_centre_hz = 216.0

[inner]
care = "heard"
want = "answer"
expect = "pause"
"#;

const KETTLE: &str = "the kettle";
const KETTLE_REPLY: &str = "The kettle is on the hob.";
const MISS: &str = "where is the spare fuse";

fn persona_with_id(id: &str) -> Persona {
    let text = VALID.replace("id = \"sample\"", &format!("id = \"{id}\""));
    Persona::parse(&text).unwrap_or_else(|err| panic!("parse {id}: {err}"))
}

struct KettleCard;

impl CardLookup for KettleCard {
    fn lookup(&self, query: &str) -> Option<CardHit> {
        if query.to_ascii_lowercase().contains("kettle") {
            Some(CardHit {
                id: "kettle".into(),
                gist: KETTLE_REPLY.into(),
            })
        } else {
            None
        }
    }

    fn len(&self) -> usize {
        1
    }
}

fn brain(persona: Persona) -> Brain {
    Brain::with_persona(KettleCard, None, persona)
}

/// Bit pattern of the moment fields a text turn must not move.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MomentBits {
    pos: [u32; 4],
    tick: u32,
    t_ms: u32,
}

/// Bit pattern of the baseline a text turn must not move.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BaselineBits {
    persona: [u8; 16],
    voice: [u8; 16],
    pos: [u32; 4],
    pitch_centre_hz: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Place {
    moment: MomentBits,
    baseline: BaselineBits,
    persona_id: String,
}

fn f32_bits(values: &[f32; 4]) -> [u32; 4] {
    [
        values[0].to_bits(),
        values[1].to_bits(),
        values[2].to_bits(),
        values[3].to_bits(),
    ]
}

fn moment_bits(moment: MomentState) -> MomentBits {
    MomentBits {
        pos: f32_bits(&moment.pos),
        tick: moment.tick,
        t_ms: moment.t_ms,
    }
}

fn baseline_bits(baseline: VoiceBaseline) -> BaselineBits {
    BaselineBits {
        persona: baseline.persona,
        voice: baseline.voice,
        pos: f32_bits(&baseline.pos),
        pitch_centre_hz: baseline.pitch_centre_hz.to_bits(),
    }
}

fn place(brain: &Brain) -> Place {
    Place {
        moment: moment_bits(brain.emotion().moment()),
        baseline: baseline_bits(brain.emotion().baseline()),
        persona_id: brain.persona().id().to_string(),
    }
}

fn each_persona(check: impl Fn(&mut Brain)) {
    let mut seen = Vec::new();
    for persona in [persona_with_id("northwind"), Persona::standin()] {
        let mut brain = brain(persona);
        let id = brain.persona().id().to_string();
        assert!(
            id == "northwind" || id == "standin-speaker",
            "unexpected persona id {id}"
        );
        assert!(!seen.contains(&id), "persona {id} ran twice");
        seen.push(id);
        let resting = place(&brain);
        assert_eq!(resting.moment.tick, 0);
        assert_eq!(resting.moment.t_ms, 0);
        assert_eq!(
            resting.moment.pos, resting.baseline.pos,
            "the moment starts on the baseline"
        );
        check(&mut brain);
    }
    assert_eq!(seen, ["northwind", "standin-speaker"]);
}

fn assert_turn_leaves(brain: &mut Brain, line: &str, reply: &str) {
    let before = place(brain);
    let out = brain.turn(line);
    assert_eq!(out.reply, reply);
    drop(out);
    assert_eq!(place(brain), before);
}

#[test]
fn text_turn_leaves_the_moment_and_baseline() {
    each_persona(|brain| {
        assert_turn_leaves(brain, KETTLE, KETTLE_REPLY);
        assert_eq!(brain.emotion().moment().tick, 0);
    });
}

#[test]
fn a_miss_and_an_empty_line_leave_the_moment() {
    each_persona(|brain| {
        assert_turn_leaves(brain, MISS, UNKNOWN_REPLY);
        assert_turn_leaves(brain, "", "");
        assert_eq!(brain.emotion().moment().tick, 0);
        assert_eq!(brain.emotion().moment().t_ms, 0);
    });
}

#[test]
fn fifty_turns_leave_the_moment() {
    let lines = [KETTLE, MISS, ""];
    let replies = [KETTLE_REPLY, UNKNOWN_REPLY, ""];
    each_persona(|brain| {
        let origin = place(brain);
        for index in 0..50 {
            let line = lines[index % lines.len()];
            let reply = replies[index % replies.len()];
            let before = place(brain);
            let out = brain.turn(line);
            assert_eq!(out.reply, reply, "turn {index}");
            drop(out);
            let after = place(brain);
            assert_eq!(after, before, "turn {index}");
            assert_eq!(after, origin, "turn {index}");
        }
    });
}

#[test]
fn step_moment_moves_the_moment_and_not_the_baseline() {
    each_persona(|brain| {
        let before = place(brain);
        let mut intent = IntentMsg::steady(brain.emotion().next_ms());
        intent.fatigue = 0.9;
        brain.step_moment(&intent).unwrap();
        let after = place(brain);
        assert_ne!(
            after.moment.pos, before.moment.pos,
            "a real step must move the moment position"
        );
        assert_eq!(after.moment.tick, before.moment.tick + 1);
        assert_eq!(after.moment.t_ms, intent.t_ms);
        assert_ne!(after.moment.pos, after.baseline.pos);
        assert_eq!(after.baseline, before.baseline);
        assert_eq!(after.persona_id, before.persona_id);
    });
}
