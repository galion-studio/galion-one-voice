//! Local research when Hermes does not answer.
//!
//! Okapi BM25 (`k1` 1.2, `b` 0.75) over the session's attached files and the
//! notes already stored on the desk. Std only: no socket, no crate, no model.
//!
//! A Found line is a sentence copied from an attached file. Its citation is
//! that file's id and name, and no other name. An Inferred line is a claim
//! already stored on this desk, repeated only when no attached file contains
//! it. An Unknown line is a stored gap, or the one fixed sentence used when
//! nothing local matches. The reply starts with `Offline.` The two fixed
//! sentences (the gap, and that the research path is down) are not claims
//! about the question.

use std::collections::HashMap;

use crate::hermes::{ResearchAnswer, ResearchSource};

/// One earlier desk note. Only these stored claims may be repeated.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LocalNote {
    pub found: Vec<String>,
    pub inferred: Vec<String>,
    pub unknown: Vec<String>,
}

const K1: f64 = 1.2;
const B: f64 = 0.75;
const MAX_FOUND: usize = 3;
const MAX_INFERRED: usize = 2;
const MAX_UNKNOWN: usize = 2;
const MAX_SENTENCES: usize = 40;

const OFFLINE: &str = "Offline.";
const PATH_DOWN: &str = "The research path is not reachable from here.";
const NO_LOCAL: &str = "I don't have a source for that.";

