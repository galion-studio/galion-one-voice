// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Margin test over the covering lookup's scored cards.
//!
//! The lookup already decided which cards cover the question and scored
//! each one by haystack length (fewer content tokens ranks first). This
//! function sees at most [`crate::MEMORY_HIT_CAP`] of those cards. It does
//! not load a model, does not walk the pack, and compares integers only.
//!
//! The comparison is the whole ranker. The 2026-11-27 brain note searched
//! MIT and Apache Rust for a Hopfield net, a drift-diffusion fit, and a
//! boosted tree, and found nothing smaller than subtracting two scores
//! that are already in hand.

use crate::MEMORY_HIT_CAP;

/// Minimum lead, in haystack content tokens, of the shortest covering card
/// over the next covering card.
///
/// The covering lookup still requires every content token of the question
/// to be in the card, and a shorter haystack still ranks ahead of a longer
/// one. This constant is the second bound. The card is spoken only when
/// that lead is at least [`RANK_MARGIN`]. A tie (lead 0), a smaller lead,
/// or no covering card is not spoken. The turn then says
/// [`crate::UNKNOWN_REPLY`].
///
/// The bound is the evidence threshold and the opt-out in
/// `docs/horbm/ranker-research-2026-11-27/HUMAN-BRAIN-RANKER.md` ("Evidence,
/// a threshold, and when to stop"; "Confidence: knowing you don't know").
/// Ratcliff and McKoon (2008) and Gold and Shadlen (2007) commit when
/// evidence crosses a threshold. Kiani and Shadlen (2009), Kepecs, Uchida,
/// Zariwala, and Mainen (2008), and Pouget, Drugowitsch, and Kepecs (2016)
/// withhold the action when the balance of evidence is a near-tie. That
/// note's floor of 1 abstains on a tie and says a higher floor is not a
/// calibrated probability until a labelled set exists. This step uses 2, so
/// a one-token lead also abstains. It is not a fitted P(correct).
pub const RANK_MARGIN: usize = 2;

/// One card the covering lookup already scored.
///
/// `haystack_tokens` is that lookup's score: the number of distinct content
/// tokens in the card. Fewer is the shorter card. `covers` is the same
/// predicate as the lookup (every content token of the question is in the
/// card). A card that does not cover is not in the commit race.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScoredCandidate<'a> {
    pub id: &'a str,
    pub gist: &'a str,
    pub haystack_tokens: usize,
    pub covers: bool,
}

