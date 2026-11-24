// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Ear text. A Vosk engine, when one opens, is the only transcript. The
//! scripted word list is the stand-in and is not a recognition result.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use galion_sam_asr::{open_engine, Asr, AsrKind, EngineOpts};
use horbm_duplex::{Hypothesis, HypothesisKind, RecogError, Recognizer, FRAME_MS, FRAME_SAMPLES};

pub fn asr_available() -> bool {
    let opts = EngineOpts::backend("vosk");
    open_engine(&opts).is_ok()
}

pub struct EarRec {
    script: VecDeque<Vec<String>>,
    words: Vec<String>,
    shown: usize,
    frames: u32,
    open: bool,
    asr: Option<Box<dyn Asr>>,
    final_text: Rc<RefCell<Option<String>>>,
}

impl EarRec {
    pub fn scripted(lines: &[&str]) -> Self {
        Self::new(lines, false)
    }

    pub fn new(lines: &[&str], asr: bool) -> Self {
        let engine = if asr {
            let opts = EngineOpts::backend("vosk");
            open_engine(&opts).ok().map(|(engine, _, _)| engine)
        } else {
            None
        };
        Self {
            script: lines
                .iter()
                .map(|line| line.split_whitespace().map(str::to_string).collect())
                .collect(),
            words: Vec::new(),
            shown: 0,
            frames: 0,
            open: false,
            asr: engine,
            final_text: Rc::new(RefCell::new(None)),
        }
    }

    pub fn using_asr(&self) -> bool {
        self.asr.is_some()
    }

    pub fn final_handle(&self) -> Rc<RefCell<Option<String>>> {
        Rc::clone(&self.final_text)
    }

    fn script_partial(&self, kind: HypothesisKind) -> Hypothesis {
        Hypothesis {
            kind,
            text: self.words[..self.shown].join(" "),
            start_ms: 0,
            end_ms: self.frames.saturating_mul(FRAME_MS),
        }
    }

    fn note_final(&self, text: &str) {
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            *self.final_text.borrow_mut() = Some(trimmed.to_string());
        }
    }
}

impl Recognizer for EarRec {
    fn push_frame(&mut self, pcm: &[i16]) -> Result<Vec<Hypothesis>, RecogError> {
        if pcm.len() != FRAME_SAMPLES {
            return Err(RecogError::Frame {
                got: pcm.len(),
                expected: FRAME_SAMPLES,
            });
        }
        self.frames = self.frames.saturating_add(1);
        if self.asr.is_some() {
            let latest = {
                let asr = self.asr.as_mut().expect("asr");
                let mut latest = String::new();
                for chunk in pcm.chunks(320) {
                    let events = asr
                        .push_pcm(chunk)
                        .map_err(|err| RecogError::Engine(err.to_string()))?;
                    if let Some(text) = events.iter().rev().find_map(|event| {
                        let text = event.text.trim();
                        if text.is_empty() {
                            None
                        } else {
                            Some(text.to_string())
                        }
                    }) {
                        latest = text;
                    }
                }
                latest
            };
            self.open = true;
            if latest.is_empty() {
                return Ok(Vec::new());
            }
            return Ok(vec![Hypothesis {
                kind: HypothesisKind::Partial,
                text: latest,
                start_ms: 0,
                end_ms: self.frames.saturating_mul(FRAME_MS),
            }]);
        }
        if !self.open {
            self.words = self.script.pop_front().unwrap_or_default();
            self.shown = 0;
            self.open = true;
        }
        if self.shown < self.words.len() {
            self.shown += 1;
            return Ok(vec![self.script_partial(HypothesisKind::Partial)]);
        }
        Ok(Vec::new())
    }

    fn finish(&mut self) -> Result<Vec<Hypothesis>, RecogError> {
        let events = match self.asr.as_mut() {
            Some(asr) => Some(
                asr.finish()
                    .map_err(|err| RecogError::Engine(err.to_string()))?,
            ),
            None => None,
        };
        if let Some(events) = events {
            self.open = false;
            if let Some(event) = events
                .iter()
                .rev()
                .find(|event| event.kind == AsrKind::Final)
            {
                self.note_final(&event.text);
                return Ok(vec![Hypothesis {
                    kind: HypothesisKind::Final,
                    text: event.text.clone(),
                    start_ms: 0,
                    end_ms: self.frames.saturating_mul(FRAME_MS),
                }]);
            }
            return Ok(Vec::new());
        }
        if !self.open || self.shown == 0 {
            self.open = false;
            self.shown = 0;
            return Ok(Vec::new());
        }
        let done = self.script_partial(HypothesisKind::Final);
        self.open = false;
        self.shown = 0;
        Ok(vec![done])
    }

    fn cancel(&mut self) -> Result<(), RecogError> {
        if let Some(asr) = &mut self.asr {
            asr.reset()
                .map_err(|err| RecogError::Engine(err.to_string()))?;
        }
        self.open = false;
        self.shown = 0;
        self.words.clear();
        Ok(())
    }

    fn audio_ms(&self) -> u32 {
        self.frames.saturating_mul(FRAME_MS)
    }

    fn name(&self) -> &str {
        if self.asr.is_some() {
            "vosk"
        } else {
            "stub"
        }
    }
}
