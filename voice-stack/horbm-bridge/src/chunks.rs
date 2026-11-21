// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Speakable sentence chunks for the streaming TTS path.
//!
//! The rules are the ones in `crates/galion-cascade/src/early_reply.rs`
//! (first sentence streamed to the warm Piper worker):
//! a sentence closes on `.` `!` or `?` at a word boundary, `Mr.` and `3.14`
//! stay open, and a run with no mark closes at [`FIRST_SENTENCE_WORDS`] so
//! a missing period cannot hold the mouth. A leading `[...]` tag is not
//! spoken. A sentence that contains a phrase from [`FORBIDDEN`] is not
//! yielded, and neither is anything after it in that reply.
//!
//! This crate is outside the root workspace, so it cannot depend on
//! `galion-cascade` (that crate uses workspace keys). The list and the split
//! are copied here. `tests` check the list against `hermes.rs` and the word
//! cap against `early_reply.rs`.
//!
//! [`crate::Reply::text`] is the brain's line unchanged. Chunks are what the
//! mouth speaks.

/// Close here when the reply has not sent a sentence mark yet.
/// Same value as `galion_cascade::early_reply::FIRST_SENTENCE_WORDS`.
pub const FIRST_SENTENCE_WORDS: usize = 16;

/// Same character cap as the voice completion guard in `early_reply.rs`.
pub const SPOKEN_CAP: usize = 480;

const ABBREV: &[&str] = &["mr", "mrs", "ms", "dr", "st", "jr", "sr", "prof"];

/// Empathy scripts the mouth must not speak.
/// Same phrases as `galion_cascade::hermes::FORBIDDEN`.
pub const FORBIDDEN: &[&str] = &[
    "i hear you",
    "that sounds so hard",
    "that sounds hard",
    "i'm here for you",
    "i feel that in my soul",
    "i know this is difficult",
    "i'm worried about you",
    "take a deep breath",
    "so what i'm hearing",
    "noted, that is hard",
    "i'm sorry for your loss",
    "let's slow down",
    "i'm sorry",
    "i am sorry",
    "i'm so sorry",
    "i am so sorry",
    "so sorry to hear",
    "sorry to hear",
    "sorry for your loss",
    "my condolences",
    "my deepest condolences",
    "you are not alone",
    "you're not alone",
    "allow yourself to grieve",
    "losing a loved one",
    "i can only imagine",
    "i can't imagine how",
    "that must be so hard",
    "that must be hard",
    "that must be difficult",
];

/// One piece of reply text ready for TTS, in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpeakChunk {
    pub text: String,
    /// 0-based order in this reply.
    pub index: u32,
    pub words: usize,
}

/// Streaming splitter. One chunker per reply.
#[derive(Clone, Debug, Default)]
pub struct Chunker {
    buf: String,
    index: u32,
    blocked: bool,
    chars: usize,
}

impl Chunker {
    pub fn new() -> Self {
        Self::default()
    }

    /// True once a forbidden sentence arrived. Later pushes yield nothing.
    pub fn blocked(&self) -> bool {
        self.blocked
    }

    /// Push a text delta. Returns each new chunk that should be synthesized now.
    pub fn push(&mut self, delta: &str) -> Vec<SpeakChunk> {
        if self.blocked || delta.is_empty() || self.chars >= SPOKEN_CAP {
            return Vec::new();
        }
        self.buf.push_str(delta);
        self.take(false)
    }

    /// Flush a clean tail. Empty when the tail is empty or the reply is blocked.
    ///
    /// Every remaining sentence is returned. `early_reply::EarlyReply::finish`
    /// only hands back the last one; the others stay in its `spoken()` buffer.
    /// This bridge has no second buffer, so the caller receives all of them.
    pub fn finish(&mut self) -> Vec<SpeakChunk> {
        if self.blocked {
            return Vec::new();
        }
        self.take(true)
    }

