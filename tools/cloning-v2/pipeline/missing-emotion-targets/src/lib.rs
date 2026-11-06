//! Opt-in stage: detect missing universal-scorer emotions, synthesize probes,
//! score them with the unmodified scorer, and write per-emotion reference targets.
//!
//! This crate does not change scorer code, models, thresholds, or existing clones.

pub mod catalog;
pub mod detect;
pub mod invoke;
pub mod scorer;
pub mod targets;
pub mod windows;

/// Default presence floor used by the universal scorer (`universal_match.PRESENCE`).
pub const PRESENCE: f64 = 0.20;

/// Voice names that must never be written or overwritten.
pub const PROTECTED_VOICE_NAMES: &[&str] = &["own-samantha", "own-jarvis"];

pub fn is_protected_voice_name(name: &str) -> bool {
    PROTECTED_VOICE_NAMES.contains(&name) || name.contains(".bak") || name.contains(".off")
}

/// Opt-in resolution: explicit flag wins; otherwise read config; default is off.
pub fn targets_enabled(explicit: Option<bool>, config: &serde_json::Value) -> bool {
    if let Some(v) = explicit {
        return v;
    }
    config
        .get("missing_emotion_targets")
        .and_then(|v| v.get("enabled"))
        .and_then(|v| v.as_bool())
        == Some(true)
}

pub fn default_probe_voice_name(voice: &str) -> String {
    format!("emo-tgt-{voice}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn opt_in_defaults_off() {
        assert!(!targets_enabled(None, &json!({})));
        assert!(!targets_enabled(None, &json!({"missing_emotion_targets": {"enabled": false}})));
        assert!(!targets_enabled(Some(false), &json!({"missing_emotion_targets": {"enabled": true}})));
        assert!(targets_enabled(Some(true), &json!({})));
        assert!(targets_enabled(None, &json!({"missing_emotion_targets": {"enabled": true}})));
    }

    #[test]
    fn protected_names() {
        assert!(is_protected_voice_name("own-samantha"));
        assert!(is_protected_voice_name("own-jarvis"));
        assert!(is_protected_voice_name("own-samantha.bak"));
        assert!(!is_protected_voice_name("emo-tgt-own-samantha"));
        assert_eq!(default_probe_voice_name("own-samantha"), "emo-tgt-own-samantha");
    }
}
