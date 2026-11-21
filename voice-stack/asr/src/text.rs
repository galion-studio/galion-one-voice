// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

/// Drop Vosk's unknown token. An empty result stays empty.
pub fn clean_transcript(text: &str) -> String {
    text.split_whitespace()
        .filter(|w| *w != "[unk]" && *w != "<unk>")
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_unknown_tokens_only() {
        assert_eq!(clean_transcript("hello [unk] there"), "hello there");
        assert_eq!(clean_transcript("<unk>"), "");
        assert_eq!(clean_transcript("  "), "");
        assert_eq!(clean_transcript("fine"), "fine");
    }
}
