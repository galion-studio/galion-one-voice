// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

use crate::error::AsrError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipRef {
    pub id: String,
    pub text: String,
}

/// Tab-separated `id<TAB>transcript`. Blank lines and `#` comments are skipped.
pub fn parse_manifest(text: &str) -> Result<Vec<ClipRef>, AsrError> {
    let mut out = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((id, transcript)) = line.split_once('\t') else {
            return Err(AsrError::new(format!(
                "manifest line {} needs a tab between id and transcript",
                idx + 1
            )));
        };
        if id.is_empty() || transcript.is_empty() {
            return Err(AsrError::new(format!(
                "manifest line {} is incomplete",
                idx + 1
            )));
        }
        out.push(ClipRef {
            id: id.to_string(),
            text: transcript.to_string(),
        });
    }
    if out.is_empty() {
        return Err(AsrError::new("manifest has no clips"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_comments_and_keeps_the_twelve_clip_list() {
        let sample = "# SPDX-License-Identifier: MIT\n\nLJ001-0001\thello there\n";
        let clips = parse_manifest(sample).unwrap();
        assert_eq!(clips.len(), 1);
        assert_eq!(clips[0].id, "LJ001-0001");
        assert_eq!(clips[0].text, "hello there");

        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("eval/cc0-ljspeech.tsv");
        let text = std::fs::read_to_string(&path).unwrap();
        let clips = parse_manifest(&text).unwrap();
        assert_eq!(clips.len(), 12);
        assert_eq!(clips[0].id, "LJ001-0001");
        assert_eq!(clips[11].id, "LJ001-0012");
        assert!(clips[6].text.contains("fourteen fifty-five"));
    }
}
