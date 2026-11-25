// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! One line per field. A stub is the word `stub`. An off switch is `off`.
//! Neither is a timing.

#[cfg(test)]
use galion_sam_asr::word_error;

#[derive(Clone, Debug, Default)]
pub struct Report {
    rows: Vec<(String, String)>,
}

impl Report {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        let value = sanitize(&value.into());
        if let Some(row) = self.rows.iter_mut().find(|(existing, _)| existing == &key) {
            row.1 = value;
        } else {
            self.rows.push((key, value));
        }
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.rows
            .iter()
            .find(|(existing, _)| existing == key)
            .map(|(_, value)| value.as_str())
    }

    pub fn render(&self, emotion_allocs: usize) -> String {
        let mut text = String::new();
        let mut wrote_allocs = false;
        for (key, value) in &self.rows {
            text.push_str(key);
            text.push('=');
            text.push_str(value);
            text.push('\n');
            if key == "emotion" {
                text.push_str(&format!("emotion_allocs={emotion_allocs}\n"));
                wrote_allocs = true;
            }
        }
        if !wrote_allocs {
            text.push_str(&format!("emotion_allocs={emotion_allocs}\n"));
        }
        text
    }
}

fn sanitize(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            '\n' | '\r' | ' ' | '\t' => '_',
            _ => ch,
        })
        .collect()
}

/// Word error against the script. `None` means no recognizer ran, which is
/// `stub` and not a zero.
#[cfg(test)]
pub fn format_wer(reference: &str, hypothesis: Option<&str>) -> String {
    let Some(hypothesis) = hypothesis else {
        return "stub".into();
    };
    format_rate(&word_error(reference, hypothesis))
}

pub fn format_rate(wer: &galion_sam_asr::Wer) -> String {
    match wer.rate() {
        Some(rate) => format!("{rate:.4}"),
        None => "stub".into(),
    }
}

pub fn format_ms(us: u64) -> String {
    format!("{:.3}", us as f64 / 1000.0)
}
