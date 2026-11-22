// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Speakable chunks for the streaming TTS path.
//!
//! A sentence still closes on `.` `!` or `?` at a word boundary, `Mr.` and
//! `3.14` stay open, and a run with no mark closes at [`FIRST_SENTENCE_WORDS`]
//! so a missing period cannot hold the mouth. Those sentence rules match
//! `crates/galion-cascade/src/early_reply.rs`.
//!
//! Inside a sentence, the first clause is yielded as soon as it is ready so
//! Piper can start on a long card line. A clause boundary is a comma,
//! semicolon, colon, or dash, or a conjunction (`and`, `but`, `or`, …) when
//! at least [`MIN_CLAUSE_WORDS`] words would already be spoken. A shorter
//! mark stays with the next clause so the mouth does not chop prosody.
//! A sentence end still closes even when it is shorter than that, so
//! `I don't know.` stays one piece. A leading `[...]` tag is not spoken.
//! A sentence that contains a phrase from [`FORBIDDEN`] is not yielded, and
//! neither is anything after it in that reply.
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

/// Do not cut a clause until it has this many words.
/// A sentence end is still spoken when it is shorter.
pub const MIN_CLAUSE_WORDS: usize = 4;

/// Conjunctions that open the next clause. The cut is before the word.
const CONJUNCTIONS: &[&str] = &[
    "and", "but", "or", "nor", "yet", "so", "because", "although", "though", "while", "whereas",
];

/// Whether the first piece handed to Piper is a clause or a whole sentence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SliceGrain {
    /// The piece ends before the sentence does.
    Clause,
    /// The piece is a sentence, or a span the sentence splitter did not cut.
    Sentence,
}

