// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! What the talk is about, and which callback may be spoken later.
//!
//! A callback is stored only when remember is granted. The string is the
//! caller's text. This module does not invent one and does not write a file.

use super::filter::is_scripted_comfort;

const CALLBACK_CAP: usize = 4;
const CALLBACK_CHARS: usize = 160;

#[derive(Clone, Debug, Default)]
pub struct TopicManager {
    current: Option<String>,
    callbacks: Vec<String>,
}

impl TopicManager {
    pub fn current(&self) -> Option<&str> {
        self.current.as_deref()
    }

    pub fn callbacks(&self) -> &[String] {
        &self.callbacks
    }

    pub fn first_callback(&self) -> Option<&str> {
        self.callbacks.first().map(String::as_str)
    }

    pub fn clear_callbacks(&mut self) {
        self.callbacks.clear();
    }

    /// Remember `topic` when it changes. The previous word stays current
    /// until a new one arrives, so a switch can point back at it.
    pub fn note(&mut self, topic: String) {
        if self.current.as_deref() == Some(topic.as_str()) {
            return;
        }
        self.current = Some(topic);
    }

    /// Store a callback the person asked us to keep. False when the text is
    /// empty, too long, a stock comfort line, or not granted by the caller.
    pub fn push_callback(&mut self, text: &str) -> bool {
        let text = text.trim();
        if text.is_empty()
            || text.len() > CALLBACK_CHARS
            || text.bytes().any(|b| b == b'\n' || b == b'\r')
            || is_scripted_comfort(text)
        {
            return false;
        }
        if self.callbacks.iter().any(|kept| kept == text) {
            return true;
        }
        if self.callbacks.len() >= CALLBACK_CAP {
            return false;
        }
        self.callbacks.push(text.to_string());
        true
    }
}
