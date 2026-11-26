// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! One closed intent for this turn.
//!
//! Empathy has already named the line `chat`, `ask`, `task`, `vent`, or
//! `goodbye`. This layer keeps that label, then refines it with a fixed
//! English phrase table for recall and correction. The scan is the empathy
//! crate's word-boundary scan. That helper is private there, so the same
//! byte loop lives here. No model, no network, no heap.

/// What this turn is asking the dialogue manager to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum TurnIntent {
    Chat,
    Ask,
    Task,
    Vent,
    Goodbye,
    Recall,
    Correct,
}

impl TurnIntent {
    // The dialogue state reads this. Nothing else in the non-test crate does yet.
    #[allow(dead_code)]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::Ask => "ask",
            Self::Task => "task",
            Self::Vent => "vent",
            Self::Goodbye => "goodbye",
            Self::Recall => "recall",
            Self::Correct => "correct",
        }
    }

    /// Empathy label first. A recall or correction phrase on `heard` wins.
    /// A correction phrase wins when both tables match. An unknown label is
    /// chat, which is empathy's label for an empty line.
    pub(crate) fn read(heard: &str, label: &str) -> Self {
        let base = from_label(label);
        if contains_any(heard, CORRECT) {
            Self::Correct
        } else if contains_any(heard, RECALL) {
            Self::Recall
        } else {
            base
        }
    }
}

/// Charter recall: "do you remember…", "what did I tell you…".
const RECALL: &[&str] = &["do you remember", "what did i tell you"];

/// Charter correction: "no, that's wrong", "actually it's…".
/// The comma in "no, that's wrong" is not part of the match: "that's wrong"
/// is the phrase, on a word boundary.
const CORRECT: &[&str] = &[
    "that's wrong",
    "thats wrong",
    "that is wrong",
    "actually it's",
    "actually its",
    "actually it is",
    "actually, it's",
    "actually, its",
    "actually, it is",
];

fn from_label(label: &str) -> TurnIntent {
    let label = label.trim();
    if label.eq_ignore_ascii_case("ask") {
        TurnIntent::Ask
    } else if label.eq_ignore_ascii_case("task") {
        TurnIntent::Task
    } else if label.eq_ignore_ascii_case("vent") {
        TurnIntent::Vent
    } else if label.eq_ignore_ascii_case("goodbye") {
        TurnIntent::Goodbye
    } else if label.eq_ignore_ascii_case("recall") {
        TurnIntent::Recall
    } else if label.eq_ignore_ascii_case("correct") {
        TurnIntent::Correct
    } else {
        TurnIntent::Chat
    }
}

fn contains_any(hay: &str, phrases: &[&str]) -> bool {
    phrases.iter().any(|phrase| contains_phrase(hay, phrase))
}

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric()
}

fn boundary_before(hay: &[u8], at: usize) -> bool {
    at == 0 || !is_word_byte(hay[at - 1])
}

fn boundary_after(hay: &[u8], at: usize) -> bool {
    at >= hay.len() || !is_word_byte(hay[at])
}

fn eq_ci(hay: &[u8], needle: &[u8]) -> bool {
    hay.len() >= needle.len()
        && hay[..needle.len()]
            .iter()
            .zip(needle)
            .all(|(h, n)| h.to_ascii_lowercase() == *n)
}

/// `phrase` is a whole token sequence, case-insensitive. Same rule as
/// `galion_sam_empathy`'s phrase scan.
fn contains_phrase(hay: &str, phrase: &str) -> bool {
    let h = hay.as_bytes();
    let p = phrase.as_bytes();
    if p.is_empty() || h.len() < p.len() {
        return false;
    }
    let mut i = 0;
    while i + p.len() <= h.len() {
        if boundary_before(h, i) && eq_ci(&h[i..], p) && boundary_after(h, i + p.len()) {
            return true;
        }
        i += 1;
    }
    false
}

#[cfg(test)]
mod tests {
    use std::hint::black_box;

