//! Galion Voice — Perpetual Open MIT License
//!
//! G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network
//! Developed and distributed by Galion Labs
//! Perpetual Open MIT License
//! Copyright © 2026 Galion Labs. All rights reserved.
//!
//! Local notes index for Galion Research. Okapi BM25 over text the session
//! already holds: attached sources, or notes inserted beside them. Keyword
//! overlap only. No socket, no weight, no download.
//!
//! A Found line is a verbatim span of a note that was inserted. A citation is
//! that note's id and name. Inferred stays empty: this index does not reason
//! a step the note does not state. A query word that is in no note is Unknown.
//! Nothing here invents an id, a title, or a measurement.

use std::collections::HashMap;

use crate::hermes::{
    contains_forbidden, ResearchAnswer, ResearchSource, MAX_RESEARCH_SOURCES,
    MAX_RESEARCH_SOURCE_CHARS,
};

/// Notes kept in one index. A bound, not a measured capacity.
pub const MAX_NOTES: usize = 64;
/// Usual Okapi BM25 term-frequency saturation (Robertson). Not a Galion measurement.
const K1: f64 = 1.2;
/// Usual Okapi BM25 length normalisation (Robertson). Not a Galion measurement.
const B: f64 = 0.75;
/// Spoken limit when the far path did not answer. The desk labels the turn Offline.
const OFFLINE_LIMIT: &str = "Offline. The research path is not reachable from here.";
const NO_SOURCE: &str = "I don't have a source for that.";

/// Closed English function words. They are not evidence of a hit.
const STOP: &[&str] = &[
    "a", "an", "the", "of", "and", "or", "to", "in", "on", "for", "is", "are", "was", "were", "be",
    "what", "which", "who", "whom", "whose", "why", "how", "when", "where", "does", "do", "did",
    "can", "could", "would", "should", "from", "with", "that", "this", "these", "those", "it",
    "its", "as", "at", "by", "if", "not", "no", "yes", "into", "about", "than", "then", "there",
    "their",
];

struct NoteDoc {
    id: u32,
    name: String,
    /// Text actually indexed. Every passage is a substring of this.
    text: String,
    passages: Vec<String>,
    tf: HashMap<String, u32>,
    len: u32,
}

/// In-memory keyword index. Dropping it drops the notes. Nothing is written.
#[derive(Default)]
pub struct NotesIndex {
    docs: Vec<NoteDoc>,
    df: HashMap<String, u32>,
}

impl NotesIndex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.docs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.docs.is_empty()
    }

    /// One note per attached source that has readable text. Images (empty text)
    /// and blank names are not indexed, so they cannot be cited.
    pub fn from_sources(sources: &[ResearchSource]) -> Self {
        let mut index = Self::new();
        for s in sources.iter().take(MAX_RESEARCH_SOURCES) {
            index.insert(s.id, &s.name, &s.text);
        }
        index
    }

    /// Insert or replace one note. Returns false when there is nothing to index
    /// (empty name, id 0, no usable words, or the index is already at [`MAX_NOTES`]).
    pub fn insert(&mut self, id: u32, name: &str, text: &str) -> bool {
        let name = name.trim();
        if id == 0 || name.is_empty() {
            return false;
        }
        let stored = clip_chars(text.trim(), MAX_RESEARCH_SOURCE_CHARS);
        let passages = usable_passages(&stored);
        if passages.is_empty() {
            return false;
        }
        let mut tf: HashMap<String, u32> = HashMap::new();
        for passage in &passages {
            for term in content_tokens(passage) {
                *tf.entry(term).or_insert(0) += 1;
            }
        }
        if tf.is_empty() {
            return false;
        }
        let len = tf.values().sum::<u32>().max(1);
        if let Some(pos) = self.docs.iter().position(|d| d.id == id) {
            self.forget_df(pos);
            self.docs.remove(pos);
        } else if self.docs.len() >= MAX_NOTES {
            return false;
        }
        let text = passages.join("\n");
        for term in tf.keys() {
            *self.df.entry(term.clone()).or_insert(0) += 1;
        }
        self.docs.push(NoteDoc {
            id,
            name: clip_chars(name, 80),
            text,
            passages,
            tf,
            len,
        });
        true
    }

    fn forget_df(&mut self, pos: usize) {
        let terms: Vec<String> = self.docs[pos].tf.keys().cloned().collect();
        for term in terms {
            let left = self.df.get(&term).copied().unwrap_or(0).saturating_sub(1);
            if left == 0 {
                self.df.remove(&term);
            } else {
                self.df.insert(term, left);
            }
        }
    }

    fn vocab_has(&self, term: &str) -> bool {
        self.docs.iter().any(|d| d.tf.contains_key(term))
    }
}

