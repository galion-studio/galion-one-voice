// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! One closed intent for this turn.
//!
//! Empathy has already named the line `chat`, `ask`, `task`, `vent`, or
//! `goodbye`. This layer keeps that label, then refines it with a fixed
//! English phrase table for recall, forget, and correction. The scan is the
//! empathy crate's word-boundary scan. That helper is private there, so the
//! same byte loop lives here. No model, no network, no heap.

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
    Forget,
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
            Self::Forget => "forget",
        }
    }

    /// Empathy label first. A recall, forget, or correction phrase on
    /// `heard` wins. Precedence is correction, then forget, then recall.
    /// A forget phrase sitting against "don't", "dont", "do not", or "never"
    /// does not count. An unknown label is chat, which is empathy's label
    /// for an empty line.
    pub(crate) fn read(heard: &str, label: &str) -> Self {
        let base = from_label(label);
        if contains_any(heard, CORRECT) {
            Self::Correct
        } else if contains_forget(heard) {
            Self::Forget
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

/// The person takes a fact back. "I forgot" and "forget-me-not" are not
/// rows here. A hit with "don't", "dont", "do not", or "never" immediately
/// before it is not this intent: "don't forget that" stays chat.
const FORGET: &[&str] = &[
    "forget that",
    "forget what i said",
    "forget what i told you",
    "please forget",
    "don't remember that",
    "stop remembering",
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
    } else if label.eq_ignore_ascii_case("forget") {
        TurnIntent::Forget
    } else {
        TurnIntent::Chat
    }
}

fn contains_any(hay: &str, phrases: &[&str]) -> bool {
    phrases.iter().any(|phrase| contains_phrase(hay, phrase))
}

fn contains_forget(hay: &str) -> bool {
    FORGET.iter().any(|phrase| contains_unnegated(hay, phrase))
}

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric()
}

fn is_token_byte(b: u8) -> bool {
    is_word_byte(b) || b == b'\''
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
    first_phrase_at(hay.as_bytes(), phrase.as_bytes(), 0).is_some()
}

/// Words from the earliest recall cue, when this line is a recall.
/// A correction or a forget keeps `heard` whole.
pub(crate) fn recall_clause(heard: &str) -> &str {
    if TurnIntent::read(heard, "chat") != TurnIntent::Recall {
        return heard;
    }
    let bytes = heard.as_bytes();
    let mut at: Option<usize> = None;
    for phrase in RECALL {
        let Some(hit) = first_phrase_at(bytes, phrase.as_bytes(), 0) else {
            continue;
        };
        at = Some(at.map_or(hit, |prev| prev.min(hit)));
    }
    match at {
        Some(at) => &heard[at..],
        None => heard,
    }
}

/// Words after the earliest recall cue.
///
/// The cue is not part of the diary query. "you" and "remember" are not
/// stopwords, and a stored fact such as a sister's name does not contain
/// them, so searching the whole cue matches nothing. A line that is not a
/// recall is returned whole. A bare cue, with nothing after it, is returned
/// whole so the search is not empty.
pub(crate) fn recall_topic(heard: &str) -> &str {
    if TurnIntent::read(heard, "chat") != TurnIntent::Recall {
        return heard;
    }
    let bytes = heard.as_bytes();
    let mut at: Option<(usize, usize)> = None;
    for phrase in RECALL {
        let Some(hit) = first_phrase_at(bytes, phrase.as_bytes(), 0) else {
            continue;
        };
        let replace = at.is_none_or(|(prev, _)| hit < prev);
        if replace {
            at = Some((hit, phrase.len()));
        }
    }
    let Some((start, len)) = at else {
        return heard;
    };
    let rest = trim_topic(&heard[start + len..]);
    if rest.is_empty() {
        heard
    } else {
        rest
    }
}

/// What a forget line is asking to drop. The topic borrows `heard`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ForgetAsk<'a> {
    /// "forget that", "don't remember that", or "stop remembering".
    That,
    /// Words after "forget what i said about", "forget what i told you about",
    /// or "please forget". Edge punctuation is already removed.
    About(&'a str),
}