/// Sorted. A query made only of these words matches nothing.
const STOP: &[&str] = &[
    "a", "about", "am", "an", "and", "are", "as", "at", "be", "been", "being", "by", "can",
    "could", "did", "do", "does", "doing", "for", "from", "had", "has", "have", "here", "how", "i",
    "if", "im", "in", "into", "is", "it", "its", "may", "me", "might", "my", "no", "not", "of",
    "on", "or", "our", "over", "shall", "should", "than", "that", "the", "their", "them", "then",
    "there", "these", "they", "this", "those", "to", "under", "was", "we", "were", "what", "when",
    "where", "which", "who", "whom", "whose", "why", "will", "with", "would", "you", "your",
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Pile {
    Found,
    Inferred,
    Unknown,
}

struct Doc {
    text: String,
    pile: Pile,
    source_id: u32,
    source_name: String,
    tf: HashMap<String, u32>,
    len: u32,
}

/// Answer `question` from `sources` and `history`. Hermes is not called.
pub fn offline_research(
    question: &str,
    sources: &[ResearchSource],
    history: &[LocalNote],
) -> ResearchAnswer {
    let query = content_terms(question);
    let docs = if query.is_empty() {
        Vec::new()
    } else {
        index(sources, history)
    };
    let (found, inferred, mut unknown, citations) = if docs.is_empty() {
        (Vec::new(), Vec::new(), Vec::new(), Vec::new())
    } else {
        retrieve(&query, &docs, sources, history)
    };
    if found.is_empty() && inferred.is_empty() && unknown.is_empty() {
        unknown.push(NO_LOCAL.to_string());
    }
    let lead = found
        .first()
        .cloned()
        .or_else(|| inferred.first().cloned())
        .or_else(|| unknown.first().cloned())
        .unwrap_or_else(|| NO_LOCAL.to_string());
    ResearchAnswer {
        text: reply_of(&lead),
        citations,
        found,
        inferred,
        unknown,
        standing: "Offline",
    }
}

fn reply_of(lead: &str) -> String {
    let lead = lead.trim();
    if same(lead, PATH_DOWN) {
        format!("{OFFLINE} {PATH_DOWN}")
    } else {
        format!("{OFFLINE} {lead} {PATH_DOWN}")
    }
}

fn index(sources: &[ResearchSource], history: &[LocalNote]) -> Vec<Doc> {
    let mut docs = Vec::new();
    for source in sources {
        let name = source.name.trim();
        if source.id == 0 || name.is_empty() {
            continue;
        }
        for sentence in sentences(&source.text) {
            push_doc(
                &mut docs,
                sentence,
                Pile::Found,
                source.id,
                name.to_string(),
            );
        }
    }
    for note in history {
        for claim in &note.found {
            push_history(&mut docs, claim, Pile::Found, sources);
        }
        for claim in &note.inferred {
            push_history(&mut docs, claim, Pile::Inferred, sources);
        }
        for claim in &note.unknown {
            push_history(&mut docs, claim, Pile::Unknown, sources);
        }
    }
    docs
}

fn push_history(docs: &mut Vec<Doc>, claim: &str, pile: Pile, sources: &[ResearchSource]) {
    let claim = claim.trim();
    if !usable(claim) || is_boilerplate(claim) {
        return;
    }
    if let Some(source) = sources
        .iter()
        .find(|s| citable(s) && source_contains(s, claim))
    {
        push_doc(
            docs,
            claim.to_string(),
            Pile::Found,
            source.id,
            source.name.trim().to_string(),
        );
        return;
    }
    let pile = match pile {
        Pile::Unknown => Pile::Unknown,
        Pile::Found | Pile::Inferred => Pile::Inferred,
    };
    push_doc(docs, claim.to_string(), pile, 0, String::new());
}

fn push_doc(docs: &mut Vec<Doc>, text: String, pile: Pile, source_id: u32, source_name: String) {
    let key = flat(&text);
    if let Some(existing) = docs.iter_mut().find(|d| flat(&d.text) == key) {
        if pile_rank(pile) < pile_rank(existing.pile) {
            let tf = term_counts(&text);
            let len = tf.values().sum();
            if len == 0 {
                return;
            }
            existing.text = text;
            existing.pile = pile;
            existing.source_id = source_id;
            existing.source_name = source_name;
            existing.tf = tf;
            existing.len = len;
        }
        return;
    }
    let tf = term_counts(&text);
    let len = tf.values().sum();
    if len == 0 {
        return;
    }
    docs.push(Doc {
        text,
        pile,
        source_id,
        source_name,
        tf,
        len,
    });
}

type Retrieved = (Vec<String>, Vec<String>, Vec<String>, Vec<(u32, String)>);

fn retrieve(
    query: &[String],
    docs: &[Doc],
    sources: &[ResearchSource],
    history: &[LocalNote],
) -> Retrieved {
    let n = docs.len();
    let mut df: HashMap<&str, usize> = HashMap::new();
    for doc in docs {
        for term in doc.tf.keys() {
            *df.entry(term.as_str()).or_insert(0) += 1;
        }
    }
    let avgdl = docs.iter().map(|d| d.len as f64).sum::<f64>() / n as f64;
    let mut ranked: Vec<(usize, f64)> = docs
        .iter()
        .enumerate()
        .filter_map(|(i, doc)| {
            let score = bm25(doc, query, &df, n, avgdl);
            (score > 0.0).then_some((i, score))
        })
        .collect();
    ranked.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| pile_rank(docs[a.0].pile).cmp(&pile_rank(docs[b.0].pile)))
            .then(a.0.cmp(&b.0))
    });

    let mut found = Vec::new();
    let mut inferred = Vec::new();
    let mut unknown = Vec::new();
    let mut citations: Vec<(u32, String)> = Vec::new();
    let mut used: Vec<String> = Vec::new();
    for (i, _) in ranked {
        let doc = &docs[i];
        let key = flat(&doc.text);
        if used.iter().any(|k| k == &key) {
            continue;
        }
        match doc.pile {
            Pile::Found => {
                if found.len() >= MAX_FOUND {
                    continue;
                }
                let Some(source) = sources.iter().find(|s| {
                    s.id == doc.source_id
                        && s.name.trim() == doc.source_name
                        && source_contains(s, &doc.text)
                }) else {
                    continue;
                };
                if !citations.iter().any(|(id, _)| *id == source.id) {
                    citations.push((source.id, source.name.trim().to_string()));
                }
                found.push(doc.text.clone());
                used.push(key);
            }
            Pile::Inferred => {
                if inferred.len() >= MAX_INFERRED {
                    continue;
                }
                if sources.iter().any(|s| source_contains(s, &doc.text)) {
                    continue;
                }
                if !history.iter().any(|note| {
                    note.found
                        .iter()
                        .chain(note.inferred.iter())
                        .any(|c| same(c, &doc.text))
                }) {
                    continue;
                }
                inferred.push(doc.text.clone());
                used.push(key);
            }
            Pile::Unknown => {
                if unknown.len() >= MAX_UNKNOWN {
                    continue;
                }
                if is_boilerplate(&doc.text) {
                    continue;
                }
                if !history
                    .iter()
                    .any(|note| note.unknown.iter().any(|c| same(c, &doc.text)))
                {
                    continue;
                }
                unknown.push(doc.text.clone());
                used.push(key);
            }
        }
    }
    (found, inferred, unknown, citations)
}

fn bm25(doc: &Doc, query: &[String], df: &HashMap<&str, usize>, n: usize, avgdl: f64) -> f64 {
    let mut score = 0.0;
    for term in query {
        let tf = f64::from(*doc.tf.get(term).unwrap_or(&0));
        if tf == 0.0 {
            continue;
        }
        let df = *df.get(term.as_str()).unwrap_or(&0) as f64;
        let idf = ((n as f64 - df + 0.5) / (df + 0.5) + 1.0).ln();
        let denom = tf + K1 * (1.0 - B + B * (doc.len as f64 / avgdl.max(1.0)));
        score += idf * (tf * (K1 + 1.0)) / denom;
    }
    score
}