/// Answer from `index` only. Citations are ids that were inserted. Passages are
/// copied from those notes. Inferred is empty.
pub fn answer_offline(question: &str, index: &NotesIndex) -> ResearchAnswer {
    let query = content_terms(question);
    let ranked = rank(index, &query);
    let mut found = Vec::new();
    let mut citations: Vec<(u32, String)> = Vec::new();
    let mut spoken: Option<String> = None;

    for doc_i in ranked {
        let doc = &index.docs[doc_i];
        let mut added = false;
        for passage in best_passages(doc, &query) {
            if !doc.text.contains(passage.as_str()) {
                continue;
            }
            if spoken.is_none() {
                spoken = Some(passage.clone());
            }
            if found.len() < 8 {
                found.push(passage);
            }
            added = true;
        }
        if added
            && citations.len() < MAX_RESEARCH_SOURCES
            && !citations.iter().any(|(n, _)| *n == doc.id)
        {
            citations.push((doc.id, doc.name.clone()));
        }
    }

    let missing: Vec<String> = query.into_iter().filter(|t| !index.vocab_has(t)).collect();

    if found.is_empty() {
        return ResearchAnswer {
            text: format!("Unknown. {NO_SOURCE} {OFFLINE_LIMIT}"),
            citations: Vec::new(),
            found: Vec::new(),
            inferred: Vec::new(),
            unknown: vec![NO_SOURCE.to_string()],
            standing: "Unknown",
        };
    }

    let passage = spoken.unwrap_or_else(|| found[0].clone());
    let mut text = speak(&passage);
    let mut unknown = Vec::new();
    if !missing.is_empty() {
        let gap = gap_line(&missing);
        text.push(' ');
        text.push_str(&gap);
        unknown.push(gap);
    }
    text.push(' ');
    text.push_str(OFFLINE_LIMIT);

    ResearchAnswer {
        text,
        citations,
        found,
        inferred: Vec::new(),
        unknown,
        standing: "Found",
    }
}

fn speak(passage: &str) -> String {
    let mut s = passage.trim().to_string();
    if !s.ends_with(['.', '?', '!']) {
        s.push('.');
    }
    s
}

fn gap_line(missing: &[String]) -> String {
    let shown: Vec<&str> = missing.iter().take(8).map(String::as_str).collect();
    format!("Not in the local notes: {}.", shown.join(", "))
}

fn rank(index: &NotesIndex, query: &[String]) -> Vec<usize> {
    if index.docs.is_empty() || query.is_empty() {
        return Vec::new();
    }
    let n = index.docs.len() as f64;
    let avgdl = index.docs.iter().map(|d| d.len as f64).sum::<f64>() / n;
    if !(avgdl > 0.0) {
        return Vec::new();
    }
    let mut scored: Vec<(usize, f64)> = Vec::new();
    for (i, doc) in index.docs.iter().enumerate() {
        let mut score = 0.0;
        for term in query {
            let f = match doc.tf.get(term) {
                Some(v) if *v > 0 => *v as f64,
                _ => continue,
            };
            let df = index.df.get(term).copied().unwrap_or(0) as f64;
            let idf = (1.0 + (n - df + 0.5) / (df + 0.5)).ln();
            let norm = 1.0 - B + B * (doc.len as f64 / avgdl);
            score += idf * (f * (K1 + 1.0)) / (f + K1 * norm);
        }
        if score > 0.0 {
            scored.push((i, score));
        }
    }
    scored.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    scored.into_iter().map(|(i, _)| i).collect()
}

fn best_passages(doc: &NoteDoc, query: &[String]) -> Vec<String> {
    let mut rows: Vec<(usize, usize, String)> = Vec::new();
    for (ord, passage) in doc.passages.iter().enumerate() {
        let overlap = query
            .iter()
            .filter(|t| content_tokens(passage).iter().any(|p| p == *t))
            .count();
        if overlap == 0 {
            continue;
        }
        rows.push((overlap, ord, passage.clone()));
    }
    rows.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    rows.into_iter().take(3).map(|(_, _, p)| p).collect()
}

fn usable_passages(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for sentence in sentences(text) {
        if contains_forbidden(sentence) {
            continue;
        }
        let passage = clip_passage(sentence);
        if passage.is_empty() || contains_forbidden(passage) {
            continue;
        }
        out.push(passage.to_string());
    }
    out
}

