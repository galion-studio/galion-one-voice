// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Legal-move filter.
//!
//! Persona and the truth gate run before a move can win. An answer needs a
//! real source. A stock comfort line is not a move we may speak. Ask is
//! always legal, and it is what an illegal answer becomes.

use super::moves::Move;
use crate::research_style::ResearchHit;
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
/// `style:` is a dialogue mark from the research root and is not one either.
/// `research:` is the off-tick gateway, not a research-root card.
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

/// A truth source that is not a style-example hit. A research hit never counts.
pub fn is_sourced_truth(reply: &str, sources: &[String], hits: &[ResearchHit]) -> bool {
    if research_blocks_speech(reply, sources, hits) {
        return false;
    }
    let reply = reply.trim();
    if reply.is_empty() || reply == UNKNOWN_REPLY || is_scripted_comfort(reply) {
        return false;
    }
    sources
        .iter()
        .any(|source| is_real_source(source) && !source_is_research(source, hits))
}

/// The reply is the style example, or the only card source is that hit.
/// An independent truth source keeps its own clause.
pub fn research_blocks_speech(reply: &str, sources: &[String], hits: &[ResearchHit]) -> bool {
    if hits.is_empty() || has_independent(sources, hits) {
        return false;
    }
    let uses_example = hits.iter().any(|hit| reply_uses_example(reply, hit));
    let names_hit = sources
        .iter()
        .any(|source| source_is_research(source, hits));
    uses_example || names_hit
}

pub fn source_is_research(source: &str, hits: &[ResearchHit]) -> bool {
    if source.starts_with("style:") {
        return true;
    }
    let Some(id) = source_id(source) else {
        return false;
    };
    hits.iter().any(|hit| hit.id == id)
}

pub fn reply_uses_example(reply: &str, hit: &ResearchHit) -> bool {
    let reply = reply.trim();
    let text = hit.text.trim();
    if text.is_empty() || reply.is_empty() || reply == UNKNOWN_REPLY {
        return false;
    }
    if reply.eq_ignore_ascii_case(text) {
        return true;
    }
    if crate::cards::index_tokens(text).len() >= 3 {
        let reply_l = reply.to_ascii_lowercase();
        let text_l = text.to_ascii_lowercase();
        return reply_l.contains(&text_l);
    }
    false
}

fn has_independent(sources: &[String], hits: &[ResearchHit]) -> bool {
    sources
        .iter()
        .any(|source| is_real_source(source) && !source_is_research(source, hits))
}

fn source_id(source: &str) -> Option<&str> {
    const PREFIXES: &[&str] = &["card:", "retrieve:", "fixture:", "research:", "style:"];
    for prefix in PREFIXES {
        if let Some(id) = source.strip_prefix(prefix) {
            if !id.is_empty() && !id.contains(':') {
                return Some(id);
            }
        }
    }
    None
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
        assert!(!is_sourced_truth(
            "I don't know.",
            &["honest:unknown".into()],
            &[]
        ));
        assert!(!is_sourced_truth(
            "The kettle is on the hob.",
            &["honest:unknown".into()],
            &[]
        ));
        assert!(is_sourced_truth(
            "The kettle is on the hob.",
            &["card:kettle".into()],
            &[]
        ));
        assert!(!is_real_source("style:soft-pace"));
        assert!(!is_sourced_truth(
            "Keep the line short.",
            &["style:soft-pace".into()],
            &[]
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
