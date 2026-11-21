// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Wire protocol shared with `PiperWorker` and `galion-piper-rt`.
//!
//! One JSON line `{"text":"..."}` on stdin. Raw s16le PCM on stdout.
//! After that write is flushed, stderr gets a line containing
//! `Real-time factor:`. That line is the clause boundary.

/// One JSON line. Quotes and backslashes are escaped. Controls become spaces.
pub fn json_line(text: &str) -> String {
    let mut s = String::with_capacity(text.len() + 16);
    s.push_str("{\"text\":\"");
    for c in text.chars() {
        match c {
            '"' => s.push_str("\\\""),
            '\\' => s.push_str("\\\\"),
            '\n' | '\r' | '\t' => s.push(' '),
            c if (c as u32) < 0x20 => s.push(' '),
            c => s.push(c),
        }
    }
    s.push_str("\"}\n");
    s
}

/// Piper's own real-time factor line. `factor` is infer/audio as printed.
#[derive(Clone, Debug, PartialEq)]
pub struct Rtf {
    pub factor: f32,
    pub infer_s: Option<f32>,
    pub audio_s: Option<f32>,
}

pub fn parse_rtf(line: &str) -> Option<Rtf> {
    let idx = line.find("Real-time factor:")?;
    let rest = line[idx + "Real-time factor:".len()..].trim_start();
    let num: String = rest
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
        .collect();
    if num.is_empty() {
        return None;
    }
    let factor = num.parse().ok()?;
    Some(Rtf {
        factor,
        infer_s: field(line, "infer="),
        audio_s: field(line, "audio="),
    })
}

fn field(line: &str, key: &str) -> Option<f32> {
    let i = line.find(key)?;
    let rest = &line[i + key.len()..];
    let num: String = rest
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
        .collect();
    num.parse().ok()
}

pub(crate) fn tail(s: &str) -> String {
    let t = s.trim();
    if t.len() <= 300 {
        t.to_string()
    } else {
        let mut cut = t.len() - 300;
        while cut < t.len() && !t.is_char_boundary(cut) {
            cut += 1;
        }
        t[cut..].to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_line_escapes() {
        let s = json_line("say \"hi\" \\ now");
        assert_eq!(s, "{\"text\":\"say \\\"hi\\\" \\\\ now\"}\n");
        assert!(!s[..s.len() - 1].contains('\n'));
    }

    #[test]
    fn rtf_line_parses_legacy_and_rt() {
        let a = parse_rtf("Real-time factor: 0.10 (infer=0.01 sec, audio=0.20 sec)").unwrap();
        assert!((a.factor - 0.10).abs() < 1.0e-6);
        assert_eq!(a.infer_s, Some(0.01));
        assert_eq!(a.audio_s, Some(0.20));
        let b = parse_rtf(
            "[galion-piper-rt] [info] Real-time factor: 0.25 (infer=0.02 sec, audio=0.08 sec)",
        )
        .unwrap();
        assert!((b.factor - 0.25).abs() < 1.0e-6);
        assert!(parse_rtf("loaded model").is_none());
    }
}
