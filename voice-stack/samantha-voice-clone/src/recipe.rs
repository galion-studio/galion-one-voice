// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Owner-approved s711 recipe. The knobs do not follow the mood.
//! Mood is carried on the request so a later stage can see it. It does not
//! change exaggeration, cfg, or the words.

use crate::consent::CONSENT_SCOPE;
use crate::error::TtsError;
use crate::frames::{FRAME_SAMPLES, SAMPLE_RATE_HZ};
use crate::reference::Reference;
use crate::tts::Pad;

pub const MODE: &str = "expr";
pub const EXAGGERATION: f64 = 0.5;
pub const CFG: f64 = 0.5;
pub const SEED_BASE: u32 = 711;
pub const REF_SHA_PREFIX: &str = "10ac44dc";

#[derive(Clone, Debug, PartialEq)]
pub struct SentenceJob {
    pub index: usize,
    pub text: String,
    pub seed: u32,
    pub reference_path: String,
    pub reference_sha256: String,
    pub device: String,
    pub mood: Pad,
}

impl SentenceJob {
    pub fn to_json_line(&self) -> Result<String, TtsError> {
        let value = serde_json::json!({
            "text": self.text,
            "persona": "samantha",
            "mode": MODE,
            "model": MODE,
            "exaggeration": EXAGGERATION,
            "cfg_weight": CFG,
            "cfg": CFG,
            "seed": self.seed,
            "index": self.index,
            "audio_prompt_path": self.reference_path,
            "reference": self.reference_path,
            "ref_sha256": self.reference_sha256,
            "device": self.device,
            "raw": true,
            "effects": false,
            "mastering": false,
            "slowdown": false,
            "denoise": false,
            "chain": false,
            "consent": CONSENT_SCOPE,
            "sample_rate": SAMPLE_RATE_HZ,
            "channels": 1,
            "bits": 16,
            "frame_samples": FRAME_SAMPLES,
            "mood": {
                "arousal": self.mood.arousal,
                "valence": self.mood.valence,
                "dominance": self.mood.dominance,
            },
        });
        serde_json::to_string(&value).map_err(|err| TtsError::new("recipe", err.to_string()))
    }
}

pub fn seed_for(index: usize) -> Result<u32, TtsError> {
    let index =
        u32::try_from(index).map_err(|_| TtsError::new("recipe", "sentence index overflow"))?;
    SEED_BASE
        .checked_add(index)
        .ok_or_else(|| TtsError::new("recipe", "seed overflow"))
}

pub fn sha_prefix_ok(digest: &str) -> bool {
    let folded = digest.trim();
    if folded.len() != 64 || !folded.chars().all(|c| c.is_ascii_hexdigit()) {
        return false;
    }
    folded.to_ascii_lowercase().starts_with(REF_SHA_PREFIX)
}

/// Split on sentence punctuation. A run of `.!?` stays on the same sentence.
/// Newlines also break. This is for short spoken lines, not numbers like 3.14.
pub fn sentences(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '\n' || ch == '\r' {
            push_sentence(&mut out, &mut cur);
            i += 1;
            continue;
        }
        cur.push(ch);
        if ch == '.' || ch == '!' || ch == '?' {
            while i + 1 < chars.len() && matches!(chars[i + 1], '.' | '!' | '?') {
                i += 1;
                cur.push(chars[i]);
            }
            push_sentence(&mut out, &mut cur);
        }
        i += 1;
    }
    push_sentence(&mut out, &mut cur);
    out
}

fn push_sentence(out: &mut Vec<String>, cur: &mut String) {
    let text = cur.split_whitespace().collect::<Vec<_>>().join(" ");
    cur.clear();
    if !text.is_empty() {
        out.push(text);
    }
}