fn sentences(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        let break_here = c == '\n'
            || c == '\r'
            || ((c == '.' || c == '?' || c == '!')
                && chars.peek().map(|(_, n)| n.is_whitespace()).unwrap_or(true));
        if !break_here {
            continue;
        }
        let end = if c == '\n' || c == '\r' {
            i
        } else {
            i + c.len_utf8()
        };
        push_sentence(text, start, end, &mut out);
        start = end;
        if c == '\r' {
            if let Some(&(j, '\n')) = chars.peek() {
                chars.next();
                start = j + 1;
            }
        }
    }
    push_sentence(text, start, text.len(), &mut out);
    out
}

fn push_sentence<'a>(text: &'a str, start: usize, end: usize, out: &mut Vec<&'a str>) {
    if start >= end || end > text.len() {
        return;
    }
    let trimmed = text[start..end].trim();
    if !trimmed.is_empty() {
        out.push(trimmed);
    }
}

fn clip_passage(s: &str) -> &str {
    const CAP: usize = 400;
    if s.chars().count() <= CAP {
        return s;
    }
    let end = s.char_indices().nth(CAP).map(|(i, _)| i).unwrap_or(s.len());
    let head = &s[..end];
    if let Some(i) = head.rfind(' ') {
        if i >= 40 {
            return &s[..i];
        }
    }
    head
}

