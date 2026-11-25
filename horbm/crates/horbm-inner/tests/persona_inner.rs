// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Any persona. The fixture id is `northwind`. No audio and no user store.

use horbm_inner::{
    InnerHome, Perceived, PersonaInner, SamanthaInner, CARE_ABOVE_REST, CARE_HOME, CURIOSITY_HOME,
    EXPECT_HOME, HELP_HOME, PERSONA_ID_BYTES, TICK_MS,
};

fn northwind() -> InnerHome {
    InnerHome {
        care: 0.11,
        curiosity: 0.73,
        help: 0.19,
        expect: 0.61,
    }
}

fn seen(words: &str) -> Perceived<'_> {
    Perceived {
        t_ms: 0,
        words,
        ear: None,
        sense: false,
    }
}

#[test]
fn new_matches_the_old_numbers_bit_for_bit() {
    let fresh = PersonaInner::new();
    let alias = SamanthaInner::new();
    assert_eq!(fresh.care().to_bits(), alias.care().to_bits());
    assert_eq!(fresh.curiosity().to_bits(), alias.curiosity().to_bits());
    assert_eq!(fresh.help().to_bits(), alias.help().to_bits());
    assert_eq!(fresh.expect().to_bits(), alias.expect().to_bits());
    assert_eq!(fresh.care().to_bits(), CARE_HOME.to_bits());
    assert_eq!(fresh.curiosity().to_bits(), CURIOSITY_HOME.to_bits());
    assert_eq!(fresh.help().to_bits(), HELP_HOME.to_bits());
    assert_eq!(fresh.expect().to_bits(), EXPECT_HOME.to_bits());
    assert_eq!(fresh.home().care.to_bits(), CARE_HOME.to_bits());
    assert_eq!(fresh.persona_id(), "");
    assert_eq!(alias.persona_id(), "");
    assert_eq!(
        (CARE_ABOVE_REST + 0.25).to_bits(),
        CARE_HOME.to_bits(),
        "standalone care stays 0.30 above a rest valence of 0.25"
    );
}

#[test]
fn with_home_takes_any_persona() {
    let home = northwind();
    assert_ne!(home.care.to_bits(), CARE_HOME.to_bits());
    assert_ne!(home.curiosity.to_bits(), CURIOSITY_HOME.to_bits());
    assert_ne!(home.help.to_bits(), HELP_HOME.to_bits());
    assert_ne!(home.expect.to_bits(), EXPECT_HOME.to_bits());

    let mut life = PersonaInner::with_home_for("northwind", home).expect("id fits the slot");
    assert_eq!(life.persona_id(), "northwind");
    assert_eq!(life.care().to_bits(), home.care.to_bits());
    assert_eq!(life.curiosity().to_bits(), home.curiosity.to_bits());
    assert_eq!(life.help().to_bits(), home.help.to_bits());
    assert_eq!(life.expect().to_bits(), home.expect.to_bits());

    let raised = life.turn(seen("I feel really sad"));
    assert_eq!(raised.verb, horbm_inner::Verb::Comfort);
    let after = life.care();
    assert!(
        after < CARE_HOME,
        "sad step from 0.11 stays under the old home"
    );
    assert_ne!(after.to_bits(), home.care.to_bits());
    let _ = life.tick(TICK_MS, false);
    assert!(
        life.care() < after,
        "decay walks toward the configured home, not the standalone one"
    );

    let back = life.reset();
    assert_eq!(back.verb, horbm_inner::Verb::Hold);
    assert_eq!(life.care().to_bits(), home.care.to_bits());
    assert_eq!(life.curiosity().to_bits(), home.curiosity.to_bits());
    assert_eq!(life.help().to_bits(), home.help.to_bits());
    assert_eq!(life.expect().to_bits(), home.expect.to_bits());
    assert_eq!(life.persona_id(), "northwind");

    let wide = "abcdefghijklmnop";
    assert_eq!(wide.len(), PERSONA_ID_BYTES);
    let full = PersonaInner::with_home_for(wide, home).expect("16 bytes fit");
    assert_eq!(full.persona_id(), wide);
    assert!(PersonaInner::with_home_for("abcdefghijklmnopq", home).is_none());
    assert!(PersonaInner::with_home_for("Northwind", home).is_none());
    assert!(PersonaInner::with_home_for("nörth", home).is_none());
    let empty = PersonaInner::with_home_for("", home).expect("empty id");
    assert_eq!(empty.persona_id(), "");
    assert_eq!(empty.care().to_bits(), home.care.to_bits());
}

#[test]
fn home_from_baseline_is_pure() {
    let once = InnerHome::from_baseline(0.25, 0.42, 0.36, 0.18).expect("finite");
    let twice = InnerHome::from_baseline(0.25, 0.42, 0.36, 0.18).expect("same input");
    assert_eq!(once.care.to_bits(), twice.care.to_bits());
    assert_eq!(once.curiosity.to_bits(), twice.curiosity.to_bits());
    assert_eq!(once.help.to_bits(), twice.help.to_bits());
    assert_eq!(once.expect.to_bits(), twice.expect.to_bits());
    assert_eq!(once.care.to_bits(), CARE_HOME.to_bits());
    assert_eq!(once.curiosity.to_bits(), CURIOSITY_HOME.to_bits());
    assert_eq!(once.help.to_bits(), HELP_HOME.to_bits());
    assert_eq!(once.expect.to_bits(), EXPECT_HOME.to_bits());

    let shifted = InnerHome::from_baseline(0.10, 0.42, 0.36, 0.18).expect("other valence");
    assert_ne!(shifted.care.to_bits(), once.care.to_bits());
    assert_eq!(shifted.care.to_bits(), (CARE_ABOVE_REST + 0.10).to_bits());

    let high = InnerHome::from_baseline(0.9, 0.2, 0.2, 0.2).expect("clamped care");
    assert_eq!(high.care.to_bits(), 1.0f32.to_bits());
    let low = InnerHome::from_baseline(-1.0, 0.2, 0.2, 0.2).expect("clamped care");
    assert_eq!(low.care.to_bits(), 0.0f32.to_bits());
    assert!(InnerHome::from_baseline(0.2, 1.1, 0.2, 0.2).is_none());
    assert!(InnerHome::from_baseline(0.2, -0.01, 0.2, 0.2).is_none());

    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(InnerHome::from_baseline(bad, 0.4, 0.4, 0.4).is_none());
        assert!(InnerHome::from_baseline(0.2, bad, 0.4, 0.4).is_none());
        assert!(InnerHome::from_baseline(0.2, 0.4, bad, 0.4).is_none());
        assert!(InnerHome::from_baseline(0.2, 0.4, 0.4, bad).is_none());
    }

    let dumped = format!("{once:?}").to_ascii_lowercase();
    for name in ["samantha", "aria", "northwind", "standin"] {
        assert!(!dumped.contains(name), "{dumped}");
    }
    let life = PersonaInner::with_home(once);
    assert_eq!(life.persona_id(), "");
    assert_eq!(life.care().to_bits(), once.care.to_bits());
}
