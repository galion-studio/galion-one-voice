// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! What the live Piper mouth is allowed to say.
//!
//! The reply text is `TurnOut.reply`. This module does not rewrite it into
//! a warmer sentence. A refusal is silence plus an error on the socket.

/// Lines `docs/galion-voice-v2/personas/SAMANTHA.md` names as stock comfort.
/// Whole reply only, after trim and a trailing punctuation mark.
const SCRIPTED_COMFORT: &[&str] = &[
    "i hear you",
    "that sounds so hard",
    "i'm here for you no matter what",
    "i feel that in my soul",
];

pub fn is_scripted_comfort(reply: &str) -> bool {
    let trimmed = reply.trim().trim_end_matches(['.', '!', '?']).trim();
    let lower = trimmed.to_ascii_lowercase();
    SCRIPTED_COMFORT.iter().any(|line| lower == *line)
}

pub fn contains_bracket_tag(reply: &str) -> bool {
    let bytes = reply.as_bytes();
    let mut open = None;
    for (i, b) in bytes.iter().enumerate() {
        match b {
            b'[' => open = Some(i),
            b']' if open.is_some() => return true,
            _ => {}
        }
    }
    false
}

/// Clone worker tags from `personas/voice/samantha.toml`: `[laugh]` and
/// `[chuckle]` only. Any other bracket is rejected. No tags is fine.
pub fn clone_tags_ok(reply: &str) -> bool {
    let bytes = reply.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'[' {
            let Some(end) = bytes[i..].iter().position(|b| *b == b']') else {
                return false;
            };
            let tag = &reply[i..i + end + 1];
            if tag != "[laugh]" && tag != "[chuckle]" {
                return false;
            }
            i += end + 1;
        } else {
            i += 1;
        }
    }
    true
}

/// Live mouth. Piper must not receive bracket tags (`samantha.toml` `on_piper = false`).
/// `violations > 0` does not speak. An empty reply does not speak.
pub fn may_speak_piper(reply: &str, violations: usize) -> bool {
    violations == 0
        && !reply.trim().is_empty()
        && !is_scripted_comfort(reply)
        && !contains_bracket_tag(reply)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stock_comfort_and_empty_and_violations_stay_unspoken() {
        assert!(!may_speak_piper("I hear you.", 0));
        assert!(!may_speak_piper("  ", 0));
        assert!(!may_speak_piper("The fence is loose.", 1));
        assert!(may_speak_piper("The fence is loose.", 0));
    }

    #[test]
    fn piper_rejects_tags_the_clone_worker_may_keep() {
        assert!(!may_speak_piper("That's funny [laugh]", 0));
        assert!(clone_tags_ok("That's funny [laugh]"));
        assert!(clone_tags_ok("No tag here."));
        assert!(!clone_tags_ok("Come closer [whisper]"));
        assert!(!clone_tags_ok("half [laugh"));
    }
}
