// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! A call ends with the want met, still open, or dropped.
//!
//! Counts come from [`horbm::Brain::want_counts`], which reads the in-tree
//! `WantList` (Galion Labs, MIT, `voice-stack/empathy/src/wants.rs`).
//! The counts are not spoken and the diary is not written.

use horbm::{Brain, CardHit, CardLookup, WantCounts};

struct Empty;

impl CardLookup for Empty {
    fn lookup(&self, _query: &str) -> Option<CardHit> {
        None
    }
}

fn brain() -> Brain {
    Brain::with_cards(Empty, None)
}

fn print_counts(fixture: &str, counts: WantCounts) {
    eprintln!(
        "fixture {fixture} open={} met={} dropped={}",
        counts.open, counts.met, counts.dropped
    );
}

#[test]
fn met_ending_closes_the_open_want() {
    let mut brain = brain();
    assert_eq!(
        brain.want_counts(),
        WantCounts {
            open: 0,
            met: 0,
            dropped: 0
        }
    );
    let _ = brain.turn("I want the gate checked");
    let out = brain.turn("that works");
    let counts = brain.want_counts();
    print_counts("A", counts);
    eprintln!("fixture A reply={}", out.reply);
    assert!(counts.met >= 1, "met={}", counts.met);
    assert_eq!(counts.open, 0);
    assert_eq!(counts.dropped, 0);
}

#[test]
fn unmet_ending_stays_open() {
    let mut brain = brain();
    let out = brain.turn("I want tea");
    let counts = brain.want_counts();
    print_counts("B", counts);
    eprintln!("fixture B reply={}", out.reply);
    assert!(counts.open >= 1, "open={}", counts.open);
    assert_eq!(counts.met, 0);
    assert_eq!(counts.dropped, 0);
    let reply = out.reply.to_ascii_lowercase();
    assert!(
        !reply.contains("that works") && !reply.contains("that's what i wanted"),
        "comfort line papered over an open want: {}",
        out.reply
    );
}

#[test]
fn dropped_ending_is_closed_and_not_met() {
    let mut brain = brain();
    let _ = brain.turn("I want tea");
    let out = brain.turn("never mind");
    let counts = brain.want_counts();
    print_counts("drop", counts);
    eprintln!("fixture drop reply={}", out.reply);
    assert_eq!(counts.open, 0);
    assert_eq!(counts.met, 0);
    assert!(counts.dropped >= 1, "dropped={}", counts.dropped);
}