    fn take(&mut self, finish: bool) -> Vec<SpeakChunk> {
        let mut out = Vec::new();
        if self.blocked {
            return out;
        }
        strip_leading_tag(&mut self.buf);
        if self.buf.starts_with('[') && !self.buf.contains(']') {
            return out;
        }
        loop {
            let Some((raw, rest)) = split_first_sentence(&self.buf, finish) else {
                break;
            };
            self.buf = rest;
            let sentence = strip_brackets(&raw);
            if sentence.is_empty() {
                if self.buf.is_empty() {
                    break;
                }
                continue;
            }
            if contains_forbidden(&sentence) {
                self.blocked = true;
                self.buf.clear();
                break;
            }
            let room = SPOKEN_CAP.saturating_sub(self.chars);
            if room == 0 {
                self.buf.clear();
                break;
            }
            let sentence = clip_sentence(&sentence, room);
            self.chars += sentence.chars().count();
            let words = sentence.split_whitespace().count();
            let chunk = SpeakChunk {
                text: sentence,
                index: self.index,
                words,
            };
            self.index = self.index.saturating_add(1);
            out.push(chunk);
            if self.chars >= SPOKEN_CAP {
                self.buf.clear();
                break;
            }
            if self.buf.is_empty() {
                break;
            }
        }
        out
    }
}

/// Split `text` into speakable chunks, including a tail with no final mark.
pub fn speakable_chunks(text: &str) -> Vec<SpeakChunk> {
    let mut chunker = Chunker::new();
    let mut out = chunker.push(text);
    out.extend(chunker.finish());
    out
}

/// `true` when `reply` contains a [`FORBIDDEN`] phrase.
/// Curly apostrophes and extra whitespace do not hide a phrase.
pub fn contains_forbidden(reply: &str) -> bool {
    let low: String = reply
        .chars()
        .map(|c| match c {
            '\u{2018}' | '\u{2019}' | '\u{02BC}' | '`' => '\'',
            c => c.to_ascii_lowercase(),
        })
        .collect();
    let low = low.split_whitespace().collect::<Vec<_>>().join(" ");
    FORBIDDEN.iter().any(|phrase| low.contains(phrase))
}

fn clip_sentence(sentence: &str, room: usize) -> String {
    let clipped: String = sentence.chars().take(room).collect();
    if clipped.chars().count() < sentence.chars().count() {
        if let Some(i) = clipped.rfind(' ') {
            if i > 40 {
                return clipped[..i].to_string();
            }
        }
    }
    clipped
}

/// Drop one leading `[...]` tag. A partial tag at the start is left in place.
fn strip_leading_tag(buf: &mut String) {
    let trimmed = buf.trim_start();
    if !trimmed.starts_with('[') {
        if trimmed.len() != buf.len() {
            *buf = trimmed.to_string();
        }
        return;
    }
    let Some(end) = trimmed.find(']') else {
        if trimmed.len() != buf.len() {
            *buf = trimmed.to_string();
        }
        return;
    };
    *buf = trimmed[end + 1..].trim_start().to_string();
}

