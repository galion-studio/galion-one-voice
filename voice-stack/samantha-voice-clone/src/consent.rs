// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Local zero-shot conditioning only. This crate does not fine-tune, upload,
//! or send the reference anywhere.

use crate::error::TtsError;

pub const CONSENT_SCOPE: &str = "local-zero-shot-conditioning";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Consent {
    scope: &'static str,
}

impl Consent {
    pub fn local_zero_shot() -> Self {
        Self {
            scope: CONSENT_SCOPE,
        }
    }

    pub fn scope(self) -> &'static str {
        self.scope
    }

    pub fn parse(scope: &str) -> Result<Self, TtsError> {
        if scope == CONSENT_SCOPE {
            Ok(Self::local_zero_shot())
        } else {
            Err(TtsError::new(
                "consent",
                format!("{scope} is outside local zero-shot conditioning"),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_local_zero_shot_scope_parses() {
        assert_eq!(
            Consent::parse(CONSENT_SCOPE).unwrap().scope(),
            CONSENT_SCOPE
        );
        assert!(Consent::parse("fine-tune").is_err());
        assert!(Consent::parse("upload").is_err());
        assert!(Consent::parse("").is_err());
    }
}
