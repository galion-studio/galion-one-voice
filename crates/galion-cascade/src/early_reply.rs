// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
#![allow(dead_code)] // phone lane calls this once the first sentence is streamed
//! First sentence of a streaming reply, ready for TTS before the rest arrives.
//!
//! A sentence closes on `.`, `!`, or `?` once that mark is at a word boundary.
//! Abbreviations (`Mr.`) and decimals (`3.14`) stay open. A long run with no
//! mark closes at [`FIRST_SENTENCE_WORDS`] so a missing period cannot hold the mouth.
//! A leading `[Emotion]` tag is not spoken. A forbidden sentence is not yielded.

use crate::hermes::contains_forbidden;

/// Close here when the model has not sent a sentence mark yet.
pub const FIRST_SENTENCE_WORDS: usize = 16;
/// Same character cap as the voice completion guard.
const SPOKEN_CAP: usize = 480;

const ABBREV: &[&str] = &["mr", "mrs", "ms", "dr", "st", "jr", "sr", "prof"];

/// Accumulates model deltas and yields sentences that are safe to speak.
#[derive(Clone, Debug, Default)]
pub struct EarlyReply {
    buf: String,
    spoken: Vec<String>,
    blocked: bool,
    chars: usize,
}

impl EarlyReply {
    pub fn new() -> Self {
        Self::default()
    }

    /// True once a forbidden sentence arrived. Nothing after that is spoken.
    pub fn blocked(&self) -> bool {
        self.blocked
    }

    /// Sentences yielded so far, joined with single spaces.
    pub fn spoken(&self) -> String {
        self.spoken.join(" ")
    }

    /// Push a model delta. Returns each new sentence that should be synthesized now.
    pub fn push(&mut self, delta: &str) -> Vec<String> {
        if self.blocked || delta.is_empty() || self.chars >= SPOKEN_CAP {
            return Vec::new();
        }
        self.buf.push_str(delta);
        self.take(false)
    }

    /// Flush a clean tail. `None` when the tail is empty or forbidden.
    pub fn finish(&mut self) -> Option<String> {
        if self.blocked {
            return None;
        }
        let mut got = self.take(true);
        got.pop()
    }

    fn take(&mut self, finish: bool) -> Vec<String> {
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
            if contains_forbidden(&sentence)
                || contains_forbidden(&format!("{} {}", self.spoken(), sentence))
            {
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
            self.spoken.push(sentence.clone());
            out.push(sentence);
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
    fn first_sentence_closes_before_the_second() {
        assert_eq!(
            split_first_sentence("She died this morning. I will stay with that.", false),
            Some((
                "She died this morning.".into(),
                "I will stay with that.".into()
            ))
        );
        let mut early = EarlyReply::new();
        let first = early.push("She died this morning.");
        assert_eq!(first, vec!["She died this morning.".to_string()]);
        assert!(
            early.push(" I will stay with that.").is_empty() || early.spoken().contains("stay")
        );
        let _ = early.finish();
        assert_eq!(
            early.spoken(),
            "She died this morning. I will stay with that."
        );
        assert!(!early.blocked());
    }

    #[test]
    fn deltas_flush_the_period_as_soon_as_it_arrives() {
        let mut early = EarlyReply::new();
        assert!(early.push("She died this").is_empty());
        assert_eq!(
            early.push(" morning."),
            vec!["She died this morning.".to_string()]
        );
        assert!(early.finish().is_none());
    }

    #[test]
    fn abbreviation_and_decimal_stay_open() {
        assert_eq!(
            split_first_sentence("Mr. Smith is here.", false),
            Some(("Mr. Smith is here.".into(), String::new()))
        );
        assert!(split_first_sentence("Mr. Smith", false).is_none());
        assert_eq!(
            split_first_sentence("Pi is 3.14 today.", false),
            Some(("Pi is 3.14 today.".into(), String::new()))
        );
    }

    #[test]
    fn leading_tag_is_not_spoken() {
        let mut early = EarlyReply::new();
        assert!(early.push("[Neutral]").is_empty());
        let got = early.push(" Hi! I'm doing well");
        assert_eq!(got, vec!["Hi!".to_string()]);
        assert!(!early.spoken().contains('['));
    }

    #[test]
    fn forbidden_sentence_is_not_yielded() {
        let mut early = EarlyReply::new();
        assert!(early.push("I'm sorry.").is_empty());
        assert!(early.blocked());
        assert!(early.spoken().is_empty());
        assert!(early.finish().is_none());
        assert!(early.push(" More words.").is_empty());
    }

    #[test]
    fn word_cap_closes_a_run_on() {
        let words = ["one"; FIRST_SENTENCE_WORDS];
        let text = format!("{} extra.", words.join(" "));
        let (head, rest) = split_first_sentence(&text, false).unwrap();
        assert_eq!(head.split_whitespace().count(), FIRST_SENTENCE_WORDS);
        assert_eq!(rest, "extra.");
    }
}