impl SliceGrain {
    pub fn as_str(self) -> &'static str {
        match self {
            SliceGrain::Clause => "clause",
            SliceGrain::Sentence => "sentence",
        }
    }
}

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
    /// `Clause` when this piece ends before the sentence, otherwise `Sentence`.
    pub grain: SliceGrain,
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
            if let Some((raw, rest)) = split_first_sentence(&self.buf, finish) {
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
                let mut stop = false;
                for (text, grain) in pieces_of(&sentence) {
                    if !self.push_chunk(&mut out, text, grain) {
                        stop = true;
                        break;
                    }
                }
                if stop || self.buf.is_empty() {
                    break;
                }
                continue;
            }
            let open = strip_brackets(&self.buf);
            if contains_forbidden(&open) {
                self.blocked = true;
                self.buf.clear();
                break;
            }
            let Some((clause, rest, grain)) = split_first_clause(&self.buf, false) else {
                break;
            };
            if rest.len() >= self.buf.len() {
                break;
            }
            self.buf = rest;
            let clause = strip_brackets(&clause);
            if clause.is_empty() {
                if self.buf.is_empty() {
                    break;
                }
                continue;
            }
            if !self.push_chunk(&mut out, clause, grain) || self.buf.is_empty() {
                break;
            }
        }
        out
    }

    /// Push one piece. `false` means the spoken cap is full and the caller should stop.
    fn push_chunk(&mut self, out: &mut Vec<SpeakChunk>, text: String, grain: SliceGrain) -> bool {
        let room = SPOKEN_CAP.saturating_sub(self.chars);
        if room == 0 {
            self.buf.clear();
            return false;
        }
        let text = clip_sentence(&text, room);
        if text.is_empty() {
            return true;
        }
        self.chars += text.chars().count();
        let words = text.split_whitespace().count();
        let chunk = SpeakChunk {
            text,
            index: self.index,
            words,
            grain,
        };
        self.index = self.index.saturating_add(1);
        out.push(chunk);
        if self.chars >= SPOKEN_CAP {
            self.buf.clear();
            return false;
        }
        true
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

/// Clause pieces of one sentence span, in order.
///
/// The span is what [`split_first_sentence`] already closed. A soft cut or a
/// conjunction becomes [`SliceGrain::Clause`]. The piece that reaches the
/// sentence mark, and a span with no earlier cut, is [`SliceGrain::Sentence`].
fn pieces_of(sentence: &str) -> Vec<(String, SliceGrain)> {
    let mut rest = sentence.to_string();
    let mut out = Vec::new();
    loop {
        if rest.trim().is_empty() {
            break;
        }
        match split_first_clause(&rest, false) {
            Some((clause, next, grain)) => {
                if next.len() >= rest.len() {
                    let tail = rest.trim().to_string();
                    if !tail.is_empty() {
                        let grain = if out.is_empty() {
                            SliceGrain::Sentence
                        } else {
                            SliceGrain::Clause
                        };
                        out.push((tail, grain));
                    }
                    break;
                }
                out.push((clause, grain));
                rest = next;
            }
            None => {
                let tail = rest.trim().to_string();
                if !tail.is_empty() {
                    let grain = if out.is_empty() {
                        SliceGrain::Sentence
                    } else {
                        SliceGrain::Clause
                    };
                    out.push((tail, grain));
                }
                break;
            }
        }
    }
    out
}

/// Split one ready clause off `text`.
///
/// A comma, semicolon, colon, or dash closes a clause once it has
/// [`MIN_CLAUSE_WORDS`] words. A conjunction in [`CONJUNCTIONS`] closes the
/// words before it on the same count. `.` `!` and `?` close at any length,
/// with the same abbreviation and decimal exceptions as [`split_first_sentence`].
/// `finish` yields a tail that has no boundary yet. The rest has its leading
/// space removed.
pub fn split_first_clause(text: &str, finish: bool) -> Option<(String, String, SliceGrain)> {
    let start = text
        .find(|c: char| !c.is_whitespace())
        .unwrap_or(text.len());
    if start == text.len() {
        return None;
    }
    let mut words = 0usize;
    let mut in_word = false;
    let mut word_start = start;
    let mut i = start;
    while i < text.len() {
        let ch = text[i..].chars().next().unwrap();
        let ch_len = ch.len_utf8();
        if ch.is_whitespace() {
            if in_word {
                let word = &text[word_start..i];
                if words >= MIN_CLAUSE_WORDS && is_conjunction(word) {
                    let clause = text[start..word_start].trim().to_string();
                    let rest = text[word_start..].trim_start().to_string();
                    if !clause.is_empty() {
                        return Some((clause, rest, SliceGrain::Clause));
                    }
                }
                in_word = false;
                words += 1;
            }
            i += ch_len;
            continue;
        }
        if let Some(len) = dash_len(text, i) {
            let clause_words = if in_word { words + 1 } else { words };
            let end = i + len;
            if clause_words >= MIN_CLAUSE_WORDS {
                let clause = text[start..end].trim().to_string();
                let rest = text[end..].trim_start().to_string();
                if !clause.is_empty() {
                    return Some((clause, rest, SliceGrain::Clause));
                }
            }
            if in_word {
                words += 1;
                in_word = false;
            }
            i = end;
            continue;
        }
        if !in_word {
            in_word = true;
            word_start = i;
        }
        let end = i + ch_len;
        if is_sentence_end(text, i, ch) && at_break(text, end) {
            let clause = text[start..end].trim().to_string();
            let rest = text[end..].trim_start().to_string();
            if !clause.is_empty() {
                return Some((clause, rest, SliceGrain::Sentence));
            }
        }
        if is_soft_punct(text, i, ch) && at_break(text, end) {
            let clause_words = words + 1;
            if clause_words >= MIN_CLAUSE_WORDS {
                let clause = text[start..end].trim().to_string();
                let rest = text[end..].trim_start().to_string();
                if !clause.is_empty() {
                    return Some((clause, rest, SliceGrain::Clause));
                }
            }
        }
        i = end;
    }
    if finish {
        let clause = text[start..].trim().to_string();
        if !clause.is_empty() {
            return Some((clause, String::new(), SliceGrain::Sentence));
        }
    }
    None
}

fn is_conjunction(word: &str) -> bool {
    CONJUNCTIONS
        .iter()
        .any(|item| word.eq_ignore_ascii_case(item))
}

fn at_break(text: &str, end: usize) -> bool {
    match text[end..].chars().next() {
        None => true,
        Some(c) if c.is_whitespace() => true,
        Some(_) => false,
    }
}

/// Byte length of a dash at `i`, or `None` when this hyphen is inside a word.
fn dash_len(text: &str, i: usize) -> Option<usize> {
    let ch = text[i..].chars().next()?;
    if matches!(ch, '—' | '–') {
        return Some(ch.len_utf8());
    }
    if ch != '-' {
        return None;
    }
    let bytes = text.as_bytes();
    if i + 1 < bytes.len() && bytes[i + 1] == b'-' {
        let mut n = 2;
        while i + n < bytes.len() && bytes[i + n] == b'-' {
            n += 1;
        }
        return Some(n);
    }
    let prev_space = i == 0
        || text[..i]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_whitespace());
    let next_space = match text[i + 1..].chars().next() {
        None => true,
        Some(c) if c.is_whitespace() => true,
        Some(_) => false,
    };
    if prev_space && next_space {
        Some(1)
    } else {
        None
    }
}

