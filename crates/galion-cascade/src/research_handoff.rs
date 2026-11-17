// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Voice-only handoff from Galion One to Galion Research.
//!
//! The voice stack is the host `voice.galion.app`. A spoken cue ("go deep",
//! "research this", "research that") lifts the rest of that utterance out as
//! the question. The host then hands that question to `POST /research/ask`
//! and speaks the reply. `galion.app`, including `/one`, is not this host:
//! the same words stay on the ordinary turn, and this module does not draw
//! anything on the One page.
//!
//! No voice model is selected here. The mouth that speaks the reply is the
//! phone lane's existing speech stack.

/// Host header of the voice stack. Port is ignored. Anything else is not.
pub const VOICE_STACK_HOST: &str = "voice.galion.app";

/// Reply persona for a handed-off turn. Not a bus id and not a new weight.
pub const RESEARCH_VOICE: &str = "galion-research";

/// Spoken when the cue carries no question. From `GALION-RESEARCH.md`.
pub const RESEARCH_GREETING: &str = "Galion Research. What are we looking into?";

/// Spoken when the research path has nothing to return. Same sentence as the desk.
pub const RESEARCH_LOCAL: &str = "The research path is not reachable from here. Local. What is already known stays marked local.";

const CUES: &[&[&str]] = &[
    &["go", "deep"],
    &["research", "this"],
    &["research", "that"],
];

const FILLERS: &[&str] = &[
    "please", "hey", "hi", "ok", "okay", "galion", "can", "you", "would", "just",
];

const LEAD_IN: &[&str] = &["on", "into", "about", "regarding"];

/// The question to hand to `/research/ask`, if this utterance is a research cue.
/// An empty question means they said only the cue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResearchCue {
    pub question: String,
}

/// `true` when `Host` is the voice stack. `galion.app` is false, with or without a port.
pub fn voice_stack_host(host: &str) -> bool {
    let host = host.split(',').next().unwrap_or("").trim();
    if host.is_empty() || host.contains([' ', '/', '\\', '@']) {
        return false;
    }
    let name = match host.rsplit_once(':') {
        Some((name, port)) if !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) => name,
        _ => host,
    };
    name.eq_ignore_ascii_case(VOICE_STACK_HOST)
}

/// Detect a research cue at the start of the utterance, after a short preamble.
/// The question is the words after the cue. `None` when this is not a handoff.
pub fn research_cue(transcript: &str) -> Option<ResearchCue> {
    let words = words_of(transcript);
    if words.is_empty() {
        return None;
    }
    let mut i = 0;
    while i < words.len() && FILLERS.contains(&words[i].as_str()) {
        i += 1;
    }
    let rest = &words[i..];
    let cue_len = CUES.iter().find_map(|cue| {
        if rest.len() >= cue.len()
            && rest[..cue.len()]
                .iter()
                .zip(cue.iter())
                .all(|(w, c)| w == c)
        {
            Some(cue.len())
        } else {
            None
        }
    })?;
    let mut q = &rest[cue_len..];
    if q.first().is_some_and(|w| LEAD_IN.contains(&w.as_str())) {
        q = &q[1..];
    }
    Some(ResearchCue {
        question: q.join(" "),
    })
}

fn words_of(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in s.chars() {
        let c = match c {
            '\u{2018}' | '\u{2019}' | '\u{02BC}' | '`' => '\'',
            other => other.to_ascii_lowercase(),
        };
        if c.is_ascii_alphanumeric() || c == '\'' {
            cur.push(c);
        } else if !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voice_host_is_only_the_voice_stack() {
        assert!(voice_stack_host("voice.galion.app"));
        assert!(voice_stack_host("VOICE.GALION.APP"));
        assert!(voice_stack_host("voice.galion.app:443"));
        assert!(voice_stack_host(" voice.galion.app:443 "));
        assert!(!voice_stack_host("galion.app"));
        assert!(!voice_stack_host("galion.app:443"));
        assert!(!voice_stack_host("www.galion.app"));
        assert!(!voice_stack_host("voice.galion.app.evil"));
        assert!(!voice_stack_host("notvoice.galion.app"));
        assert!(!voice_stack_host("voice.galion.app."));
        assert!(!voice_stack_host(""));
        assert!(!voice_stack_host("localhost"));
        assert!(!voice_stack_host("not-a-host"));
    }

    #[test]
    fn cues_lift_the_spoken_question() {
        assert_eq!(
            research_cue("go deep"),
            Some(ResearchCue {
                question: String::new()
            })
        );
        assert_eq!(
            research_cue("Go deep, what colour is the wash?"),
            Some(ResearchCue {
                question: "what colour is the wash".into()
            })
        );
        assert_eq!(
            research_cue("please research this: why the fill is locked"),
            Some(ResearchCue {
                question: "why the fill is locked".into()
            })
        );
        assert_eq!(
            research_cue("can you research that on the amber wash"),
            Some(ResearchCue {
                question: "the amber wash".into()
            })
        );
        assert_eq!(
            research_cue("would you just go deep into the source"),
            Some(ResearchCue {
                question: "the source".into()
            })
        );
    }

    #[test]
    fn ordinary_speech_is_not_a_handoff() {
        assert_eq!(research_cue("hello how are you"), None);
        assert_eq!(research_cue("go deeper"), None);
        assert_eq!(research_cue("research thistle"), None);
        assert_eq!(research_cue("I already went deep yesterday"), None);
        assert_eq!(research_cue(""), None);
        assert_eq!(research_cue("   "), None);
    }
}
