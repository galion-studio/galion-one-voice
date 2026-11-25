// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! The brain's inner home comes from the persona it was given.

use horbm::{
    Brain, CardHit, CardLookup, InnerHome, Persona, CURIOSITY_HOME, EXPECT_HOME, HELP_HOME,
};

struct Cards;

impl CardLookup for Cards {
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

fn fixture(id: &str, valence: f32) -> Persona {
    let text = format!(
        r#"
schema = "galion-horbm-persona/1"
version = 1
id = "{id}"
voice = "{id}"

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
valence = {valence}
arousal = 0.31
dominance = 0.4
approach = 0.0
pitch_centre_hz = 216.0

[inner]
care = "heard"
want = "answer"
expect = "pause"
"#
    );
    Persona::parse(&text).unwrap_or_else(|err| panic!("parse {id}: {err}"))
}

fn home_care(valence: f32) -> f32 {
    InnerHome::from_baseline(valence, CURIOSITY_HOME, HELP_HOME, EXPECT_HOME)
        .expect("baseline home")
        .care
}

#[test]
fn northwind_brain_gets_northwind_home() {
    let valence = 0.10;
    let persona = fixture("northwind", valence);
    let brain = Brain::with_persona(Cards, None, persona);
    assert_eq!(brain.persona_inner().persona_id(), "northwind");
    assert_eq!(
        brain.persona_inner().care().to_bits(),
        home_care(valence).to_bits()
    );
}

#[test]
fn two_personas_two_homes() {
    let low = Brain::with_persona(Cards, None, fixture("northwind", 0.05));
    let high = Brain::with_persona(Cards, None, fixture("cedar", 0.40));
    assert_ne!(
        low.persona_inner().care().to_bits(),
        high.persona_inner().care().to_bits()
    );
    assert!(high.persona_inner().care() > low.persona_inner().care());
    assert_eq!(low.persona_inner().persona_id(), "northwind");
    assert_eq!(high.persona_inner().persona_id(), "cedar");
}

#[test]
fn invalid_id_falls_back_to_an_empty_id_not_another_persona() {
    let valence = 0.33;
    let home = InnerHome::from_baseline(valence, CURIOSITY_HOME, HELP_HOME, EXPECT_HOME)
        .expect("baseline home");
    // The inner slot refuses this id. The brain's fallback is `with_home`:
    // the same baseline numbers, and an empty id, never a different persona.
    assert!(horbm::PersonaInner::with_home_for("Northwind", home).is_none());
    let refused = horbm::PersonaInner::with_home(home);
    assert_eq!(refused.persona_id(), "");
    assert_eq!(refused.care().to_bits(), home.care.to_bits());

    let brain = Brain::with_persona(Cards, None, fixture("cedar", valence));
    assert_eq!(brain.persona_inner().persona_id(), "cedar");
    assert_ne!(brain.persona_inner().persona_id(), "");
    assert_eq!(brain.persona_inner().care().to_bits(), home.care.to_bits());
}

#[test]
fn a_turn_still_speaks_the_card_clause() {
    let mut brain = Brain::with_persona(Cards, None, fixture("northwind", 0.10));
    let out = brain.turn("where is the kettle");
    assert_eq!(out.reply, "The kettle is on the hob.");
    assert_eq!(out.first_clause, "The kettle");
    assert_eq!(brain.persona_inner().persona_id(), "northwind");
}
