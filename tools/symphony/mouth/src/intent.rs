// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Mouth-only extras on top of `galion-mne-contract`.
//!
//! [`IntentStub::from_document`] reads the utterance text from S1.
//! Phones, smile, and precision are not on that document, so they stay
//! at the caller's values (empty, false, omitted) unless a test sets them.

use serde::{Deserialize, Serialize};

use crate::error::MouthError;

pub const CONTRACT_SCHEMA: &str = galion_mne_contract::SCHEMA;
pub const CONTRACT_TODO: &str = "Text comes from galion-mne-contract. Phones, smile, and precision are mouth extras and stay unset unless the caller fills them.";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PrecisionIntent {
    /// Moon & Lindblom 1994: clear speech reduces coarticulatory displacement.
    Clear,
    /// Lindblom 1990: hypoarticulation undershoots the target.
    Slur,
}

impl PrecisionIntent {
    pub fn as_str(self) -> &'static str {
        match self {
            PrecisionIntent::Clear => "clear",
            PrecisionIntent::Slur => "slur",
        }
    }

    pub fn parse(s: &str) -> Result<Self, MouthError> {
        match s.trim().to_ascii_lowercase().as_str() {
            "clear" | "hyper" => Ok(PrecisionIntent::Clear),
            "slur" | "hypo" => Ok(PrecisionIntent::Slur),
            _ => Err(MouthError::BadBlend(
                "precision must be clear or slur".into(),
            )),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct PhoneSpec {
    pub symbol: String,
    /// When set, this phone's grid duration in milliseconds before the rate multiplier.
    /// It is a caller duration, not a measured segment from a corpus.
    #[serde(default)]
    pub duration_ms: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct IntentStub {
    #[serde(default)]
    pub text: String,
    /// Phones the caller already has. This stub does not run grapheme-to-phoneme.
    #[serde(default)]
    pub phones: Vec<PhoneSpec>,
    /// Tartter 1980: smiling is an intent. It is not inferred from valence.
    #[serde(default)]
    pub smile: bool,
    /// Garnier et al. 2010, Experiment 2. Off unless the caller sets it.
    #[serde(default)]
    pub address_in_noise: bool,
    /// "clear", "slur", or omitted. Omitted stays an unknown precision.
    #[serde(default)]
    pub precision: Option<String>,
}

impl IntentStub {
    pub fn from_document(doc: &galion_mne_contract::Document) -> Self {
        Self {
            text: doc.text.clone(),
            phones: Vec::new(),
            smile: false,
            address_in_noise: false,
            precision: None,
        }
    }

    pub fn precision_intent(&self) -> Result<Option<PrecisionIntent>, MouthError> {
        match &self.precision {
            None => Ok(None),
            Some(s) if s.trim().is_empty() => Ok(None),
            Some(s) => PrecisionIntent::parse(s).map(Some),
        }
    }
}
