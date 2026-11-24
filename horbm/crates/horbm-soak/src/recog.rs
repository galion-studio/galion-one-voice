// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Scripted recognizer. The driver sets the current words. A voiced frame
//! reveals one new word. A hold frame returns an empty list and does not
//! allocate.

use std::cell::RefCell;
use std::rc::Rc;

use horbm_duplex::{Hypothesis, HypothesisKind, RecogError, Recognizer, FRAME_MS, FRAME_SAMPLES};

#[derive(Clone)]
pub struct Scripted {
    line: Rc<RefCell<&'static [&'static str]>>,
    shown: usize,
    open: bool,
    text: String,
    frames: u32,
}

impl Scripted {
    #[must_use]
    pub fn new(line: Rc<RefCell<&'static [&'static str]>>) -> Self {
        Self {
            line,
            shown: 0,
            open: false,
            text: String::new(),
            frames: 0,
        }
    }
}

impl Recognizer for Scripted {
    fn push_frame(&mut self, pcm: &[i16]) -> Result<Vec<Hypothesis>, RecogError> {
        if pcm.len() != FRAME_SAMPLES {
            return Err(RecogError::Frame {
                got: pcm.len(),
                expected: FRAME_SAMPLES,
            });
        }
        let words = *self.line.borrow();
        if !self.open {
            self.open = true;
            self.shown = 0;
            self.text.clear();
            self.frames = 0;
        }
        self.frames = self.frames.saturating_add(1);
        if self.shown < words.len() {
            if !self.text.is_empty() {
                self.text.push(' ');
            }
            self.text.push_str(words[self.shown]);
            self.shown += 1;
            return Ok(vec![Hypothesis {
                kind: HypothesisKind::Partial,
                text: self.text.clone(),
                start_ms: 0,
                end_ms: self.frames.saturating_mul(FRAME_MS),
            }]);
        }
        Ok(Vec::new())
    }

    fn finish(&mut self) -> Result<Vec<Hypothesis>, RecogError> {
        self.open = false;
        self.shown = 0;
        self.frames = 0;
        self.text.clear();
        Ok(Vec::new())
    }

    fn cancel(&mut self) -> Result<(), RecogError> {
        self.open = false;
        self.shown = 0;
        self.frames = 0;
        self.text.clear();
        Ok(())
    }

    fn audio_ms(&self) -> u32 {
        self.frames.saturating_mul(FRAME_MS)
    }

    fn name(&self) -> &str {
        "script"
    }
}