/// The forget this line asks for.
///
/// `None` unless [`TurnIntent::read`] of `heard` as chat is
/// [`TurnIntent::Forget`]. A correction wins that read, and so does a
/// negation sitting against the phrase. The topic is a slice of `heard`.
pub(crate) fn forget_clause(heard: &str) -> Option<ForgetAsk<'_>> {
    if TurnIntent::read(heard, "chat") != TurnIntent::Forget {
        return None;
    }
    let Some((at, phrase)) = earliest_unnegated(heard, FORGET) else {
        return Some(ForgetAsk::About(""));
    };
    if is_that_phrase(phrase) {
        return Some(ForgetAsk::That);
    }
    let rest = topic_after(phrase, &heard[at + phrase.len()..]);
    Some(ForgetAsk::About(trim_topic(rest)))
}

fn is_that_phrase(phrase: &str) -> bool {
    matches!(
        phrase,
        "forget that" | "don't remember that" | "stop remembering"
    )
}

/// "forget what i said" and "forget what i told you" take the words after
/// a following "about". "please forget" takes the words after the phrase.
fn topic_after<'a>(phrase: &str, rest: &'a str) -> &'a str {
    if phrase == "forget what i said" || phrase == "forget what i told you" {
        skip_leading_about(rest)
    } else {
        rest
    }
}

fn skip_leading_about(rest: &str) -> &str {
    let Some(at) = first_phrase_at(rest.as_bytes(), b"about", 0) else {
        return rest;
    };
    if rest[..at].bytes().all(|byte| byte.is_ascii_whitespace()) {
        &rest[at + b"about".len()..]
    } else {
        rest
    }
}

fn trim_topic(text: &str) -> &str {
    text.trim_matches(|ch: char| ch.is_ascii_whitespace() || ch.is_ascii_punctuation())
}

/// Earliest unnegated phrase. A longer phrase at the same byte wins.
fn earliest_unnegated<'a>(hay: &str, phrases: &[&'a str]) -> Option<(usize, &'a str)> {
    let bytes = hay.as_bytes();
    let mut best_at = usize::MAX;
    let mut best: Option<&'a str> = None;
    for phrase in phrases {
        let needle = phrase.as_bytes();
        let mut from = 0;
        while let Some(at) = first_phrase_at(bytes, needle, from) {
            if negated_before(bytes, at) {
                from = at.saturating_add(1);
                continue;
            }
            let longer = best.is_some_and(|prev| at == best_at && phrase.len() > prev.len());
            if at < best_at || longer {
                best_at = at;
                best = Some(phrase);
            }
            break;
        }
    }
    best.map(|phrase| (best_at, phrase))
}

