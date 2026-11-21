// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Intent of the current utterance. A fixed phrase table (retrieval) plus
//! a few scores (heuristic). No model and no network.

use crate::scan::{contains_join, contains_phrase, for_each_word};

/// What the caller is doing with this utterance. Closed set.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Intent {
    Chat,
    Ask,
    Task,
    Vent,
    Goodbye,
}

impl Intent {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::Ask => "ask",
            Self::Task => "task",
            Self::Vent => "vent",
            Self::Goodbye => "goodbye",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "chat" => Some(Self::Chat),
            "ask" => Some(Self::Ask),
            "task" => Some(Self::Task),
            "vent" => Some(Self::Vent),
            "goodbye" => Some(Self::Goodbye),
            _ => None,
        }
    }
}

struct Phrase {
    text: &'static str,
    intent: Intent,
    score: i32,
}

/// Exact phrases. Longer and more specific rows carry a higher score.
const PHRASES: &[Phrase] = &[
    Phrase {
        text: "goodbye",
        intent: Intent::Goodbye,
        score: 5,
    },
    Phrase {
        text: "good bye",
        intent: Intent::Goodbye,
        score: 5,
    },
    Phrase {
        text: "good night",
        intent: Intent::Goodbye,
        score: 5,
    },
    Phrase {
        text: "goodnight",
        intent: Intent::Goodbye,
        score: 5,
    },
    Phrase {
        text: "bye",
        intent: Intent::Goodbye,
        score: 5,
    },
    Phrase {
        text: "see you",
        intent: Intent::Goodbye,
        score: 5,
    },
    Phrase {
        text: "talk later",
        intent: Intent::Goodbye,
        score: 5,
    },
    Phrase {
        text: "talk soon",
        intent: Intent::Goodbye,
        score: 5,
    },
    Phrase {
        text: "have to go",
        intent: Intent::Goodbye,
        score: 5,
    },
    Phrase {
        text: "gotta go",
        intent: Intent::Goodbye,
        score: 5,
    },
    Phrase {
        text: "signing off",
        intent: Intent::Goodbye,
        score: 5,
    },
    Phrase {
        text: "i need to vent",
        intent: Intent::Vent,
        score: 5,
    },
    Phrase {
        text: "to vent",
        intent: Intent::Vent,
        score: 5,
    },
    Phrase {
        text: "let me vent",
        intent: Intent::Vent,
        score: 5,
    },
    Phrase {
        text: "just listen",
        intent: Intent::Vent,
        score: 4,
    },
    Phrase {
        text: "remind me",
        intent: Intent::Task,
        score: 5,
    },
    Phrase {
        text: "set a timer",
        intent: Intent::Task,
        score: 5,
    },
    Phrase {
        text: "add a task",
        intent: Intent::Task,
        score: 5,
    },
    Phrase {
        text: "i need you to",
        intent: Intent::Task,
        score: 5,
    },
    Phrase {
        text: "can you",
        intent: Intent::Task,
        score: 3,
    },
    Phrase {
        text: "could you",
        intent: Intent::Task,
        score: 3,
    },
    Phrase {
        text: "would you",
        intent: Intent::Task,
        score: 3,
    },
    Phrase {
        text: "what is",
        intent: Intent::Ask,
        score: 4,
    },
    Phrase {
        text: "what are",
        intent: Intent::Ask,
        score: 4,
    },
    Phrase {
        text: "what time",
        intent: Intent::Ask,
        score: 4,
    },
    Phrase {
        text: "who is",
        intent: Intent::Ask,
        score: 4,
    },
    Phrase {
        text: "who are",
        intent: Intent::Ask,
        score: 4,
    },
    Phrase {
        text: "where is",
        intent: Intent::Ask,
        score: 4,
    },
    Phrase {
        text: "where are",
        intent: Intent::Ask,
        score: 4,
    },
    Phrase {
        text: "when is",
        intent: Intent::Ask,
        score: 4,
    },
    Phrase {
        text: "when did",
        intent: Intent::Ask,
        score: 4,
    },
    Phrase {
        text: "why is",
        intent: Intent::Ask,
        score: 4,
    },
    Phrase {
        text: "why did",
        intent: Intent::Ask,
        score: 4,
    },
    Phrase {
        text: "how do",
        intent: Intent::Ask,
        score: 4,
    },
    Phrase {
        text: "how does",
        intent: Intent::Ask,
        score: 4,
    },
    Phrase {
        text: "how can",
        intent: Intent::Ask,
        score: 4,
    },
    Phrase {
        text: "tell me",
        intent: Intent::Ask,
        score: 4,
    },
    Phrase {
        text: "ask",
        intent: Intent::Ask,
        score: 4,
    },
    Phrase {
        text: "explain",
        intent: Intent::Ask,
        score: 4,
    },
    Phrase {
        text: "how are you",
        intent: Intent::Chat,
        score: 6,
    },
    Phrase {
        text: "good morning",
        intent: Intent::Chat,
        score: 6,
    },
    Phrase {
        text: "good afternoon",
        intent: Intent::Chat,
        score: 6,
    },
    Phrase {
        text: "hello",
        intent: Intent::Chat,
        score: 5,
    },
    Phrase {
        text: "hey",
        intent: Intent::Chat,
        score: 4,
    },
    Phrase {
        text: "hi",
        intent: Intent::Chat,
        score: 4,
    },
];

