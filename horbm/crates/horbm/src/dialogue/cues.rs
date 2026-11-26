// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Phrase cues for the heuristic. Fixed strings. No model.

use super::intent::TurnIntent;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckInPhrase {
    On,
    Off,
    None,
}

pub fn check_in_phrase(text: &str) -> CheckInPhrase {
    let line = normalize(text);
    if matches!(
        line.as_str(),
        "check ins on" | "turn check ins on" | "checkins on"
    ) {
        CheckInPhrase::On
    } else if matches!(
        line.as_str(),
        "check ins off" | "turn check ins off" | "checkins off"
    ) {
        CheckInPhrase::Off
    } else {
        CheckInPhrase::None
    }
}

/// Goodbye, or a whole line whose only job is to end the talk.
pub fn is_stop(text: &str, intent: TurnIntent) -> bool {
    if intent == TurnIntent::Goodbye {
        return true;
    }
    matches!(
        normalize(text).as_str(),
        "stop" | "please stop" | "stop talking" | "be quiet" | "that's enough" | "quiet"
    )
}

pub fn is_cite(text: &str) -> bool {
    has_phrase(text, "cite")
        || has_phrase(text, "citation")
        || has_phrase(text, "where did you get")
        || has_phrase(text, "where did that come from")
        || has_phrase(text, "what is your source")
        || has_phrase(text, "what's your source")
}

pub fn is_switch(text: &str) -> bool {
    has_phrase(text, "something else")
        || has_phrase(text, "change the subject")
        || has_phrase(text, "change the topic")
        || has_phrase(text, "new topic")
        || has_phrase(text, "another topic")
        || has_phrase(text, "different topic")
        || has_phrase(text, "what else")
}

/// A short greeting. This is the only moment a check-in may speak.
pub fn is_greeting(text: &str) -> bool {
    let words = words(text);
    if words.is_empty() || words.len() > 3 {
        return false;
    }
    words.iter().all(|word| {
        matches!(
            word.as_str(),
            "hello" | "hey" | "hi" | "good" | "morning" | "afternoon" | "there"
        )
    })
}

/// Last content word, if this line is about a thing we can return to.
pub fn topic_word(text: &str) -> Option<String> {
    if is_stop(text, TurnIntent::Chat)
        || is_switch(text)
        || is_greeting(text)
        || check_in_phrase(text) != CheckInPhrase::None
    {
        return None;
    }
    let mut found = None;
    for word in words(text) {
        if word.len() > 32 || is_skip(&word) {
            continue;
        }
        if !word.bytes().any(|b| b.is_ascii_alphabetic()) {
            continue;
        }
        found = Some(word);
    }
    found
}

fn normalize(text: &str) -> String {
    let mut flat = String::new();
    for ch in text.chars() {
        if ch == '-' {
            flat.push(' ');
        } else {
            flat.push(ch);
        }
    }
    flat.trim()
        .trim_end_matches(['.', '!', '?'])
        .trim()
        .to_ascii_lowercase()
}

fn words(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            cur.push(ch.to_ascii_lowercase());
        } else if !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn has_phrase(text: &str, phrase: &str) -> bool {
    let text = text.to_ascii_lowercase();
    let phrase = phrase.to_ascii_lowercase();
    let hay = text.as_bytes();
    let needle = phrase.as_bytes();
    if needle.is_empty() || hay.len() < needle.len() {
        return false;
    }
    let mut i = 0;
    while i + needle.len() <= hay.len() {
        if &hay[i..i + needle.len()] == needle {
            let before = i == 0 || !hay[i - 1].is_ascii_alphanumeric();
            let after =
                i + needle.len() == hay.len() || !hay[i + needle.len()].is_ascii_alphanumeric();
            if before && after {
                return true;
            }
        }
        i += 1;
    }
    false
}

fn is_skip(word: &str) -> bool {
    matches!(
        word,
        "a" | "an"
            | "the"
            | "is"
            | "are"
            | "was"
            | "were"
            | "where"
            | "what"
            | "when"
            | "why"
            | "how"
            | "who"
            | "which"
            | "did"
            | "do"
            | "does"
            | "i"
            | "you"
            | "he"
            | "she"
            | "they"
            | "them"
            | "him"
            | "her"
            | "it"
            | "to"
            | "of"
            | "and"
            | "or"
            | "on"
            | "in"
            | "my"
            | "me"
            | "am"
            | "so"
            | "please"
            | "that"
            | "this"
            | "from"
            | "for"
            | "with"
            | "about"
            | "else"
            | "something"
            | "anything"
            | "talk"
            | "change"
            | "subject"
            | "topic"
            | "new"
            | "back"
            | "cite"
            | "citation"
            | "source"
            | "stop"
            | "goodbye"
            | "bye"
            | "hello"
            | "hey"
            | "hi"
            | "your"
            | "yours"
            | "come"
            | "get"
            | "got"
            | "tired"
            | "exhausted"
            | "sad"
            | "angry"
            | "anxious"
            | "frustrated"
            | "upset"
            | "lonely"
            | "miserable"
            | "worried"
            | "scared"
            | "afraid"
            | "sorry"
    )
}
