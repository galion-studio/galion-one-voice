// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

/// Word error counts. `rate` is errors / reference words.
/// A rate above 1 is kept. It is not clamped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wer {
    pub reference_words: usize,
    pub substitutions: usize,
    pub deletions: usize,
    pub insertions: usize,
}

impl Wer {
    pub fn errors(&self) -> usize {
        self.substitutions + self.deletions + self.insertions
    }

    /// `None` when the reference is empty and the hypothesis is not.
    /// Both empty is 0.
    pub fn rate(&self) -> Option<f64> {
        if self.reference_words == 0 {
            if self.errors() == 0 {
                Some(0.0)
            } else {
                None
            }
        } else {
            Some(self.errors() as f64 / self.reference_words as f64)
        }
    }
}

pub fn corpus_counts(parts: &[Wer]) -> Wer {
    let mut total = Wer {
        reference_words: 0,
        substitutions: 0,
        deletions: 0,
        insertions: 0,
    };
    for w in parts {
        total.reference_words += w.reference_words;
        total.substitutions += w.substitutions;
        total.deletions += w.deletions;
        total.insertions += w.insertions;
    }
    total
}

/// Lowercase, split on anything that is not a letter, digit, or in-word apostrophe.
/// Hyphens become word breaks. Both sides of a WER comparison go through this.
pub fn normalize_words(text: &str) -> Vec<String> {
    let lower = text.to_lowercase();
    let mut words = Vec::new();
    let mut cur = String::new();
    for ch in lower.chars() {
        if ch.is_alphanumeric() || ch == '\'' {
            cur.push(ch);
        } else if !cur.is_empty() {
            push_word(&mut words, &cur);
            cur.clear();
        }
    }
    if !cur.is_empty() {
        push_word(&mut words, &cur);
    }
    words
}

fn push_word(words: &mut Vec<String>, raw: &str) {
    let trimmed = raw.trim_matches('\'');
    if !trimmed.is_empty() {
        words.push(trimmed.to_string());
    }
}

pub fn word_error(reference: &str, hypothesis: &str) -> Wer {
    let r = normalize_words(reference);
    let h = normalize_words(hypothesis);
    let n = r.len();
    let m = h.len();
    let mut dp = vec![vec![0usize; m + 1]; n + 1];
    for (i, row) in dp.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, cell) in dp[0].iter_mut().enumerate() {
        *cell = j;
    }
    for i in 1..=n {
        for j in 1..=m {
            let sub = dp[i - 1][j - 1] + usize::from(r[i - 1] != h[j - 1]);
            let del = dp[i - 1][j] + 1;
            let ins = dp[i][j - 1] + 1;
            dp[i][j] = sub.min(del).min(ins);
        }
    }
    let mut i = n;
    let mut j = m;
    let mut substitutions = 0;
    let mut deletions = 0;
    let mut insertions = 0;
    while i > 0 || j > 0 {
        if i > 0 && j > 0 && r[i - 1] == h[j - 1] && dp[i][j] == dp[i - 1][j - 1] {
            i -= 1;
            j -= 1;
        } else if i > 0 && j > 0 && dp[i][j] == dp[i - 1][j - 1] + 1 {
            substitutions += 1;
            i -= 1;
            j -= 1;
        } else if i > 0 && dp[i][j] == dp[i - 1][j] + 1 {
            deletions += 1;
            i -= 1;
        } else {
            insertions += 1;
            j -= 1;
        }
    }
    Wer {
        reference_words: n,
        substitutions,
        deletions,
        insertions,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_match_the_edits() {
        let same = word_error("Hello, world!", "hello world");
        assert_eq!(same.errors(), 0);
        assert_eq!(same.rate(), Some(0.0));

        let sub = word_error("a b c", "a x c");
        assert_eq!(sub.substitutions, 1);
        assert_eq!(sub.deletions, 0);
        assert_eq!(sub.insertions, 0);

        let del = word_error("a b c", "a c");
        assert_eq!(del.deletions, 1);
        assert_eq!(del.errors(), 1);

        let ins = word_error("a c", "a b c");
        assert_eq!(ins.insertions, 1);

        let one = word_error("a b", "b");
        assert_eq!(one.errors(), 1);
        assert_eq!(one.deletions, 1);
    }

    #[test]
    fn hyphen_and_apostrophe() {
        assert_eq!(normalize_words("Don't stop"), vec!["don't", "stop"]);
        assert_eq!(
            normalize_words("\"forty-two line Bible\""),
            vec!["forty", "two", "line", "bible"]
        );
    }

    #[test]
    fn corpus_micro_average() {
        let a = word_error("a b", "a c");
        let b = word_error("d", "d e");
        let total = corpus_counts(&[a, b]);
        assert_eq!(total.reference_words, 3);
        assert_eq!(total.errors(), 2);
        assert!((total.rate().unwrap() - (2.0 / 3.0)).abs() < 1e-12);
    }
}
