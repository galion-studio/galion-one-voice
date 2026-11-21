// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Query rewrite before a lookup. "him" becomes the state tracker's name.
//! The pronoun stays when that tracker has no single answer.

use crate::names::{NameSource, PronounClass};

#[derive(Clone, Copy)]
enum Token {
    Bare(PronounClass),
    Poss(PronounClass),
    Her,
}

/// Rewrite `query` using `names`. Allocates the rewritten string. This is
/// the lookup path, not the 80 ms mood update.
pub fn rewrite_query(query: &str, names: &dyn NameSource) -> String {
    let mut out = String::with_capacity(query.len() + 8);
    let mut rest = query;
    while !rest.is_empty() {
        let Some(start) = rest.find(is_word_char) else {
            out.push_str(rest);
            break;
        };
        out.push_str(&rest[..start]);
        rest = &rest[start..];
        let end = rest.find(|c: char| !is_word_char(c)).unwrap_or(rest.len());
        let word = &rest[..end];
        rest = &rest[end..];
        if !push_replacement(&mut out, word, rest, names) {
            out.push_str(word);
        }
    }
    out
}

fn push_replacement(out: &mut String, word: &str, after: &str, names: &dyn NameSource) -> bool {
    let Some(token) = token_kind(word) else {
        return false;
    };
    let (class, possessive) = match token {
        Token::Bare(class) => (class, false),
        Token::Poss(class) => (class, true),
        Token::Her => (PronounClass::She, her_is_possessive(after)),
    };
    let Some(name) = names.name_for(class) else {
        return false;
    };
    if name.is_empty() {
        return false;
    }
    out.push_str(name);
    if possessive {
        out.push_str("'s");
    }
    true
}

fn token_kind(word: &str) -> Option<Token> {
    if word.len() > 16 || !word.is_ascii() {
        return None;
    }
    let mut buf = [0u8; 16];
    for (i, b) in word.bytes().enumerate() {
        buf[i] = b.to_ascii_lowercase();
    }
    Some(match &buf[..word.len()] {
        b"he" | b"him" => Token::Bare(PronounClass::He),
        b"his" => Token::Poss(PronounClass::He),
        b"she" => Token::Bare(PronounClass::She),
        b"hers" => Token::Poss(PronounClass::She),
        b"they" | b"them" => Token::Bare(PronounClass::They),
        b"their" | b"theirs" => Token::Poss(PronounClass::They),
        b"her" => Token::Her,
        _ => return None,
    })
}

fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '\''
}

/// "her notes" is possessive. "ask her" and "give her the file" are not.
fn her_is_possessive(after: &str) -> bool {
    let after = after.trim_start_matches(|c: char| !is_word_char(c));
    if after.is_empty() {
        return false;
    }
    let end = after
        .find(|c: char| !is_word_char(c))
        .unwrap_or(after.len());
    let next = &after[..end];
    if next.is_empty() {
        return false;
    }
    !FUNCTION.iter().any(|w| next.eq_ignore_ascii_case(w))
}

const FUNCTION: &[&str] = &[
    "a", "about", "after", "again", "also", "an", "and", "at", "before", "but", "for", "from",
    "in", "into", "just", "now", "of", "on", "or", "over", "please", "that", "the", "then",
    "these", "this", "those", "to", "too", "with",
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::names::{NameBook, PronounClass};

    #[test]
    fn him_becomes_the_focus_name_and_a_word_that_only_starts_with_him_stays() {
        let mut names = NameBook::new();
        names.note("Maciej", PronounClass::He);
        assert_eq!(
            rewrite_query("ask him about the gate", &names),
            "ask Maciej about the gate"
        );
        assert_eq!(rewrite_query("Him.", &names), "Maciej.");
        assert_eq!(rewrite_query("himalaya", &names), "himalaya");
        assert_eq!(rewrite_query("himself", &names), "himself");
        assert_eq!(rewrite_query("his notes", &names), "Maciej's notes");
    }

    #[test]
    fn no_name_leaves_the_pronoun() {
        let names = NameBook::new();
        assert_eq!(
            rewrite_query("ask him about the gate", &names),
            "ask him about the gate"
        );
    }
}
