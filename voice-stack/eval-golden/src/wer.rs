// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Word error rate, same normalisation and edit distance as
//! `docs/galion-voice-v2/sprint-v2/bakeoff-t648/tools/wer_t648.py` and
//! `tools/cloning-v2/scorers/quality/galion-quality/src/bin/galion-wer.rs`.
//!
//! Lowercase, map every character that is not `[a-z']` or space to a space,
//! split on whitespace. WER is Levenshtein(ref, hyp) / len(ref). An empty
//! reference has no rate. This module does not invent one.

/// Tokens the WER uses. Punctuation other than an apostrophe is dropped.
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

/// Edit distance on tokens. Insert, delete, and substitute each cost 1.
pub fn edit_distance(reference: &[String], hypothesis: &[String]) -> usize {
    let n = reference.len();
    let m = hypothesis.len();
    let mut prev: Vec<usize> = (0..=m).collect();
    let mut cur = vec![0; m + 1];
    for i in 1..=n {
        cur[0] = i;
        for j in 1..=m {
            let sub = prev[j - 1] + usize::from(reference[i - 1] != hypothesis[j - 1]);
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(sub);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[m]
}

/// `None` when the reference is empty. A rate of 0 is only a real match.
pub fn wer(reference: &str, hypothesis: &str) -> Option<f64> {
    let rf = normalize(reference);
    if rf.is_empty() {
        return None;
    }
    let hy = normalize(hypothesis);
    Some(edit_distance(&rf, &hy) as f64 / rf.len() as f64)
}

/// Mean of per-sentence rates. Empty when nothing was scored.
pub fn mean_wer(pairs: &[(String, String)]) -> Option<f64> {
    let mut sum = 0.0;
    let mut n = 0usize;
    for (reference, hypothesis) in pairs {
        if let Some(rate) = wer(reference, hypothesis) {
            sum += rate;
            n += 1;
        }
    }
    if n == 0 {
        None
    } else {
        Some(sum / n as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalisation_matches_wer_t648() {
        assert_eq!(
            normalize("Hi. I'm here!"),
            vec!["hi".to_string(), "i'm".to_string(), "here".to_string()]
        );
        assert_eq!(
            normalize("  Thursday, at 4. "),
            vec!["thursday".to_string(), "at".to_string()]
        );
    }

    #[test]
    fn distance_and_rate() {
        assert_eq!(wer("the cat sat", "the cat sat"), Some(0.0));
        assert_eq!(wer("the cat sat", "the dog sat"), Some(1.0 / 3.0));
        assert_eq!(wer("the cat sat", "cat sat"), Some(1.0 / 3.0));
        assert_eq!(wer("hello", ""), Some(1.0));
        assert_eq!(wer("...", "hello"), None);
    }
}