fn clip_chars(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

fn content_tokens(s: &str) -> Vec<String> {
    split_terms(s)
        .filter(|t| t.chars().count() >= 2 && !is_stop(t))
        .collect()
}

fn content_terms(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    for t in content_tokens(s) {
        if !out.iter().any(|e| e == &t) {
            out.push(t);
        }
    }
    out
}

fn split_terms(s: &str) -> impl Iterator<Item = String> + '_ {
    let mut cur = String::new();
    let mut out = Vec::new();
    for c in s.chars() {
        if c.is_alphanumeric() {
            for lower in c.to_lowercase() {
                cur.push(lower);
            }
        } else if !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out.into_iter()
}

fn is_stop(term: &str) -> bool {
    STOP.iter().any(|s| *s == term)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wash_index() -> NotesIndex {
        let mut index = NotesIndex::new();
        assert!(index.insert(1, "wash-note.txt", "The wash is amber."));
        assert!(index.insert(2, "kettle.txt", "A kettle boils water."));
        index
    }

    #[test]
    fn ranks_the_note_that_shares_the_words_and_cites_only_that_note() {
        let index = wash_index();
        let answer = answer_offline("Is the wash amber?", &index);
        assert_eq!(answer.standing, "Found");
        assert_eq!(answer.citations, vec![(1, "wash-note.txt".into())]);
        assert_eq!(answer.found, vec!["The wash is amber.".to_string()]);
        assert!(answer
            .found
            .iter()
            .all(|p| index.docs.iter().any(|d| d.text.contains(p.as_str()))));
        assert!(answer.text.contains("The wash is amber."));
        assert!(answer.text.contains(OFFLINE_LIMIT));
        assert!(answer.inferred.is_empty());
        assert!(answer.unknown.is_empty());
        assert!(!answer.text.contains("kettle"));
        assert!(!answer.citations.iter().any(|(_, n)| n == "kettle.txt"));
    }

    #[test]
    fn a_word_the_note_does_not_state_stays_unknown_and_uncited() {
        let index = wash_index();
        let answer = answer_offline("What colour is the wash?", &index);
        assert_eq!(answer.standing, "Found");
        assert_eq!(answer.citations, vec![(1, "wash-note.txt".into())]);
        assert_eq!(answer.found, vec!["The wash is amber.".to_string()]);
        assert!(answer
            .unknown
            .iter()
            .any(|u| u.contains("colour") && u.contains("Not in the local notes")));
        assert!(answer.text.contains("Not in the local notes: colour."));
        assert!(answer.inferred.is_empty());
        assert!(!answer.text.contains('#'));
        assert!(!answer.found.iter().any(|p| p.contains("colour")));
    }

    #[test]
    fn no_shared_word_is_unknown_with_no_citation() {
        let index = wash_index();
        let answer = answer_offline("What is the boiling point of tungsten?", &index);
        assert_eq!(answer.standing, "Unknown");
        assert!(answer.citations.is_empty());
        assert!(answer.found.is_empty());
        assert!(answer.inferred.is_empty());
        assert_eq!(answer.unknown, vec![NO_SOURCE.to_string()]);
        assert!(answer.text.starts_with("Unknown."));
        assert!(answer.text.contains(OFFLINE_LIMIT));
        assert!(!answer.text.contains("kettle"));
        assert!(!answer.text.contains("tungsten boils"));
        assert!(!answer.text.chars().any(|c| c.is_ascii_digit()));
    }

    #[test]
    fn empty_index_does_not_invent_a_source() {
        let answer = answer_offline("Where is the source?", &NotesIndex::new());
        assert_eq!(answer.standing, "Unknown");
        assert!(answer.citations.is_empty());
        assert!(answer.found.is_empty());
        assert!(answer.inferred.is_empty());
        assert!(!answer.text.contains("wash-note"));
    }

    #[test]
    fn stopwords_alone_do_not_match() {
        let index = wash_index();
        let answer = answer_offline("what is the", &index);
        assert_eq!(answer.standing, "Unknown");
        assert!(answer.citations.is_empty());
    }

    #[test]
    fn passage_is_verbatim_and_case_folds() {
        let mut index = NotesIndex::new();
        index.insert(4, "lab.txt", "The wash is amber. A kettle boils water.");
        let answer = answer_offline("Where is the KETTLE?", &index);
        assert_eq!(answer.citations, vec![(4, "lab.txt".into())]);
        assert_eq!(answer.found, vec!["A kettle boils water.".to_string()]);
        assert!(index.docs[0].text.contains("A kettle boils water."));
        assert!(!answer.found.iter().any(|p| p.contains("amber")));
    }

    #[test]
    fn forbidden_sentence_is_not_spoken_or_cited() {
        let mut index = NotesIndex::new();
        assert!(!index.insert(3, "sorry.txt", "I'm sorry for your loss."));
        let refused = answer_offline("sorry for your loss", &index);
        assert_eq!(refused.standing, "Unknown");
        assert!(refused.citations.is_empty());
        assert!(!refused.text.to_ascii_lowercase().contains("sorry"));

        assert!(index.insert(
            1,
            "wash-note.txt",
            "I'm sorry for your loss. The wash is amber."
        ));
        let answer = answer_offline("What wash is amber?", &index);
        assert_eq!(answer.found, vec!["The wash is amber.".to_string()]);
        assert!(!answer.text.to_ascii_lowercase().contains("sorry"));
        assert_eq!(answer.citations, vec![(1, "wash-note.txt".into())]);
    }

    #[test]
    fn from_sources_skips_empty_text_and_blank_names() {
        let sources = vec![
            ResearchSource {
                id: 0,
                name: "nope.txt".into(),
                text: "The wash is amber.".into(),
            },
            ResearchSource {
                id: 2,
                name: "  ".into(),
                text: "The wash is amber.".into(),
            },
            ResearchSource {
                id: 3,
                name: "shot.png".into(),
                text: String::new(),
            },
            ResearchSource {
                id: 1,
                name: "wash-note.txt".into(),
                text: "The wash is amber.".into(),
            },
        ];
        let index = NotesIndex::from_sources(&sources);
        assert_eq!(index.len(), 1);
        let answer = answer_offline("Is the wash amber?", &index);
        assert_eq!(answer.citations, vec![(1, "wash-note.txt".into())]);
        assert!(!answer.text.contains("shot.png"));
        assert!(!answer.text.contains("nope.txt"));
    }

    #[test]
    fn does_not_cite_an_id_that_was_not_inserted() {
        let mut index = NotesIndex::new();
        index.insert(7, "real.txt", "The wash is amber.");
        let answer = answer_offline("wash amber", &index);
        assert!(answer
            .citations
            .iter()
            .all(|(id, name)| *id == 7 && name == "real.txt"));
        assert!(!answer.text.contains("[1]"));
        assert!(!answer.text.contains("Invented"));
    }

    #[test]
    fn index_stops_at_the_bound() {
        let mut index = NotesIndex::new();
        for i in 1..=MAX_NOTES {
            assert!(index.insert(
                i as u32,
                &format!("n{i}.txt"),
                &format!("token{i} lives here")
            ));
        }
        assert!(!index.insert((MAX_NOTES as u32) + 1, "extra.txt", "tokenextra lives here"));
        assert_eq!(index.len(), MAX_NOTES);
        let past = answer_offline("tokenextra", &index);
        assert!(past.citations.is_empty());
    }
}