fn first_phrase_at(hay: &[u8], phrase: &[u8], from: usize) -> Option<usize> {
    if phrase.is_empty() || from >= hay.len() || hay.len() - from < phrase.len() {
        return None;
    }
    let mut i = from;
    while i + phrase.len() <= hay.len() {
        if boundary_before(hay, i)
            && eq_ci(&hay[i..], phrase)
            && boundary_after(hay, i + phrase.len())
        {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Same scan as `contains_phrase`. A hit with a negation word against its
/// first byte does not count, and the scan keeps looking.
fn contains_unnegated(hay: &str, phrase: &str) -> bool {
    let bytes = hay.as_bytes();
    let needle = phrase.as_bytes();
    let mut from = 0;
    while let Some(at) = first_phrase_at(bytes, needle, from) {
        if !negated_before(bytes, at) {
            return true;
        }
        from = at.saturating_add(1);
    }
    false
}

/// Byte look-back, no heap. The token against `at` is "don't", "dont", or
/// "never", or the two tokens "do not".
fn negated_before(hay: &[u8], at: usize) -> bool {
    let Some((word, start)) = prev_token(hay, at) else {
        return false;
    };
    if token_is(word, b"don't") || token_is(word, b"dont") || token_is(word, b"never") {
        return true;
    }
    if !token_is(word, b"not") {
        return false;
    }
    prev_token(hay, start).is_some_and(|(earlier, _)| token_is(earlier, b"do"))
}

fn prev_token(hay: &[u8], at: usize) -> Option<(&[u8], usize)> {
    let mut end = at;
    while end > 0 && hay[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    if end == 0 || !is_token_byte(hay[end - 1]) {
        return None;
    }
    let mut start = end;
    while start > 0 && is_token_byte(hay[start - 1]) {
        start -= 1;
    }
    Some((&hay[start..end], start))
}

fn token_is(word: &[u8], lit: &[u8]) -> bool {
    word.len() == lit.len() && eq_ci(word, lit)
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
        assert_eq!(TurnIntent::read("thanks", "forget"), TurnIntent::Forget);
        assert_eq!(TurnIntent::read("thanks", " FORGET "), TurnIntent::Forget);

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
        assert_eq!(TurnIntent::Forget.as_str(), "forget");
    }

    #[test]
    fn forget_phrases_read_as_forget() {
        for heard in [
            "forget that",
            "Forget that.",
            "forget what i said",
            "Forget what I said about the bowl",
            "forget what i told you",
            "Forget what I told you",
            "please forget",
            "Please forget",
            "don't remember that",
            "Don't Remember That",
            "stop remembering",
            "STOP REMEMBERING",
            "don't forget that, but please forget",
        ] {
            assert_eq!(
                TurnIntent::read(heard, "chat"),
                TurnIntent::Forget,
                "{heard}"
            );
            assert_eq!(TurnIntent::read(heard, "ask").as_str(), "forget", "{heard}");
        }
    }

    #[test]
    fn dont_forget_is_not_forget() {
        for heard in [
            "please don't forget that",
            "never forget that",
            "Don't forget that",
            "NEVER forget that",
            "do not forget that",
            "Do Not forget that",
            "dont forget that",
            "please dont forget what i said",
            "do  not  forget that",
        ] {
            assert_eq!(TurnIntent::read(heard, "chat"), TurnIntent::Chat, "{heard}");
            assert_ne!(
                TurnIntent::read(heard, "ask"),
                TurnIntent::Forget,
                "{heard}"
            );
        }
        assert_eq!(
            TurnIntent::read("please don't forget that, do you remember", "chat"),
            TurnIntent::Recall
        );
        assert_eq!(
            TurnIntent::read("never forget that, what did I tell you", "ask"),
            TurnIntent::Recall
        );
    }

    #[test]
    fn forgot_and_forget_me_not_are_not_forget() {
        for heard in ["I forgot my keys", "forget-me-not", "forget"] {
            assert_eq!(TurnIntent::read(heard, "chat"), TurnIntent::Chat, "{heard}");
        }
        assert_eq!(
            TurnIntent::read("I forgot my keys, do you remember", "chat"),
            TurnIntent::Recall
        );
    }

    #[test]
    fn correct_beats_forget() {
        for heard in [
            "no, that's wrong, forget that",
            "forget that, no, that's wrong",
            "actually it's blue, please forget",
            "please forget, that is wrong",
            "stop remembering, actually, it's blue",
            "forget that, do you remember, that's wrong",
        ] {
            assert_eq!(
                TurnIntent::read(heard, "recall"),
                TurnIntent::Correct,
                "{heard}"
            );
        }
    }

    #[test]
    fn forget_beats_recall() {
        for heard in [
            "forget that, do you remember",
            "do you remember, forget that",
            "what did I tell you, please forget",
            "stop remembering what did I tell you",
            "don't remember that, do you remember",
        ] {
            assert_eq!(
                TurnIntent::read(heard, "ask"),
                TurnIntent::Forget,
                "{heard}"
            );
        }
    }

    #[test]
    fn recall_clause_starts_at_the_earliest_cue() {
        assert_eq!(
            recall_clause("please don't forget that, do you remember the blue bowl"),
            "do you remember the blue bowl"
        );
        assert_eq!(
            recall_clause("I forgot my keys, do you remember the blue bowl"),
            "do you remember the blue bowl"
        );
        assert_eq!(
            recall_clause("forget-me-not, do you remember the blue bowl"),
            "do you remember the blue bowl"
        );
        assert_eq!(
            recall_clause("Please, Do you remember the bowl"),
            "Do you remember the bowl"
        );
        assert_eq!(
            recall_clause("hello, what did I tell you, do you remember"),
            "what did I tell you, do you remember"
        );
        assert_eq!(
            recall_clause("do you remember the blue bowl"),
            "do you remember the blue bowl"
        );
        assert_eq!(
            recall_clause("forget that, do you remember the blue bowl"),
            "forget that, do you remember the blue bowl"
        );
        assert_eq!(
            recall_clause("no, that's wrong, do you remember the blue bowl"),
            "no, that's wrong, do you remember the blue bowl"
        );
        assert_eq!(
            recall_clause("please don't forget that"),
            "please don't forget that"
        );
        assert_eq!(recall_clause("where is the kettle"), "where is the kettle");
        assert_eq!(recall_clause(""), "");
    }

    #[test]
    fn recall_topic_drops_the_cue_and_keeps_the_fact_words() {
        assert_eq!(recall_topic("do you remember my sister"), "my sister");
        assert_eq!(
            recall_topic("Do you remember the blue bowl?"),
            "the blue bowl"
        );
        assert_eq!(
            recall_topic("what did I tell you about the bowl"),
            "about the bowl"
        );
        assert_eq!(
            recall_topic("please, do you remember my sister."),
            "my sister"
        );
        assert_eq!(recall_topic("do you remember"), "do you remember");
        assert_eq!(recall_topic("do you remember?"), "do you remember?");
        assert_eq!(recall_topic("where is the kettle"), "where is the kettle");
        assert_eq!(
            recall_topic("please don't forget that, do you remember the blue bowl"),
            "the blue bowl"
        );
        assert_eq!(
            recall_topic("forget that, do you remember my sister"),
            "forget that, do you remember my sister"
        );
        assert_eq!(recall_topic(""), "");
    }

    #[test]
    fn forget_clause_is_that_or_the_topic_after_the_phrase() {
        for heard in [
            "forget that",
            "Forget that.",
            "don't remember that",
            "Don't Remember That",
            "please don't remember that",
            "stop remembering",
            "STOP REMEMBERING",
            "stop remembering my sister",
            "forget that, do you remember the blue bowl",
        ] {
            assert_eq!(forget_clause(heard), Some(ForgetAsk::That), "{heard}");
        }
        assert_eq!(
            forget_clause("please forget my sister"),
            Some(ForgetAsk::About("my sister"))
        );
        assert_eq!(
            forget_clause("Please forget my sister."),
            Some(ForgetAsk::About("my sister"))
        );
        assert_eq!(
            forget_clause("please forget, my sister!"),
            Some(ForgetAsk::About("my sister"))
        );
        assert_eq!(
            forget_clause("forget what i said about the bowl"),
            Some(ForgetAsk::About("the bowl"))
        );
        assert_eq!(
            forget_clause("Forget what I said about the bowl."),
            Some(ForgetAsk::About("the bowl"))
        );
        assert_eq!(
            forget_clause("forget what i told you about my sister"),
            Some(ForgetAsk::About("my sister"))
        );
        assert_eq!(
            forget_clause("Forget what I told you about my sister!"),
            Some(ForgetAsk::About("my sister"))
        );
        assert_eq!(
            forget_clause("don't forget that, but please forget the bowl"),
            Some(ForgetAsk::About("the bowl"))
        );
        assert_eq!(
            forget_clause("please forget that"),
            Some(ForgetAsk::About("that"))
        );
        assert_eq!(forget_clause("please forget"), Some(ForgetAsk::About("")));
        assert_eq!(forget_clause("please forget."), Some(ForgetAsk::About("")));
        assert_eq!(
            forget_clause("forget what i said"),
            Some(ForgetAsk::About(""))
        );
        assert_eq!(
            forget_clause("Forget what I told you"),
            Some(ForgetAsk::About(""))
        );
        assert_eq!(
            forget_clause("forget what i said yesterday"),
            Some(ForgetAsk::About("yesterday"))
        );
    }

    #[test]
    fn forget_clause_is_none_unless_the_line_is_forget() {
        for heard in [
            "",
            "hello",
            "where is the kettle",
            "do you remember the blue bowl",
            "what did I tell you about the bowl",
            "I forgot my keys",
            "forget-me-not",
            "forget",
            "please don't forget that",
            "never forget that",
            "do not forget that",
            "dont forget that",
            "please dont forget what i said",
            "never stop remembering",
            "no, that's wrong, forget that",
            "actually it's blue, please forget",
            "please don't forget that, do you remember",
        ] {
            assert_eq!(forget_clause(heard), None, "{heard}");
        }
    }

    #[test]
    fn forget_scan_does_not_allocate() {
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
                "the forget scan must not allocate via {needle}"
            );
        }
        for heard in [
            "forget that",
            "Forget what I said yesterday",
            "please forget what I told you",
            "don't remember that",
            "stop remembering",
            "please don't forget that",
            "never forget that",
            "do not forget that",
            "dont forget that",
            "I forgot my keys",
            "forget-me-not",
            "no, that's wrong, forget that",
            "forget that, do you remember",
            "don't forget that, but please forget",
        ] {
            black_box(TurnIntent::read(black_box(heard), black_box("chat")));
            black_box(forget_clause(black_box(heard)));
        }
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
            ("forget that", "chat"),
            ("please don't forget that", "chat"),
            ("do not forget that", "ask"),
            ("forget what I told you, do you remember", "ask"),
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
