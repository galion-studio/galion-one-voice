// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Score rules for the virtual user.
//! Task facts are the canned English lines. Interim must not claim recognition.
//! The same metric names the presence harness rejects are rejected here.

use galion_cascade::AsrSource;
use galion_presence::json::Value;

/// Names that must not appear in a report key. Same list as `galion-presence`.
pub const FORBIDDEN: &[&str] = &[
    "engagement",
    "retention",
    "session_length",
    "time_in_session",
    "dwell",
    "speech_elicited",
];

/// Canned intents the virtual user checks. The reply is the contract.
pub const TASK_FACTS: &[(&str, &str)] = &[
    ("hello", "Hi. I'm here — go ahead."),
    ("how are you", "Doing fine, thanks. What's on your mind?"),
    (
        "hello how are you",
        "Hi! I'm doing well, thanks. How about you?",
    ),
    ("can you hear me", "Yes, I can hear you."),
    ("thanks", "Any time."),
    ("zebra quantum", "Mm. Say a bit more?"),
];

pub fn forbidden_word(key: &str) -> Option<&'static str> {
    let k = key.to_ascii_lowercase();
    FORBIDDEN.iter().copied().find(|w| k.contains(w))
}

pub fn walk_keys(v: &Value) -> Result<(), String> {
    match v {
        Value::Obj(kv) => {
            for (k, child) in kv {
                if let Some(w) = forbidden_word(k) {
                    return Err(format!("forbidden metric `{k}` ({w})"));
                }
                walk_keys(child)?;
            }
            Ok(())
        }
        Value::Arr(items) => {
            for child in items {
                walk_keys(child)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Interim and text injection must not be reported as recognition.
/// A stub fixture is an exact codec decode; the report still does not call that recognition.
pub fn claims_recognition(asr: AsrSource) -> bool {
    let id = asr.id();
    if id.contains("recognition") || id.contains("recognised") {
        return true;
    }
    match asr {
        AsrSource::InterimPlaceholder | AsrSource::TextInject => asr.is_recognised(),
        AsrSource::StubFixture => false,
    }
}

pub fn obj(fields: Vec<(&str, Value)>) -> Result<Value, String> {
    let mut kv = Vec::with_capacity(fields.len());
    for (k, v) in fields {
        if let Some(w) = forbidden_word(k) {
            return Err(format!("refusing metric `{k}` ({w})"));
        }
        kv.push((k.to_string(), v));
    }
    Ok(Value::Obj(kv))
}

#[cfg(test)]
mod tests {
    use super::*;
    use galion_cascade::{CannedLlm, Emotion};

    #[test]
    fn task_facts_match_the_canned_table() {
        let llm = CannedLlm::new();
        for (text, expect) in TASK_FACTS {
            assert_eq!(llm.body(text, Emotion::Neutral), *expect, "{text}");
        }
    }

    #[test]
    fn denylist_hits_dwell_and_spares_ttfa() {
        assert_eq!(forbidden_word("user_dwell_ms"), Some("dwell"));
        assert_eq!(forbidden_word("ttfa_ms"), None);
        assert_eq!(forbidden_word("barge_in_stop_ms"), None);
        let poisoned = Value::Obj(vec![("session_length".into(), Value::Num(1.0))]);
        assert!(walk_keys(&poisoned).is_err());
    }

    #[test]
    fn interim_does_not_claim_recognition() {
        assert!(!claims_recognition(AsrSource::InterimPlaceholder));
        assert!(!AsrSource::InterimPlaceholder.is_recognised());
        assert!(!claims_recognition(AsrSource::TextInject));
        assert!(!claims_recognition(AsrSource::StubFixture));
        assert!(AsrSource::StubFixture.is_recognised());
    }
}
