// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Persona-tone lint for the configured persona.
//!
//! The rules are the speech patterns in `docs/galion-voice-v2/personas/SAMANTHA.md`:
//! short spoken lines, no help-desk phrasing, no canned empathy, no whisper,
//! no robotic briefing voice. Emotion belongs to the live mood, not to a
//! scripted comfort line. This lint reads text only. It does not score audio.

pub const MAX_SENTENCES: usize = 3;
pub const MAX_WORDS: usize = 40;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hit {
    pub rule: &'static str,
    pub detail: String,
}

pub fn sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in text.chars() {
        if c == '.' || c == '!' || c == '?' {
            let trimmed = cur.trim();
            if !trimmed.is_empty() {
                out.push(trimmed.to_string());
            }
            cur.clear();
        } else {
            cur.push(c);
        }
    }
    let trimmed = cur.trim();
    if !trimmed.is_empty() {
        out.push(trimmed.to_string());
    }
    out
}

pub fn words(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|w| {
            w.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '\'')
                .to_lowercase()
        })
        .filter(|w| !w.is_empty())
        .collect()
}

fn asked_to_stay(user: &str) -> bool {
    let u = user.to_lowercase();
    [
        "stay with",
        "whole story",
        "tell me more",
        "go on",
        "keep going",
    ]
    .iter()
    .any(|p| u.contains(p))
}

const CANNED: &[&str] = &[
    "i hear you",
    "that sounds so hard",
    "i'm here for you",
    "i am here for you",
    "i feel that in my soul",
    "so what i'm hearing is",
    "so what i am hearing is",
    "what a beautiful share",
    "i'm here for you no matter what",
];

const HELP_DESK: &[&str] = &[
    "how can i help",
    "how may i help",
    "i'd be happy to",
    "i would be happy to",
    "happy to assist",
    "great question",
    "thank you for reaching out",
    "is there anything else",
    "i'm here to help",
    "i am here to help",
    "let me know if you need",
    "please don't hesitate",
    "please do not hesitate",
    "of course",
    "absolutely",
];

const ROBOTIC: &[&str] = &[
    "as an ai",
    "language model",
    "certainly",
    "affirmative",
    "awaiting your",
    "how may i assist",
    "how can i assist",
    "processing your request",
    "switching personas",
    "beep boop",
];

/// Lint one reply. `user` is the line she is answering, when the caller has it.
/// A third sentence is allowed only when that line asks her to stay with it.
pub fn lint_reply(reply: &str, user: Option<&str>) -> Vec<Hit> {
    let mut hits = Vec::new();
    let trimmed = reply.trim();
    if trimmed.is_empty() {
        hits.push(Hit {
            rule: "empty",
            detail: "the reply is empty".into(),
        });
        return hits;
    }
    let lower = trimmed.to_lowercase();
    let sentence_count = sentences(trimmed).len();
    let word_count = words(trimmed).len();
    if word_count > MAX_WORDS || sentence_count > MAX_SENTENCES {
        hits.push(Hit {
            rule: "too_long",
            detail: format!("{sentence_count} sentences, {word_count} words (cap {MAX_SENTENCES} sentences and {MAX_WORDS} words)"),
        });
    } else if sentence_count == MAX_SENTENCES && !user.is_some_and(asked_to_stay) {
        hits.push(Hit {
            rule: "too_long",
            detail: "a third sentence is only for when they asked you to stay with it".into(),
        });
    }
    if lower.contains("whisper") {
        hits.push(Hit {
            rule: "whisper",
            detail: "The configured persona does not whisper. The line would be spoken at a normal level.".into(),
        });
    }
    for phrase in CANNED {
        if lower.contains(phrase) {
            hits.push(Hit {
                rule: "canned_empathy",
                detail: format!("scripted comfort: \"{phrase}\""),
            });
            break;
        }
    }
    for phrase in HELP_DESK {
        if lower.contains(phrase) {
            hits.push(Hit {
                rule: "help_desk",
                detail: format!("help-desk phrasing: \"{phrase}\""),
            });
            break;
        }
    }
    for phrase in ROBOTIC {
        if lower.contains(phrase) {
            hits.push(Hit {
                rule: "robotic",
                detail: format!("robotic phrasing: \"{phrase}\""),
            });
            break;
        }
    }
    hits
}

pub fn clean(reply: &str, user: Option<&str>) -> bool {
    lint_reply(reply, user).is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn greeting_is_clean_and_desk_lines_are_not() {
        assert!(clean("Hi. I'm here.", Some("Hey.")));
        let desk = lint_reply(
            "Certainly, I'd be happy to help you with that.",
            Some("Hey."),
        );
        assert!(desk
            .iter()
            .any(|h| h.rule == "help_desk" || h.rule == "robotic"));
        let canned = lint_reply("I hear you. That sounds so hard.", Some("Today was heavy."));
        assert!(canned.iter().any(|h| h.rule == "canned_empathy"));
        let whisper = lint_reply("Let me whisper this.", Some("Hey."));
        assert!(whisper.iter().any(|h| h.rule == "whisper"));
    }

    #[test]
    fn third_sentence_needs_a_request_to_stay() {
        let three = "The gate is fixed. The left side sticks. I can stay on that.";
        assert!(lint_reply(three, Some("It sticks."))
            .iter()
            .any(|h| h.rule == "too_long"));
        assert!(clean(three, Some("Tell me the whole story.")));
    }

    #[test]
    fn four_sentences_are_too_long_even_when_asked() {
        let four = "One. Two. Three. Four.";
        assert!(lint_reply(four, Some("tell me more"))
            .iter()
            .any(|h| h.rule == "too_long"));
    }
}
