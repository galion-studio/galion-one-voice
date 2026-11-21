// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TtsError {
    pub kind: &'static str,
    pub detail: String,
}

impl TtsError {
    pub fn new(kind: &'static str, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }
}

impl fmt::Display for TtsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.kind, self.detail)
    }
}

impl std::error::Error for TtsError {}
