// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Word error rate.
//!
//! Normalisation matches `galion-wer`: lower case, any character outside
//! `a-z`, apostrophe, and space becomes a space, then split on whitespace.
//! The rate is the Levenshtein distance on words divided by the reference
//! word count. Whisper supplies the hypothesis. This module supplies the rate.

#[derive(Debug, Clone, PartialEq)]
pub struct Wer {
    pub rate: f64,
    pub errors: usize,
    pub substitutions: usize,
    pub deletions: usize,
    pub insertions: usize,
    pub reference_words: usize,
    pub hypothesis: String,
}

pub fn normalize(text: &str) -> Vec<String> {
    let low = text.to_lowercase();
    let mapped: String = low
        .chars()
        .map(|c| {
            if c.is_ascii_lowercase() || c == '\'' || c == ' ' {
                c
            } else {
                ' '
            }
        })
        .collect();
    mapped.split_whitespace().map(|w| w.to_string()).collect()
}

pub fn word_error_rate(reference: &str, hypothesis: &str) -> Result<Wer, String> {
    let rf = normalize(reference);
    let hy = normalize(hypothesis);
    if rf.is_empty() {
        return Err("WER reference has no words".into());
    }
    let (errors, substitutions, deletions, insertions) = align(&rf, &hy);
    Ok(Wer {
        rate: errors as f64 / rf.len() as f64,
        errors,
        substitutions,
        deletions,
        insertions,
        reference_words: rf.len(),
        hypothesis: hy.join(" "),
    })
}

fn align(rf: &[String], hy: &[String]) -> (usize, usize, usize, usize) {
    let (n, m) = (rf.len(), hy.len());
    let mut d = vec![vec![0usize; m + 1]; n + 1];
    for i in 0..=n {
        d[i][0] = i;
    }
    for j in 0..=m {
        d[0][j] = j;
    }
    for i in 1..=n {
        for j in 1..=m {
            let sub = usize::from(rf[i - 1] != hy[j - 1]);
            d[i][j] = (d[i - 1][j] + 1)
                .min(d[i][j - 1] + 1)
                .min(d[i - 1][j - 1] + sub);
        }
    }
    let (mut i, mut j) = (n, m);
    let (mut substitutions, mut deletions, mut insertions) = (0, 0, 0);
    while i > 0 || j > 0 {
        if i > 0 && j > 0 && d[i][j] == d[i - 1][j - 1] + usize::from(rf[i - 1] != hy[j - 1]) {
            substitutions += usize::from(rf[i - 1] != hy[j - 1]);
            i -= 1;
            j -= 1;
        } else if i > 0 && d[i][j] == d[i - 1][j] + 1 {
            deletions += 1;
            i -= 1;
        } else {
            insertions += 1;
            j -= 1;
        }
    }
    (d[n][m], substitutions, deletions, insertions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn punctuation_and_case_do_not_count() {
        let w = word_error_rate("Hello, world.", "hello world").unwrap();
        assert_eq!(w.rate, 0.0);
        assert_eq!(w.errors, 0);
    }

    #[test]
    fn one_substitution_in_a_hundred_words_is_under_the_gate() {
        let reference = vec!["alpha"; 100].join(" ");
        let mut words = vec!["alpha"; 100];
        words[3] = "beta".into();
        let w = word_error_rate(&reference, &words.join(" ")).unwrap();
        assert!((w.rate - 0.01).abs() < 1e-12);
        assert!(w.rate <= crate::ROLLOUT_WER_MAX);
        assert_eq!(w.substitutions, 1);
    }

    #[test]
    fn a_short_line_with_one_miss_fails_the_gate() {
        let w = word_error_rate("one two three four five", "one two three four six").unwrap();
        assert!((w.rate - 0.2).abs() < 1e-12);
        assert!(w.rate > crate::ROLLOUT_WER_MAX);
    }

    #[test]
    fn insertions_and_deletions_move_the_rate() {
        let extra = word_error_rate("alpha beta", "alpha beta gamma").unwrap();
        assert_eq!(extra.insertions, 1);
        assert!((extra.rate - 0.5).abs() < 1e-12);
        let missing = word_error_rate("alpha beta", "alpha").unwrap();
        assert_eq!(missing.deletions, 1);
        assert!((missing.rate - 0.5).abs() < 1e-12);
    }
}