fn pile_rank(pile: Pile) -> u8 {
    match pile {
        Pile::Found => 0,
        Pile::Inferred => 1,
        Pile::Unknown => 2,
    }
}

fn citable(source: &ResearchSource) -> bool {
    source.id >= 1 && !source.name.trim().is_empty() && !source.text.trim().is_empty()
}

fn source_contains(source: &ResearchSource, sentence: &str) -> bool {
    let needle = norm_ws(sentence);
    !needle.is_empty() && norm_ws(&source.text).contains(&needle)
}

fn sentences(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut cur = String::new();
    for (i, c) in chars.iter().copied().enumerate() {
        cur.push(c);
        let next = chars.get(i + 1).copied();
        let prev = i.checked_sub(1).and_then(|p| chars.get(p)).copied();
        let end = next.map(|n| n.is_whitespace()).unwrap_or(true);
        let boundary = match c {
            '\n' | '\r' => true,
            '!' | '?' => end,
            '.' => {
                end && !(prev.is_some_and(|p| p.is_ascii_digit())
                    && next.is_some_and(|n| n.is_ascii_digit()))
            }
            _ => false,
        };
        if boundary {
            take_sentence(&mut cur, &mut out);
            if out.len() >= MAX_SENTENCES {
                return out;
            }
        }
    }
    take_sentence(&mut cur, &mut out);
    out
}

fn take_sentence(cur: &mut String, out: &mut Vec<String>) {
    let t = cur.trim().to_string();
    cur.clear();
    if usable(&t) && out.len() < MAX_SENTENCES {
        out.push(t);
    }
}

fn usable(s: &str) -> bool {
    s.trim().chars().count() >= 8 && !term_counts(s).is_empty()
}

fn is_boilerplate(s: &str) -> bool {
    const LINES: &[&str] = &[
        "offline",
        "unknown",
        "i don't have a source for that",
        "i do not have a source for that",
        "the research path is not reachable from here",
        "the research path is not reachable from here local what is already known stays marked local",
        "model can't be reached",
        "that took too long i don't have an answer yet",
        "i'll stay on what you said",
    ];
    let n = flat(s);
    LINES.iter().any(|line| n == *line)
}

fn tokens(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in s.chars() {
        if c.is_alphanumeric() {
            for l in c.to_lowercase() {
                cur.push(l);
            }
        } else if !cur.is_empty() {
            keep(&mut cur, &mut out);
        }
    }
    if !cur.is_empty() {
        keep(&mut cur, &mut out);
    }
    out
}

fn keep(cur: &mut String, out: &mut Vec<String>) {
    let tok = std::mem::take(cur);
    if tok.chars().count() >= 2 && STOP.binary_search(&tok.as_str()).is_err() {
        out.push(tok);
    }
}

fn term_counts(s: &str) -> HashMap<String, u32> {
    let mut tf = HashMap::new();
    for tok in tokens(s) {
        *tf.entry(tok).or_insert(0) += 1;
    }
    tf
}

fn content_terms(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    for tok in tokens(s) {
        if !out.iter().any(|e| e == &tok) {
            out.push(tok);
        }
    }
    out
}

fn norm_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn flat(s: &str) -> String {
    let mut out = String::new();
    let mut gap = false;
    for c in s.chars() {
        if c.is_alphanumeric() || c == '\'' {
            if gap && !out.is_empty() {
                out.push(' ');
            }
            gap = false;
            for l in c.to_lowercase() {
                out.push(l);
            }
        } else {
            gap = true;
        }
    }
    out
}

