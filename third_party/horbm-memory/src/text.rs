// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

/// Lowercase alphanumeric tokens. The query is a quoted OR of these tokens,
/// the same shape `openmind-memory` uses for its FTS5 leg, without that
/// crate's CJK dictionary. A run longer than 64 characters is dropped.
pub fn tokens(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in raw.chars() {
        if ch.is_alphanumeric() {
            for c in ch.to_lowercase() {
                cur.push(c);
            }
        } else if !cur.is_empty() {
            push_token(&mut out, &mut cur);
        }
    }
    if !cur.is_empty() {
        push_token(&mut out, &mut cur);
    }
    out
}

fn push_token(out: &mut Vec<String>, cur: &mut String) {
    if cur.len() <= 64 {
        out.push(std::mem::take(cur));
    } else {
        cur.clear();
    }
}

/// English function words. Dropped from the FTS index and from queries.
/// A term in every row has no IDF, and scoring its postings list is the
/// broad-query cost this crate has to stay under.
pub fn is_stopword(term: &str) -> bool {
    matches!(
        term,
        "a" | "an"
            | "the"
            | "and"
            | "or"
            | "of"
            | "to"
            | "in"
            | "on"
            | "for"
            | "with"
            | "at"
            | "by"
            | "from"
            | "as"
            | "is"
            | "are"
            | "was"
            | "were"
            | "be"
            | "been"
            | "being"
            | "it"
            | "its"
            | "this"
            | "that"
            | "these"
            | "those"
            | "not"
            | "but"
            | "if"
            | "into"
            | "than"
            | "then"
            | "so"
            | "such"
            | "no"
            | "nor"
            | "too"
            | "very"
            | "can"
            | "will"
            | "just"
            | "about"
            | "over"
            | "after"
            | "before"
            | "between"
            | "out"
            | "up"
            | "down"
            | "off"
            | "own"
            | "same"
            | "other"
            | "which"
            | "who"
            | "whom"
            | "what"
            | "when"
            | "where"
            | "how"
            | "all"
            | "any"
            | "both"
            | "each"
            | "few"
            | "more"
            | "most"
            | "some"
            | "only"
            | "do"
            | "does"
            | "did"
            | "doing"
            | "have"
            | "has"
            | "had"
            | "having"
    )
}

/// Tokens written into FTS. Stopwords are removed so they have no postings.
pub fn index_tokens(raw: &str) -> Vec<String> {
    tokens(raw)
        .into_iter()
        .filter(|t| !is_stopword(t))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_stopwords_before_the_index() {
        assert_eq!(index_tokens("Hello, BM25!"), vec!["hello", "bm25"]);
        assert!(index_tokens("  ,,, ").is_empty());
        assert_eq!(index_tokens("the wall"), vec!["wall"]);
    }
}