/// Speak the shortest covering card when it leads the runner-up by
/// [`RANK_MARGIN`] haystack tokens.
///
/// At most [`MEMORY_HIT_CAP`] entries are read. A later entry is not a
/// candidate. No covering card, an empty gist on the shortest covering
/// card, a tie, or a lead below [`RANK_MARGIN`] returns `None`. One covering
/// card and no covering runner-up commits: there is no second score to be
/// close. The id is not a score. Two equal lengths abstain.
#[must_use]
pub fn rank_by_margin<'a>(
    candidates: &'a [ScoredCandidate<'a>],
) -> Option<&'a ScoredCandidate<'a>> {
    let mut best: Option<usize> = None;
    let mut second: Option<usize> = None;
    for (i, candidate) in candidates.iter().enumerate().take(MEMORY_HIT_CAP) {
        if !candidate.covers {
            continue;
        }
        match best {
            None => best = Some(i),
            Some(b) => {
                if candidate.haystack_tokens < candidates[b].haystack_tokens {
                    second = Some(b);
                    best = Some(i);
                } else if second
                    .map(|s| candidate.haystack_tokens < candidates[s].haystack_tokens)
                    .unwrap_or(true)
                {
                    second = Some(i);
                }
            }
        }
    }
    let best = best?;
    if candidates[best].gist.is_empty() {
        return None;
    }
    let Some(second) = second else {
        return Some(&candidates[best]);
    };
    let lead = candidates[second]
        .haystack_tokens
        .saturating_sub(candidates[best].haystack_tokens);
    if lead >= RANK_MARGIN {
        Some(&candidates[best])
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(
        id: &'static str,
        gist: &'static str,
        haystack_tokens: usize,
        covers: bool,
    ) -> ScoredCandidate<'static> {
        ScoredCandidate {
            id,
            gist,
            haystack_tokens,
            covers,
        }
    }

    #[test]
    fn margin_is_two_haystack_tokens_and_the_cap_is_four() {
        assert_eq!(RANK_MARGIN, 2);
        assert_eq!(MEMORY_HIT_CAP, 4);
    }

    #[test]
    fn a_clear_lead_speaks_the_shorter_card() {
        let cards = [
            card("long", "A longer covering card.", 8, true),
            card("short", "The kettle is on the hob.", 3, true),
            card("mid", "A middle covering card.", 6, true),
            card("aside", "This card does not cover.", 1, false),
        ];
        let winner = rank_by_margin(&cards).expect("clear lead");
        assert_eq!(winner.id, "short");
        assert_eq!(winner.gist, "The kettle is on the hob.");
    }

    #[test]
    fn a_tie_abstains() {
        let cards = [
            card("red", "The kettle is red.", 4, true),
            card("blue", "The kettle is blue.", 4, true),
        ];
        assert!(rank_by_margin(&cards).is_none());
    }

    #[test]
    fn a_one_token_lead_is_below_the_margin() {
        let cards = [
            card("short", "The kettle is on the hob.", 4, true),
            card("near", "The kettle is near the hob.", 5, true),
        ];
        assert_eq!(5 - 4, RANK_MARGIN - 1);
        assert!(rank_by_margin(&cards).is_none());
    }

    #[test]
    fn no_covering_card_abstains() {
        let cards = [
            card("a", "Partial overlap only.", 2, false),
            card("b", "Another miss.", 3, false),
            card("c", "", 1, false),
            card("d", "Still not a cover.", 9, false),
        ];
        assert!(rank_by_margin(&cards).is_none());
        assert!(rank_by_margin(&[]).is_none());
    }

    #[test]
    fn one_covering_card_speaks_and_an_empty_gist_does_not() {
        let alone = [card("only", "The kettle is on the hob.", 12, true)];
        assert_eq!(rank_by_margin(&alone).expect("unique cover").id, "only");
        let blank = [card("blank", "", 2, true)];
        assert!(rank_by_margin(&blank).is_none());
        let blank_then_real = [
            card("blank", "", 2, true),
            card("real", "The kettle is on the hob.", 6, true),
        ];
        assert!(rank_by_margin(&blank_then_real).is_none());
    }

    #[test]
    fn equal_lengths_do_not_break_on_a_shorter_gist() {
        let cards = [
            card("long-gist", "The kettle is on the hob today.", 4, true),
            card("short-gist", "Kettle.", 4, true),
        ];
        assert!(rank_by_margin(&cards).is_none());
    }

    #[test]
    fn a_fifth_candidate_is_outside_the_cap() {
        let mut cards = [
            card("a", "Cover a.", 10, true),
            card("b", "Cover b.", 10, true),
            card("c", "Cover c.", 10, true),
            card("d", "Cover d.", 10, true),
            card("short", "The kettle is on the hob.", 2, true),
        ];
        assert!(rank_by_margin(&cards).is_none());
        cards.swap(0, 4);
        let winner = rank_by_margin(&cards).expect("short card inside the four");
        assert_eq!(winner.id, "short");
    }

    /// Wall time of [`rank_by_margin`] on this process. The sample is one
    /// call, so the figure includes the clock. It is not a laptop-gate
    /// number and it is not the card scan.
    #[test]
    fn ranking_step_is_well_under_five_milliseconds() {
        let cards = [
            card("a", "Cover a.", 10, true),
            card("b", "Cover b.", 40, true),
            card("c", "Cover c.", 30, true),
            card("d", "The kettle is on the hob.", 4, true),
        ];
        const WARMUP: usize = 64;
        const SAMPLES: usize = 200;
        for _ in 0..WARMUP {
            std::hint::black_box(rank_by_margin(std::hint::black_box(&cards)));
        }
        let mut nanos = [0u64; SAMPLES];
        for sample in &mut nanos {
            let started = std::time::Instant::now();
            let winner = rank_by_margin(std::hint::black_box(&cards));
            *sample = u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
            std::hint::black_box(winner);
        }
        nanos.sort_unstable();
        let min = nanos[0];
        let median = nanos[SAMPLES / 2];
        let max = nanos[SAMPLES - 1];
        let profile = if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        };
        eprintln!(
            "rank margin: samples={SAMPLES} min_us={:.3} median_us={:.3} max_us={:.3} profile={profile}",
            min as f64 / 1000.0,
            median as f64 / 1000.0,
            max as f64 / 1000.0
        );
        let five_ms = 5_000_000u64;
        assert!(max < five_ms, "ranking step max {max} ns is not under 5 ms");
        assert_eq!(
            rank_by_margin(&cards).expect("timed case still ranks").id,
            "d"
        );
    }
}
