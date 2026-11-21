// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Sentence chunks for the speaker.
//!
//! The boundary rules match `crates/galion-ml-voice` `speech/clause.rs`:
//! flush on `.` `!` `?` `,` `;` or a dash, or after [`CLAUSE_WORDS`] closed
//! words. Decimal points and thousands separators are not boundaries.
//! Piper speaks one of these chunks at a time. This copy lives here because
//! `samantha-core` is not a workspace member and does not link that crate.

/// Flush here when no punctuation has arrived yet.
pub const CLAUSE_WORDS: usize = 8;

/// Accumulates text and yields chunks ready to synthesize.
#[derive(Clone, Debug, Default)]
pub struct ClauseBuf {
    pending: String,
}

impl ClauseBuf {
    pub fn new() -> Self {
        Self {
            pending: String::new(),
        }
    }

    /// Push text. Returns every chunk that is now ready.
    pub fn push(&mut self, delta: &str) -> Vec<String> {
        if delta.is_empty() {
            return Vec::new();
        }
        self.pending.push_str(delta);
        self.take(false)
    }

    /// Flush whatever is still buffered.
    pub fn finish(&mut self) -> Vec<String> {
        self.take(true)
    }

    fn take(&mut self, finish: bool) -> Vec<String> {
        let mut out = Vec::new();
        loop {
            match split_first_clause(&self.pending, finish) {
                Some((clause, rest)) => {
                    self.pending = rest;
                    if !clause.is_empty() {
                        out.push(clause);
                    }
                    if self.pending.is_empty() {
                        break;
                    }
                }
                None => break,
            }
        }
        out
    }
}

/// Split `text` into speaker chunks. The tail is included.
pub fn clauses(text: &str) -> Vec<String> {
    let mut buf = ClauseBuf::new();
    let mut out = buf.push(text);
    out.extend(buf.finish());
    out
}

/// Split one ready clause off `text`.
pub fn split_first_clause(text: &str, finish: bool) -> Option<(String, String)> {
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
                if words >= CLAUSE_WORDS {
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
        if is_clause_boundary(text, i, ch) {
            let end = i + ch_len;
            let clause = text[start..end].trim().to_string();
            let rest = text[end..].trim_start().to_string();
            if !clause.is_empty() {
                return Some((clause, rest));
            }
        }
        i += ch_len;
    }
    if finish {
        let clause = text[start..].trim().to_string();
        if !clause.is_empty() {
            return Some((clause, String::new()));
        }
    }
    let _ = in_word;
    None
}

fn is_clause_boundary(text: &str, i: usize, ch: char) -> bool {
    if matches!(ch, '—' | '–') {
        return true;
    }
    let next = text[i + ch.len_utf8()..].chars().next();
    let at_break = match next {
        None => true,
        Some(c) if c.is_whitespace() => true,
        Some(_) => false,
    };
    if !at_break {
        return false;
    }
    match ch {
        '!' | '?' => true,
        '.' | ',' | ';' => {
            let prev = text[..i].chars().next_back();
            !matches!(prev, Some(p) if p.is_ascii_digit())
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn punctuation_flushes_before_eight_words() {
        assert_eq!(
            split_first_clause("Hi! I'm doing well, thanks.", false),
            Some(("Hi!".into(), "I'm doing well, thanks.".into()))
        );
        let mut b = ClauseBuf::new();
        assert!(b.push("Hel").is_empty(), "an open word is not a clause");
        assert_eq!(b.push("lo! and more"), vec!["Hello!"]);
        assert_eq!(b.finish(), vec!["and more"]);
    }

    #[test]
    fn eight_words_flush_without_punctuation() {
        let text = "alpha bravo charlie delta echo foxtrot golf hotel india juliet";
        assert_eq!(
            split_first_clause(text, false),
            Some((
                "alpha bravo charlie delta echo foxtrot golf hotel".into(),
                "india juliet".into(),
            ))
        );
        let open = "alpha bravo charlie delta echo foxtrot golf hotel";
        assert_eq!(split_first_clause(open, false), None);
        assert_eq!(split_first_clause(open, true).unwrap().0, open);
    }

    #[test]
    fn decimals_and_thousands_are_not_boundaries() {
        let text = "Use 3.14 and 1,000 then stop";
        assert_eq!(split_first_clause(text, false), None);
        assert_eq!(split_first_clause("you said: hello galion", false), None);
    }

    #[test]
    fn two_sentences_are_two_chunks() {
        assert_eq!(
            clauses("I am here. Glad you came."),
            vec!["I am here.".to_string(), "Glad you came.".to_string()]
        );
    }
}
