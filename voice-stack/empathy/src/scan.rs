// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! ASCII scans. No allocation. Used by intent, wants, rewrite, and the cue list.

pub(crate) fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric()
}

pub(crate) fn boundary_before(hay: &[u8], at: usize) -> bool {
    at == 0 || !is_word_byte(hay[at - 1])
}

pub(crate) fn boundary_after(hay: &[u8], at: usize) -> bool {
    at >= hay.len() || !is_word_byte(hay[at])
}

pub(crate) fn eq_ci(hay: &[u8], needle: &[u8]) -> bool {
    hay.len() >= needle.len()
        && hay[..needle.len()]
            .iter()
            .zip(needle)
            .all(|(h, n)| h.to_ascii_lowercase() == *n)
}

/// `phrase` is matched as a whole token sequence, case-insensitive.
pub(crate) fn contains_phrase(hay: &str, phrase: &str) -> bool {
    let h = hay.as_bytes();
    let p = phrase.as_bytes();
    if p.is_empty() || h.len() < p.len() {
        return false;
    }
    let mut i = 0;
    while i + p.len() <= h.len() {
        if boundary_before(h, i) && eq_ci(&h[i..], p) && boundary_after(h, i + p.len()) {
            return true;
        }
        i += 1;
    }
    false
}

/// Contiguous `a` + `b` with a word boundary around the join.
pub(crate) fn contains_join(hay: &str, a: &str, b: &str) -> bool {
    let h = hay.as_bytes();
    let ab = a.as_bytes();
    let bb = b.as_bytes();
    let n = ab.len() + bb.len();
    if n == 0 || h.len() < n {
        return false;
    }
    let mut i = 0;
    while i + n <= h.len() {
        if boundary_before(h, i)
            && eq_ci(&h[i..], ab)
            && eq_ci(&h[i + ab.len()..], bb)
            && boundary_after(h, i + n)
        {
            return true;
        }
        i += 1;
    }
    false
}

/// Lowercase ASCII words, including an apostrophe inside the word.
/// Words longer than 24 bytes are skipped.
pub(crate) fn for_each_word(text: &str, mut f: impl FnMut(&[u8])) {
    let b = text.as_bytes();
    let mut i = 0;
    let mut buf = [0u8; 24];
    while i < b.len() {
        if !b[i].is_ascii_alphabetic() {
            i += 1;
            continue;
        }
        let mut n = 0usize;
        let mut overflow = false;
        while i < b.len() && (b[i].is_ascii_alphabetic() || b[i] == b'\'') {
            if n < buf.len() {
                buf[n] = b[i].to_ascii_lowercase();
                n += 1;
            } else {
                overflow = true;
            }
            i += 1;
        }
        if !overflow && n > 0 {
            f(&buf[..n]);
        }
    }
}