    use super::*;

    #[test]
    fn the_phrase_tables_refine_the_empathy_label() {
        assert_eq!(
            TurnIntent::read("where is the kettle", "ask"),
            TurnIntent::Ask
        );
        assert_eq!(TurnIntent::read("set a timer", "task"), TurnIntent::Task);
        assert_eq!(TurnIntent::read("I am so tired", "vent"), TurnIntent::Vent);
        assert_eq!(TurnIntent::read("goodbye", "goodbye"), TurnIntent::Goodbye);
        assert_eq!(TurnIntent::read("hello", "chat"), TurnIntent::Chat);
        assert_eq!(TurnIntent::read("thanks", "GOODBYE"), TurnIntent::Goodbye);
        assert_eq!(TurnIntent::read("thanks", " ask "), TurnIntent::Ask);
        assert_eq!(TurnIntent::read("thanks", "nope"), TurnIntent::Chat);
        assert_eq!(TurnIntent::read("", ""), TurnIntent::Chat);
        assert_eq!(TurnIntent::read("thanks", "recall"), TurnIntent::Recall);
        assert_eq!(TurnIntent::read("thanks", "correct"), TurnIntent::Correct);

        for heard in [
            "do you remember",
            "Do you remember the blue bowl?",
            "what did I tell you",
            "What did I tell you about the bowl",
        ] {
            assert_eq!(
                TurnIntent::read(heard, "ask"),
                TurnIntent::Recall,
                "{heard}"
            );
        }
        for heard in [
            "no, that's wrong",
            "No, thats wrong",
            "that is wrong",
            "actually it's blue",
            "Actually its blue",
            "actually it is blue",
            "actually, it's blue",
            "actually, it is blue",
        ] {
            assert_eq!(
                TurnIntent::read(heard, "ask"),
                TurnIntent::Correct,
                "{heard}"
            );
        }
        assert_eq!(
            TurnIntent::read("no, that's wrong, do you remember", "chat"),
            TurnIntent::Correct
        );
        assert_eq!(
            TurnIntent::read("doyouremember", "ask"),
            TurnIntent::Ask,
            "a phrase needs a word boundary"
        );
        assert_eq!(
            TurnIntent::read("do you remembered", "ask"),
            TurnIntent::Ask
        );
        assert_eq!(TurnIntent::read("that's wrongful", "ask"), TurnIntent::Ask);
        assert_eq!(
            TurnIntent::read("actually it isn't", "chat"),
            TurnIntent::Chat
        );
        assert_eq!(TurnIntent::read("that's enough", "chat"), TurnIntent::Chat);
        assert_eq!(TurnIntent::Chat.as_str(), "chat");
        assert_eq!(TurnIntent::Recall.as_str(), "recall");
        assert_eq!(TurnIntent::Correct.as_str(), "correct");
    }

    /// The library crate forbids unsafe code, so the counting allocator from
    /// `tests/retrieve_alloc.rs` cannot sit in this file. `read` is checked
    /// here by the source of this module: the scan builds nothing on the heap.
    #[test]
    fn read_does_not_allocate() {
        let src = include_str!("intent.rs");
        let head = src.split("#[cfg(test)]").next().expect("module head");
        for needle in [
            "String",
            "Vec",
            "format!",
            "to_string",
            "to_owned",
            "Box::",
            "collect",
        ] {
            assert!(
                !head.contains(needle),
                "TurnIntent::read must not allocate via {needle}"
            );
        }
        for (heard, label) in [
            ("do you remember the blue bowl", "ask"),
            ("what did I tell you", "chat"),
            ("no, that's wrong", "ask"),
            ("actually, it's blue", "chat"),
            ("where is the kettle", "ask"),
            ("I am so tired", "vent"),
            ("goodbye", "goodbye"),
            ("", "chat"),
            ("thanks", "NOPE"),
        ] {
            black_box(TurnIntent::read(black_box(heard), black_box(label)));
        }
    }
}
