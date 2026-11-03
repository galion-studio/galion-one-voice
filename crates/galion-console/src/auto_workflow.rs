// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! One-dot auto-record proceed.
//!
//! The `/` page already cuts speech (VAD) and posts the WAV to `POST /turn`.
//! This module names what that utterance does next. It does not open a page,
//! draw a mark, spawn a process, or run a model.

/// Header the one-dot page sends on `POST /turn`.
pub const PROCEED_HEADER: &str = "X-Galion-Proceed";
/// Header value. It marks the auto-record proceed. It is not a skill name.
pub const PROCEED_AUTO: &str = "auto";
/// Program that runs when the mic take names no skill. The phone reply lane.
pub const AUTO_PROGRAM: &str = "phone-turn";

/// Where the utterance came from. The workflow has one source: the `/` mic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProceedSource {
    /// `POST /turn` from the one-dot page.
    Mic,
}

/// What runs after the take ends.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Proceed {
    /// Named skill slot. The voice lane still speaks. No window opens.
    Skill(String),
    /// Default program: the phone reply lane on this same turn.
    Program,
}

/// Decide the proceed for one finished mic take.
///
/// `skill_hint` is an already-known token from a later owner program.
/// The page does not collect one. `None`, empty, `auto`, and anything that
/// is not a single token become [`Proceed::Program`].
pub fn proceed(source: ProceedSource, skill_hint: Option<&str>) -> Proceed {
    match source {
        ProceedSource::Mic => match skill_token(skill_hint) {
            Some(name) => Proceed::Skill(name),
            None => Proceed::Program,
        },
    }
}

/// `true` when `value` is the auto-record header, not a skill name.
pub fn is_auto_mark(value: Option<&str>) -> bool {
    matches!(value.map(str::trim), Some(PROCEED_AUTO))
}

fn skill_token(hint: Option<&str>) -> Option<String> {
    let raw = hint?.trim();
    if raw.is_empty() || raw.len() > 32 || raw == PROCEED_AUTO || raw == "program" {
        return None;
    }
    let mut chars = raw.chars();
    let first = chars.next()?;
    if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
        return None;
    }
    if raw.ends_with('-') || raw.contains("--") {
        return None;
    }
    if !chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
        return None;
    }
    Some(raw.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mic_without_a_hint_is_the_auto_program() {
        assert_eq!(proceed(ProceedSource::Mic, None), Proceed::Program);
        assert_eq!(proceed(ProceedSource::Mic, Some("")), Proceed::Program);
        assert_eq!(proceed(ProceedSource::Mic, Some("  ")), Proceed::Program);
        assert_eq!(proceed(ProceedSource::Mic, Some("auto")), Proceed::Program);
        assert_eq!(
            proceed(ProceedSource::Mic, Some("program")),
            Proceed::Program
        );
        assert!(is_auto_mark(Some("auto")));
        assert!(is_auto_mark(Some(" auto ")));
        assert!(!is_auto_mark(Some("open-mind")));
        assert!(!is_auto_mark(None));
    }

    #[test]
    fn a_token_launches_a_skill_and_junk_does_not() {
        assert_eq!(
            proceed(ProceedSource::Mic, Some("open-mind")),
            Proceed::Skill("open-mind".into())
        );
        assert_eq!(
            proceed(ProceedSource::Mic, Some(" jarvis ")),
            Proceed::Skill("jarvis".into())
        );
        assert_eq!(
            proceed(ProceedSource::Mic, Some("samantha")),
            Proceed::Skill("samantha".into())
        );
        for bad in [
            "Open-Mind",
            "a/b",
            "../etc",
            "rm -rf",
            "a--b",
            "-skill",
            "skill-",
            "has space",
            "a_b",
            "https://x",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ] {
            assert_eq!(
                proceed(ProceedSource::Mic, Some(bad)),
                Proceed::Program,
                "{bad}"
            );
        }
    }

    #[test]
    fn the_page_proceeds_on_the_mic_path_and_draws_no_second_ui() {
        const PAGE: &str = include_str!("../assets/v1-screen.html");
        assert!(PAGE.contains("function proceedTurn"));
        assert!(PAGE.contains("if(ok) proceedTurn(frames,barge);"));
        assert!(PAGE.contains("X-Galion-Proceed"));
        assert!(PAGE.contains("\"auto\""));
        assert!(PAGE.contains("fetch(\"turn\""));
        assert!(!PAGE.contains("id=\"workflow\""));
        assert!(!PAGE.contains("id=\"chat\""));
        assert!(!PAGE.contains("id=\"skills\""));
        assert!(!PAGE.contains("id=\"agent\""));
        assert_eq!(PROCEED_HEADER, "X-Galion-Proceed");
        assert_eq!(PROCEED_AUTO, "auto");
        assert_eq!(AUTO_PROGRAM, "phone-turn");
    }
}
