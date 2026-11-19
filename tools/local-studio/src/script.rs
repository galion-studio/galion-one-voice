// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Line script with an emotion direction on each line.
//!
//! ```text
//! # comment
//! [calm] The meeting is on Thursday.
//! [warm pause_ms=360] I left the notes in your inbox.
//! ```

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub emotion: String,
    pub text: String,
    pub pause_after_ms: Option<u32>,
}

pub fn parse(source: &str) -> Result<Vec<Line>, String> {
    let mut lines = Vec::new();
    for (idx, raw) in source.split('\n').enumerate() {
        let raw = raw.trim_end_matches('\r').trim();
        if raw.is_empty() || raw.starts_with('#') {
            continue;
        }
        let row = idx + 1;
        let rest = raw
            .strip_prefix('[')
            .ok_or_else(|| format!("line {row}: expected [emotion] before the text"))?;
        let (bracket, text) = rest
            .split_once(']')
            .ok_or_else(|| format!("line {row}: missing ]"))?;
        let text = text.trim();
        if text.is_empty() {
            return Err(format!("line {row}: the spoken text is empty"));
        }
        if text.chars().count() > 2000 {
            return Err(format!(
                "line {row}: the spoken text is longer than 2000 characters"
            ));
        }
        let mut emotion = String::new();
        let mut pause_after_ms = None;
        for token in bracket.split_whitespace() {
            if let Some(value) = token.strip_prefix("pause_ms=") {
                let ms: u32 = value
                    .parse()
                    .map_err(|_| format!("line {row}: pause_ms is not a whole number"))?;
                if ms > 10_000 {
                    return Err(format!("line {row}: pause_ms is above 10000"));
                }
                pause_after_ms = Some(ms);
            } else if emotion.is_empty() {
                if !valid_emotion(token) {
                    return Err(format!(
                        "line {row}: emotion {token} uses letters, digits, hyphen, or underscore"
                    ));
                }
                emotion = token.to_string();
            } else {
                return Err(format!("line {row}: unknown direction {token}"));
            }
        }
        if emotion.is_empty() {
            return Err(format!("line {row}: emotion is missing"));
        }
        lines.push(Line {
            emotion,
            text: text.to_string(),
            pause_after_ms,
        });
    }
    if lines.is_empty() {
        return Err("script has no spoken lines".into());
    }
    if lines.len() > 200 {
        return Err("script has more than 200 lines".into());
    }
    Ok(lines)
}

fn valid_emotion(token: &str) -> bool {
    !token.is_empty()
        && token.len() <= 40
        && token
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_script_parses() {
        let lines = parse(include_str!("../examples/script.txt")).unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].emotion, "calm");
        assert!(lines[0].text.contains("Thursday"));
        assert_eq!(lines[1].pause_after_ms, Some(360));
    }

    #[test]
    fn comments_and_blank_lines_are_skipped() {
        let lines = parse("# note\n\n[joy] Hello there.\n").unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].emotion, "joy");
    }

    #[test]
    fn a_line_without_text_is_rejected() {
        let err = parse("[calm]\n").unwrap_err();
        assert!(err.contains("empty"));
    }
}