pub fn plan_sentences(
    text: &str,
    reference: &Reference,
    device: &str,
    mood: Pad,
) -> Result<Vec<SentenceJob>, TtsError> {
    if !mood.is_finite() {
        return Err(TtsError::new("mood", "mood values must be finite"));
    }
    let parts = sentences(text);
    if parts.is_empty() {
        return Err(TtsError::new("text", "empty text"));
    }
    let path = reference.path.to_string_lossy().into_owned();
    let mut jobs = Vec::with_capacity(parts.len());
    for (index, part) in parts.into_iter().enumerate() {
        if part.chars().count() > 2_000 {
            return Err(TtsError::new(
                "text",
                "sentence is longer than 2000 characters",
            ));
        }
        jobs.push(SentenceJob {
            index,
            seed: seed_for(index)?,
            text: part,
            reference_path: path.clone(),
            reference_sha256: reference.sha256.clone(),
            device: device.to_string(),
            mood,
        });
    }
    Ok(jobs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference::Reference;
    use crate::tts::Pad;
    use std::path::PathBuf;

    fn reference() -> Reference {
        Reference::trusted_for_tests(
            PathBuf::from("G:/galion/samantha.wav"),
            format!("{REF_SHA_PREFIX}{}", "ab".repeat(28)),
        )
        .unwrap()
    }

    #[test]
    fn seeds_start_at_711_and_share_the_original_reference() {
        let jobs = plan_sentences(
            "Hello. Again!",
            &reference(),
            "cpu",
            Pad {
                arousal: 1.0,
                valence: -1.0,
                dominance: 0.0,
            },
        )
        .unwrap();
        assert_eq!(jobs.len(), 2);
        assert_eq!(jobs[0].seed, 711);
        assert_eq!(jobs[1].seed, 712);
        assert_eq!(jobs[0].text, "Hello.");
        assert_eq!(jobs[1].text, "Again!");
        assert_eq!(jobs[0].reference_path, jobs[1].reference_path);
        assert_eq!(jobs[0].reference_sha256, jobs[1].reference_sha256);
        assert!(jobs[0].reference_sha256.starts_with(REF_SHA_PREFIX));
        let line = jobs[0].to_json_line().unwrap();
        let value: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(value["mode"], "expr");
        assert!((value["exaggeration"].as_f64().unwrap() - 0.5).abs() < 1e-9);
        assert!((value["cfg_weight"].as_f64().unwrap() - 0.5).abs() < 1e-9);
        assert_eq!(value["chain"], false);
        assert_eq!(value["raw"], true);
        assert_eq!(value["effects"], false);
        assert_eq!(value["mastering"], false);
        assert_eq!(value["slowdown"], false);
        assert_eq!(value["denoise"], false);
        assert_eq!(value["consent"], CONSENT_SCOPE);
        assert_eq!(value["audio_prompt_path"], value["reference"]);
        assert!((value["mood"]["arousal"].as_f64().unwrap() - 1.0).abs() < 1e-9);
        assert!(value.get("prosody").is_none());
    }

    #[test]
    fn ellipsis_stays_one_sentence_and_blank_text_fails() {
        assert_eq!(sentences("Wait..."), vec!["Wait...".to_string()]);
        assert_eq!(
            sentences("Hi!\nHow are you?"),
            vec!["Hi!".to_string(), "How are you?".to_string()]
        );
        let err = plan_sentences("   ", &reference(), "cpu", Pad::RESTING).unwrap_err();
        assert_eq!(err.kind, "text");
    }

    #[test]
    fn prefix_gate_rejects_the_cc0_pin_and_short_digests() {
        let cc0 = "e4bc60c33f697108cf5585dead213da1065979010f5be1d513574893584a4641";
        assert!(!sha_prefix_ok(cc0));
        assert!(!sha_prefix_ok(REF_SHA_PREFIX));
        assert!(!sha_prefix_ok(&format!(
            "{REF_SHA_PREFIX}zz{}",
            "ab".repeat(27)
        )));
        assert!(sha_prefix_ok(&format!(
            "{REF_SHA_PREFIX}{}",
            "AB".repeat(28)
        )));
    }
}
