// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! One-dot workflow. The page stays the circle.
//!
//! Speech is already recorded by the mic on `/` (`POST /turn`). This module
//! is what happens next: stay in the conversation, or hand the turn to a
//! program or a skill that already lives in the process. No second screen,
//! no phrase of performed care, no accelerator.

/// What the one dot does with an utterance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Next {
    /// Audio is in. Words are not in yet.
    Recorded,
    /// Stay on the voice. The persona answers.
    Speak,
    /// Launch a skill already in the process.
    Skill(&'static str),
    /// Start an automatic program already in the process.
    Program(&'static str),
}

impl Next {
    pub fn as_str(self) -> &'static str {
        match self {
            Next::Recorded => "recorded",
            Next::Speak => "speak",
            Next::Skill(name) => name,
            Next::Program(name) => name,
        }
    }
}

/// Decide from the words. Empty words stay on the voice.
/// Unknown requests stay on the voice too: the dot does not invent a tool.
pub fn next_step(text: &str) -> Next {
    if text.trim().is_empty() {
        return Next::Speak;
    }
    if has(text, "run the heartbeat") || has(text, "start the heartbeat") {
        return Next::Program("heartbeat");
    }
    if has(text, "run the demo") || has(text, "start the demo") {
        return Next::Program("live-demo");
    }
    if has(text, "check the locks") || has(text, "skill locks") {
        return Next::Skill("locks");
    }
    if has(text, "skill presence") || has(text, "run presence") {
        return Next::Skill("presence");
    }
    Next::Speak
}

fn has(hay: &str, needle: &str) -> bool {
    let h = hay.as_bytes();
    let n = needle.as_bytes();
    if n.is_empty() || h.len() < n.len() {
        return false;
    }
    h.windows(n.len()).any(|w| w.iter().zip(n.iter()).all(|(a, b)| a.to_ascii_lowercase() == *b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_pick_a_step_and_never_a_script() {
        assert_eq!(next_step(""), Next::Speak);
        assert_eq!(next_step("   "), Next::Speak);
        assert_eq!(next_step("hello"), Next::Speak);
        assert_eq!(next_step("I hear you"), Next::Speak);
        assert_eq!(next_step("that sounds so hard"), Next::Speak);
        assert_eq!(next_step("please start the heartbeat"), Next::Program("heartbeat"));
        assert_eq!(next_step("Run the demo now"), Next::Program("live-demo"));
        assert_eq!(next_step("skill presence"), Next::Skill("presence"));
        assert_eq!(next_step("check the locks"), Next::Skill("locks"));
        assert_eq!(next_step("skill something-else"), Next::Speak);
        let src = include_str!("workflow.rs");
        let code = src.split("#[cfg(test)]").next().unwrap_or(src);
        let low = code.to_ascii_lowercase();
        assert!(!low.contains("i'm here for you"));
        assert!(!low.contains("gguf"));
        assert!(!low.contains("cuda"));
    }

    #[test]
    fn one_surface_records_without_a_second_ui() {
        let page = include_str!("../assets/v1-screen.html");
        let v2 = include_str!("../assets/v2-screen.html");
        assert_eq!(page.matches("<canvas").count(), 1);
        assert!(page.contains("function postTurn"));
        assert!(page.contains("fetch(\"turn\""));
        assert!(!page.contains("id=\"say\""), "no text box: voice-to-voice only (owner t653u)");
        assert!(!page.to_ascii_lowercase().contains("<textarea"));
        assert!(v2.contains("url=/"));
        assert!(v2.contains("location.replace(\"/\")"));
        assert!(!v2.contains("function postTurn"), "/v2 does not grow a second recorder");
    }
}