fn strip_brackets(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut depth = 0i32;
    for c in s.chars() {
        if c == '[' {
            depth += 1;
            continue;
        }
        if c == ']' && depth > 0 {
            depth -= 1;
            continue;
        }
        if depth == 0 {
            out.push(c);
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Split one ready sentence off `text`.
///
/// `finish` yields the remaining tail. The rest has its leading space removed.
pub fn split_first_sentence(text: &str, finish: bool) -> Option<(String, String)> {
    let start = text
        .find(|c: char| !c.is_whitespace())
        .unwrap_or(text.len());
    if start == text.len() {
        return None;
    }
    let mut words = 0usize;
    let mut in_word = false;
    let mut i = start;
    while i < text.len() {
        let ch = text[i..].chars().next().unwrap();
        let ch_len = ch.len_utf8();
        if ch.is_whitespace() {
            if in_word {
                in_word = false;
                words += 1;
                if words >= FIRST_SENTENCE_WORDS {
                    let clause = text[start..i].trim().to_string();
                    let rest = text[i..].trim_start().to_string();
                    if !clause.is_empty() {
                        return Some((clause, rest));
                    }
                }
            }
            i += ch_len;
            continue;
        }
        in_word = true;
        if is_sentence_end(text, i, ch) {
            let end = i + ch_len;
            let at_end = end == text.len();
            let next = text[end..].chars().next();
            let boundary = match next {
                Some(c) if c.is_whitespace() => true,
                None if finish || at_end => true,
                _ => false,
            };
            if boundary {
                let sentence = text[start..end].trim().to_string();
                let rest = text[end..].trim_start().to_string();
                if !sentence.is_empty() {
                    return Some((sentence, rest));
                }
            }
        }
        i += ch_len;
    }
    if finish {
        let sentence = text[start..].trim().to_string();
        if !sentence.is_empty() {
            return Some((sentence, String::new()));
        }
    }
    None
}

fn is_sentence_end(text: &str, i: usize, ch: char) -> bool {
    if !matches!(ch, '.' | '!' | '?') {
        return false;
    }
    let prev = text[..i].chars().next_back();
    if matches!(prev, Some(p) if p.is_ascii_digit()) {
        return false;
    }
    if ch == '.' {
        let word = word_before(text, i);
        if ABBREV.iter().any(|a| word.eq_ignore_ascii_case(a)) {
            return false;
        }
    }
    true
}

fn word_before(text: &str, punct: usize) -> &str {
    let head = &text[..punct];
    let start = head
        .rfind(|c: char| c.is_whitespace())
        .map(|i| i + head[i..].chars().next().unwrap().len_utf8())
        .unwrap_or(0);
    head[start..].trim()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_match_the_cascade_streamer() {
        let early = include_str!("../../../crates/galion-cascade/src/early_reply.rs");
        assert!(
            early.contains("pub const FIRST_SENTENCE_WORDS: usize = 16"),
            "early_reply word cap changed"
        );
        assert!(early.contains("const SPOKEN_CAP: usize = 480"));
        let hermes = include_str!("../../../crates/galion-cascade/src/hermes.rs");
        let start = hermes
            .find("pub const FORBIDDEN: &[&str] = &[")
            .expect("FORBIDDEN array");
        let rest = &hermes[start..];
        let end = rest.find("];").expect("FORBIDDEN end");
        let mut parsed = Vec::new();
        for line in rest[..end].lines() {
            let line = line.trim();
            let Some(body) = line.strip_prefix('"') else {
                continue;
            };
            let stop = body.find('"').expect("phrase end");
            parsed.push(&body[..stop]);
        }
        assert_eq!(parsed, FORBIDDEN);
    }

    #[test]
    fn two_sentences_and_a_streamed_period() {
        assert_eq!(
            split_first_sentence("She died this morning. I will stay with that.", false),
            Some((
                "She died this morning.".into(),
                "I will stay with that.".into()
            ))
        );
        let chunks = speakable_chunks("The kettle is on. It can wait.");
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].text, "The kettle is on.");
        assert_eq!(chunks[0].index, 0);
        assert_eq!(chunks[0].words, 4);
        assert_eq!(chunks[1].text, "It can wait.");
        assert_eq!(chunks[1].index, 1);

        let mut live = Chunker::new();
        assert!(live.push("She died this").is_empty());
        let first = live.push(" morning.");
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].text, "She died this morning.");
        assert!(live.finish().is_empty());
    }

    #[test]
    fn abbreviation_and_decimal_stay_one_sentence() {
        let mr = speakable_chunks("Mr. Smith is here.");
        assert_eq!(mr.len(), 1);
        assert_eq!(mr[0].text, "Mr. Smith is here.");
        let pi = speakable_chunks("Pi is 3.14 today.");
        assert_eq!(pi.len(), 1);
        assert_eq!(pi[0].text, "Pi is 3.14 today.");
    }

    #[test]
    fn word_cap_closes_a_run_on_and_the_tail_is_kept() {
        let words = ["one"; FIRST_SENTENCE_WORDS];
        let text = format!("{} extra.", words.join(" "));
        let (head, rest) = split_first_sentence(&text, false).unwrap();
        assert_eq!(head.split_whitespace().count(), FIRST_SENTENCE_WORDS);
        assert_eq!(rest, "extra.");
        let chunks = speakable_chunks(&text);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[1].text, "extra.");
    }

    #[test]
    fn a_tag_is_not_spoken_and_a_forbidden_line_blocks_the_rest() {
        let mut live = Chunker::new();
        assert!(live.push("[Neutral]").is_empty());
        let got = live.push(" Hi! I'm doing well");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].text, "Hi!");
        assert!(!got[0].text.contains('['));

        let tagged = speakable_chunks("That's [laugh] funny.");
        assert_eq!(tagged.len(), 1);
        assert_eq!(tagged[0].text, "That's funny.");

        assert!(speakable_chunks("I'm sorry.").is_empty());
        assert!(speakable_chunks("I\u{2019}m sorry. The kettle is on.").is_empty());
        let plain = speakable_chunks("Sorry.");
        assert_eq!(plain.len(), 1);
        assert_eq!(plain[0].text, "Sorry.");
        assert!(!contains_forbidden("the kettle is on"));
    }

    #[test]
    fn an_open_tail_is_one_chunk() {
        let chunks = speakable_chunks("the kettle is on");
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].text, "the kettle is on");
        assert!(speakable_chunks("   ").is_empty());
    }
}
