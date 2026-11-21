// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmotionErrorKind {
    Usage,
    Denied,
    BadFrame,
    BadAudio,
    BadRate,
    Bind,
    Onnx,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmotionError {
    pub kind: EmotionErrorKind,
    pub detail: &'static str,
}

impl EmotionError {
    pub const fn new(kind: EmotionErrorKind, detail: &'static str) -> Self {
        Self { kind, detail }
    }

    pub fn exit_code(self) -> i32 {
        match self.kind {
            EmotionErrorKind::Usage => 1,
            EmotionErrorKind::Denied => 2,
            EmotionErrorKind::BadAudio | EmotionErrorKind::BadFrame | EmotionErrorKind::BadRate => {
                3
            }
            EmotionErrorKind::Bind => 4,
            EmotionErrorKind::Onnx => 5,
        }
    }
}

impl fmt::Display for EmotionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.detail)
    }
}

impl std::error::Error for EmotionError {}