const DISTRESS: &[&str] = &[
    "tired",
    "exhausted",
    "sad",
    "angry",
    "anxious",
    "frustrated",
    "upset",
    "lonely",
    "miserable",
    "worried",
    "scared",
    "afraid",
];

const STEMS: &[&str] = &[
    "i am so ",
    "i'm so ",
    "im so ",
    "i feel so ",
    "i am ",
    "i'm ",
    "im ",
    "i feel ",
];

const IMPERATIVES: &[&str] = &[
    "add", "book", "call", "cancel", "check", "close", "find", "make", "open", "play", "put",
    "remind", "schedule", "send", "set", "start", "stop", "text", "turn",
];

/// Classify `text`. Empty text is [`Intent::Chat`].
pub fn detect_intent(text: &str) -> Intent {
    if text.trim().is_empty() {
        return Intent::Chat;
    }
    let mut scores = [0i32; 5];
    for phrase in PHRASES {
        if contains_phrase(text, phrase.text) {
            scores[idx(phrase.intent)] += phrase.score;
        }
    }
    if distress_statement(text) {
        scores[idx(Intent::Vent)] += 4;
    }
    if imperative(text) {
        scores[idx(Intent::Task)] += 3;
    }
    if text.as_bytes().contains(&b'?') {
        scores[idx(Intent::Ask)] += 3;
    }
    scores[idx(Intent::Chat)] += 1;
    best(scores)
}

fn idx(intent: Intent) -> usize {
    match intent {
        Intent::Goodbye => 0,
        Intent::Vent => 1,
        Intent::Task => 2,
        Intent::Ask => 3,
        Intent::Chat => 4,
    }
}

/// Tie goes to the earlier, more specific class: goodbye, vent, task, ask, chat.
fn best(scores: [i32; 5]) -> Intent {
    const ORDER: [Intent; 5] = [
        Intent::Goodbye,
        Intent::Vent,
        Intent::Task,
        Intent::Ask,
        Intent::Chat,
    ];
    let mut winner = 4usize;
    for i in 0..4 {
        if scores[i] > scores[winner] {
            winner = i;
        }
    }
    ORDER[winner]
}

fn distress_statement(text: &str) -> bool {
    STEMS
        .iter()
        .any(|stem| DISTRESS.iter().any(|word| contains_join(text, stem, word)))
}

fn imperative(text: &str) -> bool {
    let mut stage = 0u8;
    let mut hit = false;
    for_each_word(text, |word| {
        if stage == 1 {
            return;
        }
        if stage == 0 && word == b"please" {
            stage = 2;
            return;
        }
        hit = IMPERATIVES.iter().any(|verb| verb.as_bytes() == word);
        stage = 1;
    });
    hit
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_question_is_not_a_greeting_and_a_greeting_is_not_a_question() {
        assert_eq!(detect_intent("what is the gate"), Intent::Ask);
        assert_eq!(detect_intent("ask him about the notes"), Intent::Ask);
        assert_eq!(detect_intent("how are you?"), Intent::Chat);
        assert_eq!(detect_intent("bye"), Intent::Goodbye);
        assert_eq!(detect_intent(""), Intent::Chat);
    }
}