fn is_soft_punct(text: &str, i: usize, ch: char) -> bool {
    if !matches!(ch, ',' | ';' | ':') {
        return false;
    }
    let prev = text[..i].chars().next_back();
    !matches!(prev, Some(p) if p.is_ascii_digit())
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
        assert_eq!(chunks[0].grain, SliceGrain::Sentence);
        assert!(speakable_chunks("   ").is_empty());
    }

    #[test]
    fn a_miss_is_one_sentence_and_the_text_is_unchanged() {
        let chunks = speakable_chunks("I don't know.");
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].text, "I don't know.");
        assert_eq!(chunks[0].grain, SliceGrain::Sentence);
        assert_eq!(chunks[0].words, 3);
    }

    #[test]
    fn a_long_card_sentence_yields_the_first_clause_then_the_rest_in_order() {
        let text = "AlphaFold predicts protein structure, and the twenty twenty one paper showed it at scale.";
        let chunks = speakable_chunks(text);
        assert!(chunks.len() >= 2, "{chunks:?}");
        assert_eq!(chunks[0].grain, SliceGrain::Clause);
        assert_eq!(chunks[0].text, "AlphaFold predicts protein structure,");
        assert!(chunks[0].words >= MIN_CLAUSE_WORDS);
        assert_eq!(chunks[0].index, 0);
        let joined = chunks
            .iter()
            .map(|chunk| chunk.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(joined, text);
        assert_eq!(chunks.last().unwrap().grain, SliceGrain::Sentence);
        for pair in chunks.windows(2) {
            assert_eq!(pair[1].index, pair[0].index + 1);
        }

        let mut live = Chunker::new();
        assert!(live.push("AlphaFold predicts protein ").is_empty());
        let first = live.push("structure, and the paper");
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].text, "AlphaFold predicts protein structure,");
        assert_eq!(first[0].grain, SliceGrain::Clause);
        let rest = live.finish();
        assert_eq!(rest.len(), 1);
        assert_eq!(rest[0].text, "and the paper");
        assert_eq!(rest[0].index, 1);
    }

    #[test]
    fn clause_marks_wait_for_four_words_and_keep_the_line() {
        let short = speakable_chunks("Hi, I am here now.");
        assert_eq!(short.len(), 1, "a one-word comma is not a chop");
        assert_eq!(short[0].text, "Hi, I am here now.");
        assert_eq!(short[0].grain, SliceGrain::Sentence);

        let comma = speakable_chunks("alpha beta gamma delta, epsilon.");
        assert_eq!(
            comma.iter().map(|c| c.text.as_str()).collect::<Vec<_>>(),
            vec!["alpha beta gamma delta,", "epsilon."]
        );
        assert_eq!(comma[0].grain, SliceGrain::Clause);
        assert_eq!(comma[1].grain, SliceGrain::Sentence);

        let semi = speakable_chunks("alpha beta gamma delta; epsilon zeta.");
        assert_eq!(semi[0].text, "alpha beta gamma delta;");
        assert_eq!(semi[0].grain, SliceGrain::Clause);

        let colon = speakable_chunks("Read this short note: hello there friend.");
        assert_eq!(colon[0].text, "Read this short note:");
        assert_eq!(colon[0].grain, SliceGrain::Clause);
        assert_eq!(
            colon
                .iter()
                .map(|c| c.text.as_str())
                .collect::<Vec<_>>()
                .join(" "),
            "Read this short note: hello there friend."
        );
        assert_eq!(
            speakable_chunks("Meet at 12:30 today.").len(),
            1,
            "a time colon is not a boundary"
        );

        let dash = speakable_chunks("AlphaFold predicts protein structure — the paper followed.");
        assert_eq!(dash[0].text, "AlphaFold predicts protein structure —");
        assert_eq!(dash[0].grain, SliceGrain::Clause);
        assert_eq!(
            dash.iter()
                .map(|c| c.text.as_str())
                .collect::<Vec<_>>()
                .join(" "),
            "AlphaFold predicts protein structure — the paper followed."
        );
        let ascii = speakable_chunks("AlphaFold predicts protein structure - the paper followed.");
        assert_eq!(ascii[0].text, "AlphaFold predicts protein structure -");
        let doubled =
            speakable_chunks("AlphaFold predicts protein structure -- the paper followed.");
        assert_eq!(doubled[0].text, "AlphaFold predicts protein structure --");
        assert_eq!(
            speakable_chunks("A real-time network predicts structure today.").len(),
            1,
            "a hyphen inside a word is not a dash"
        );

        let conj = speakable_chunks(
            "AlphaFold predicts protein structure and the paper is from twenty twenty one.",
        );
        assert_eq!(conj[0].text, "AlphaFold predicts protein structure");
        assert_eq!(conj[0].grain, SliceGrain::Clause);
        assert!(conj[1].text.starts_with("and "));
        assert_eq!(
            conj.iter()
                .map(|c| c.text.as_str())
                .collect::<Vec<_>>()
                .join(" "),
            "AlphaFold predicts protein structure and the paper is from twenty twenty one."
        );
        assert_eq!(
            speakable_chunks("Go and see the long explanation of this card today.").len(),
            1,
            "a conjunction before four words stays in the sentence"
        );
        assert_eq!(
            speakable_chunks("The android build predicts structure today.").len(),
            1
        );

        let because = speakable_chunks(
            "AlphaFold predicts protein structure because the network reads the sequence.",
        );
        assert_eq!(because[0].text, "AlphaFold predicts protein structure");
        assert!(because[1].text.starts_with("because "));
    }

    #[test]
    fn a_forbidden_sentence_is_not_split_into_a_spoken_prefix() {
        assert!(
            speakable_chunks("The kettle is on the stove, and I'm sorry about that.").is_empty()
        );
        assert!(speakable_chunks("I'm sorry, the kettle is on the stove now.").is_empty());
    }
}