fn same(a: &str, b: &str) -> bool {
    flat(a) == flat(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn src(id: u32, name: &str, text: &str) -> ResearchSource {
        ResearchSource {
            id,
            name: name.into(),
            text: text.into(),
        }
    }

    #[test]
    fn slow_reply_is_not_stored_as_knowledge() {
        assert!(is_boilerplate(crate::hermes::PATIENT));
        assert!(is_boilerplate(crate::hermes::UNREACHABLE));
    }

    fn assert_honest(turn: &ResearchAnswer, sources: &[ResearchSource], history: &[LocalNote]) {
        assert_eq!(turn.standing, "Offline");
        assert!(turn.text.starts_with("Offline."), "{}", turn.text);
        assert!(turn.text.contains(PATH_DOWN), "{}", turn.text);
        for sentence in &turn.found {
            assert!(
                sources.iter().any(|s| source_contains(s, sentence)),
                "found is not in a file: {sentence}"
            );
        }
        for (id, name) in &turn.citations {
            let source = sources
                .iter()
                .find(|s| s.id == *id)
                .unwrap_or_else(|| panic!("cited missing id {id}"));
            assert_eq!(
                source.name.trim(),
                name.as_str(),
                "citation renamed the file"
            );
            assert!(
                turn.found.iter().any(|f| source_contains(source, f)),
                "citation {name} has no found sentence in that file"
            );
        }
        for sentence in &turn.found {
            let source = sources
                .iter()
                .find(|s| source_contains(s, sentence))
                .expect("file");
            assert!(
                turn.citations
                    .iter()
                    .any(|(id, name)| *id == source.id && name == source.name.trim()),
                "found sentence has no citation: {sentence}"
            );
        }
        for sentence in &turn.inferred {
            assert!(
                history.iter().any(|n| n
                    .found
                    .iter()
                    .chain(n.inferred.iter())
                    .any(|c| same(c, sentence))),
                "inferred was not stored: {sentence}"
            );
            assert!(
                sources.iter().all(|s| !source_contains(s, sentence)),
                "inferred is actually in a file"
            );
            assert!(turn.citations.is_empty() || turn.found.iter().all(|f| !same(f, sentence)));
        }
        for sentence in &turn.unknown {
            let canned = same(sentence, NO_LOCAL);
            let stored = history
                .iter()
                .any(|n| n.unknown.iter().any(|c| same(c, sentence)));
            assert!(canned || stored, "unknown was invented: {sentence}");
        }
    }

    #[test]
    fn stop_list_is_sorted() {
        let mut sorted = STOP.to_vec();
        sorted.sort_unstable();
        assert_eq!(STOP, sorted.as_slice());
    }

    #[test]
    fn offline_quotes_the_file_and_cites_only_that_file() {
        let sources = [
            src(2, "wash-note.txt", "The wash is amber. The hull is grey."),
            src(3, "other.txt", "The pier was closed on Tuesday."),
        ];
        let turn = offline_research("What colour is the wash?", &sources, &[]);
        assert_honest(&turn, &sources, &[]);
        assert_eq!(turn.citations, vec![(2, "wash-note.txt".into())]);
        assert_eq!(turn.found, vec!["The wash is amber.".to_string()]);
        assert!(turn.text.contains("The wash is amber."), "{}", turn.text);
        assert!(!turn.text.contains("grey"), "{}", turn.text);
        assert!(!turn.text.contains("pier"), "{}", turn.text);
        assert!(!turn.text.contains("other.txt"), "{}", turn.text);
        assert!(turn.inferred.is_empty());
    }

    #[test]
    fn offline_ranks_the_sentence_that_shares_more_terms_first() {
        let sources = [src(
            1,
            "notes.txt",
            "The wash is thin. Amber wash covers the bowl.",
        )];
        let turn = offline_research("amber wash bowl", &sources, &[]);
        assert_honest(&turn, &sources, &[]);
        assert!(turn.found[0].contains("bowl"), "{:?}", turn.found);
        assert!(turn.found[0].contains("Amber"), "{:?}", turn.found);
        assert_eq!(turn.citations, vec![(1, "notes.txt".into())]);
    }

    #[test]
    fn offline_with_nothing_local_does_not_invent() {
        let sources = [src(1, "wash-note.txt", "The wash is amber.")];
        let turn = offline_research("What is the capital of France?", &sources, &[]);
        assert_honest(&turn, &sources, &[]);
        assert!(turn.found.is_empty(), "{turn:?}");
        assert!(turn.citations.is_empty(), "{turn:?}");
        assert_eq!(turn.unknown, vec![NO_LOCAL.to_string()]);
        assert!(turn.text.contains(NO_LOCAL), "{}", turn.text);
        assert!(
            !turn.text.to_ascii_lowercase().contains("paris"),
            "{}",
            turn.text
        );
        assert!(!turn.text.contains("amber"), "{}", turn.text);
    }

    #[test]
    fn offline_does_not_cite_an_empty_image() {
        let sources = [
            src(1, "shot.png", ""),
            src(4, "wash-note.txt", "The wash is amber."),
        ];
        let turn = offline_research("What colour is the wash?", &sources, &[]);
        assert_honest(&turn, &sources, &[]);
        assert_eq!(turn.citations, vec![(4, "wash-note.txt".into())]);
        assert!(turn
            .citations
            .iter()
            .all(|(id, name)| *id != 1 && name != "shot.png"));
    }

    #[test]
    fn a_question_of_only_stopwords_matches_nothing() {
        let sources = [src(1, "wash-note.txt", "The wash is amber.")];
        let turn = offline_research("What is it?", &sources, &[]);
        assert_honest(&turn, &sources, &[]);
        assert!(turn.found.is_empty(), "{turn:?}");
        assert!(turn.citations.is_empty(), "{turn:?}");
        assert_eq!(turn.unknown, vec![NO_LOCAL.to_string()]);
    }

    #[test]
    fn history_inferred_is_repeated_without_a_citation() {
        let history = [LocalNote {
            found: Vec::new(),
            inferred: vec!["A cooler mix would read as night.".into()],
            unknown: Vec::new(),
        }];
        let turn = offline_research("Would a cooler mix read as night?", &[], &history);
        assert_honest(&turn, &[], &history);
        assert!(turn.found.is_empty(), "{turn:?}");
        assert!(turn.citations.is_empty(), "{turn:?}");
        assert_eq!(
            turn.inferred,
            vec!["A cooler mix would read as night.".to_string()]
        );
        assert!(turn.text.contains("cooler mix"), "{}", turn.text);
        assert!(!turn.text.contains(".txt"), "{}", turn.text);
    }

    #[test]
    fn history_found_without_the_file_stays_inferred_and_uncited() {
        let history = [LocalNote {
            found: vec!["The wash is amber.".into()],
            inferred: Vec::new(),
            unknown: Vec::new(),
        }];
        let turn = offline_research("What colour is the wash?", &[], &history);
        assert_honest(&turn, &[], &history);
        assert!(turn.found.is_empty(), "{turn:?}");
        assert!(turn.citations.is_empty(), "{turn:?}");
        assert_eq!(turn.inferred, vec!["The wash is amber.".to_string()]);
        assert!(!turn.text.contains("gone.txt"), "{}", turn.text);
    }

    #[test]
    fn history_found_still_in_a_file_is_cited_from_that_file() {
        let sources = [src(2, "wash-note.txt", "The wash is amber.")];
        let history = [LocalNote {
            found: vec!["The wash is amber.".into()],
            inferred: vec!["A cooler mix would read as night.".into()],
            unknown: vec!["The hex is not in the note.".into()],
        }];
        let turn = offline_research("What colour is the wash?", &sources, &history);
        assert_honest(&turn, &sources, &history);
        assert_eq!(turn.citations, vec![(2, "wash-note.txt".into())]);
        assert!(turn.found.iter().any(|s| s.contains("amber")), "{turn:?}");
        assert!(
            turn.inferred.iter().all(|s| !s.contains("cooler")),
            "{turn:?}"
        );
        assert!(!turn.text.contains("hex"), "{}", turn.text);
    }

    #[test]
    fn a_stored_unknown_is_not_promoted() {
        let history = [LocalNote {
            found: Vec::new(),
            inferred: Vec::new(),
            unknown: vec!["The hex is not in the note.".into()],
        }];
        let turn = offline_research("Which hex is in the note?", &[], &history);
        assert_honest(&turn, &[], &history);
        assert!(turn.found.is_empty());
        assert!(turn.inferred.is_empty());
        assert_eq!(
            turn.unknown,
            vec!["The hex is not in the note.".to_string()]
        );
        assert!(turn.citations.is_empty());
    }

    #[test]
    fn the_gap_line_is_not_retrieved_as_a_fact() {
        let history = [LocalNote {
            found: Vec::new(),
            inferred: Vec::new(),
            unknown: vec![NO_LOCAL.into(), PATH_DOWN.into()],
        }];
        let sources = [src(1, "wash-note.txt", "The wash is amber.")];
        let turn = offline_research("Where is the source?", &sources, &history);
        assert_honest(&turn, &sources, &history);
        assert!(turn.found.is_empty(), "{turn:?}");
        assert_eq!(turn.unknown, vec![NO_LOCAL.to_string()]);
        assert!(!turn.found.iter().any(|s| s.contains("source")), "{turn:?}");
    }

    #[test]
    fn a_decimal_stays_one_sentence() {
        let sources = [src(1, "ratio.txt", "The ratio is 3.14 in the note.")];
        let turn = offline_research("What ratio is in the note?", &sources, &[]);
        assert_honest(&turn, &sources, &[]);
        assert_eq!(
            turn.found,
            vec!["The ratio is 3.14 in the note.".to_string()]
        );
        assert_eq!(turn.citations, vec![(1, "ratio.txt".into())]);
    }
}
