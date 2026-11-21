// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

use std::fmt;

/// A recognizer failure. The message is the failure. It is not a transcript.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsrError {
    message: String,
}

impl AsrError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for AsrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for AsrError {}
