// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Speaking-style hook. Numbers only. No audio.
//!
//! [`StyleHook::cap_sentences`] borrows the spoken reply down to
//! `max_sentences`. It does not allocate.

/// Share of brevity subtracted from a rate of 1. A knob, not a measured curve.
pub const BREVITY_RATE: f32 = 0.15;

/// What the voice reads from the persona. The moment is not an input.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StyleHook {
    pub brevity: f32,
    pub warmth: f32,
    /// Sentences one turn may spend.
    pub max_sentences: u8,
    /// `1 - BREVITY_RATE * brevity`.
    pub rate: f32,
}

pub const fn hook(brevity: f32, warmth: f32, max_sentences: u8) -> StyleHook {
    StyleHook {
        brevity,
        warmth,
        max_sentences,
        rate: 1.0 - BREVITY_RATE * brevity,
    }
}

impl StyleHook {
    /// Longest prefix of `text` with at most `max_sentences` sentences.
    ///
    /// A sentence ends at `.`, `!`, or `?` followed by whitespace or the end
    /// of the text. A decimal (`3.5`), a single-letter initial (`J. R.`),
    /// and a closing quote or bracket after the stop do not split: the
    /// closer stays on the sentence. The prefix includes the gap after the
    /// last kept sentence and stops before the next one. Text with no
    /// terminal stop is one sentence. Empty text stays empty. A budget of
    /// zero still keeps one sentence, so a non-empty text never yields an
    /// empty prefix.
    ///
    /// The cap is applied by the brain (R8 S121) to the spoken reply only.
    /// Sources and evidence are untouched, and the honest unknown line is one
    /// sentence and passes unchanged.
    #[must_use]
    pub fn cap_sentences<'a>(&self, text: &'a str) -> &'a str {
        if text.is_empty() {
            return text;
        }
        let budget = u32::from(self.max_sentences.max(1));
        let bytes = text.as_bytes();
        let mut seen = 0u32;
        let mut i = 0;
        while i < bytes.len() {
            if let Some(end) = sentence_end(bytes, i) {
                seen += 1;
                if seen >= budget {
                    return prefix_through(text, end);
                }
                i = end;
                continue;
            }
            i += 1;
        }
        text
    }
}

fn is_stop(byte: u8) -> bool {
    matches!(byte, b'.' | b'!' | b'?')
}

fn is_closer(byte: u8) -> bool {
    matches!(byte, b'"' | b'\'' | b')' | b']' | b'}')
}

/// `.` between two digits (`3.5`). Not a sentence boundary.
fn is_decimal(bytes: &[u8], i: usize) -> bool {
    bytes[i] == b'.'
        && i > 0
        && i + 1 < bytes.len()
        && bytes[i - 1].is_ascii_digit()
        && bytes[i + 1].is_ascii_digit()
}

/// `.` after one letter (`J.` in `J. R.`). Not a sentence boundary.
fn is_initial(bytes: &[u8], i: usize) -> bool {
    bytes[i] == b'.'
        && i > 0
        && bytes[i - 1].is_ascii_alphabetic()
        && (i == 1 || !bytes[i - 2].is_ascii_alphabetic())
}

/// Byte index just after a real stop and any closing quote or bracket.
/// `None` when `i` is not a sentence boundary.
fn sentence_end(bytes: &[u8], i: usize) -> Option<usize> {
    if !is_stop(bytes[i]) || is_decimal(bytes, i) || is_initial(bytes, i) {
        return None;
    }
    let mut end = i + 1;
    while end < bytes.len() && is_closer(bytes[end]) {
        end += 1;
    }
    (end == bytes.len() || bytes[end].is_ascii_whitespace()).then_some(end)
}

/// Whole text when nothing follows the kept sentences. Otherwise the prefix
/// through the whitespace that separates them from the next sentence.
fn prefix_through(text: &str, end: usize) -> &str {
    let bytes = text.as_bytes();
    let mut prefix = end;
    while prefix < bytes.len() && bytes[prefix].is_ascii_whitespace() {
        prefix += 1;
    }
    if prefix == 0 || prefix >= bytes.len() {
        text
    } else {
        &text[..prefix]
    }
}
