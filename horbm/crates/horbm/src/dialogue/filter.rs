// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Legal-move filter.
//!
//! Persona and the truth gate run before a move can win. An answer needs a
//! real source. A stock comfort line is not a move we may speak. Ask is
//! always legal, and it is what an illegal answer becomes.

use super::moves::Move;
use crate::UNKNOWN_REPLY;

/// Stock comfort lines from `docs/galion-voice-v2/personas/GALION-ONE.md`.
/// Whole reply only, after trim and one trailing punctuation mark.
/// The voice contract lists the same four Samantha lines. This layer does
/// not depend on that crate.
const SCRIPTED_COMFORT: &[&str] = &[
    "i hear you",
    "that sounds so hard",
    "i'm here for you no matter what",
    "i feel that in my soul",
    "that sounds hard",
    "i'm so sorry you're going through this",
    "let's slow down",
    "i know how you feel",
    "that must be so difficult",
];

pub fn is_scripted_comfort(reply: &str) -> bool {
    let trimmed = reply.trim().trim_end_matches(['.', '!', '?']).trim();
    let lower = trimmed.to_ascii_lowercase();
    SCRIPTED_COMFORT.iter().any(|line| lower == *line)
}

/// A source the truth gate will stand behind. `honest:unknown` is not one.
pub fn is_real_source(source: &str) -> bool {
    let id = source
        .strip_prefix("card:")
        .or_else(|| source.strip_prefix("retrieve:"))
        .or_else(|| source.strip_prefix("fixture:"))
        .or_else(|| source.strip_prefix("research:"));
    match id {
        Some(id) => !id.is_empty(),
        None => false,
    }
}

pub fn real_source(sources: &[String]) -> Option<&str> {
    sources
        .iter()
        .find(|source| is_real_source(source))
        .map(String::as_str)
}

/// True when `reply` is a stored clause and a real source is attached.
pub fn is_sourced(reply: &str, sources: &[String]) -> bool {
    let reply = reply.trim();
    if reply.is_empty() || reply == UNKNOWN_REPLY || is_scripted_comfort(reply) {
        return false;
    }
    real_source(sources).is_some()
}

pub struct Legal {
    pub sourced: bool,
    pub stop: bool,
    pub cite_ok: bool,
    pub offer: bool,
    pub vent: bool,
}

pub fn legal(move_: Move, gate: &Legal) -> bool {
    match move_ {
        Move::Answer => gate.sourced,
        Move::Cite => gate.cite_ok,
        Move::Ask => true,
        Move::Comfort => gate.vent && !gate.sourced,
        Move::OfferTopic => gate.offer,
        Move::Stop => gate.stop,
    }
}

/// Drop illegal arms to `i32::MIN`. Ask stays.
pub fn mask(mut scores: [i32; 6], gate: &Legal) -> [i32; 6] {
    for move_ in Move::ALL {
        if move_ != Move::Ask && !legal(move_, gate) {
            scores[move_.index()] = i32::MIN;
        }
    }
    scores
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answer_needs_a_real_source_and_a_stock_comfort_line_is_not_one() {
        assert!(!is_sourced("I don't know.", &["honest:unknown".into()]));
        assert!(!is_sourced(
            "The kettle is on the hob.",
            &["honest:unknown".into()]
        ));
        assert!(is_sourced(
            "The kettle is on the hob.",
            &["card:kettle".into()]
        ));
        assert!(is_scripted_comfort("I hear you."));
        assert!(!is_scripted_comfort("I don't know."));
        assert!(!legal(
            Move::Answer,
            &Legal {
                sourced: false,
                stop: false,
                cite_ok: false,
                offer: false,
                vent: true,
            }
        ));
        assert!(legal(
            Move::Ask,
            &Legal {
                sourced: false,
                stop: false,
                cite_ok: false,
                offer: false,
                vent: false,
            }
        ));
        assert!(legal(
            Move::Comfort,
            &Legal {
                sourced: false,
                stop: false,
                cite_ok: false,
                offer: false,
                vent: true,
            }
        ));
    }
}
