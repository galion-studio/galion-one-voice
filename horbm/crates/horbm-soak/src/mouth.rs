// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Mouth for the soak. Flite when `libflite` opens, otherwise eight frames
//! of a generated tone. Both write the utterance into the output buffer
//! and hand the duplex one prebuilt frame per pull.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use galion_ml_voice::speech::flite::FliteTts;
use galion_ml_voice::{LanguageId, TtsEngine};
use horbm_duplex::{FRAME_SAMPLES, SAMPLE_RATE};
use horbm_voice::{AudioFrame, Mouth, MouthPull, SpokenLine};

use crate::buffer::OutputBuffer;

const STUB_FRAMES: usize = 8;

enum Engine {
    Flite(FliteTts),
    Stub,
}

/// Mouth for this process. The persona is whoever the duplex loaded.
pub struct SoakMouth {
    engine: Engine,
    name: &'static str,
    pending: VecDeque<Vec<i16>>,
    pending_first: bool,
    buffer: Rc<RefCell<OutputBuffer>>,
    desync: u64,
}

impl SoakMouth {
    #[must_use]
    pub fn open(buffer: Rc<RefCell<OutputBuffer>>) -> Self {
        match FliteTts::open_default() {
            Ok(tts) => {
                let mut mouth = Self {
                    engine: Engine::Flite(tts),
                    name: "flite",
                    pending: VecDeque::new(),
                    pending_first: false,
                    buffer,
                    desync: 0,
                };
                mouth.prime();
                mouth
            }
            Err(_) => {
                let mut mouth = Self::stub(buffer);
                mouth.prime();
                mouth
            }
        }
    }

    #[must_use]
    pub fn stub(buffer: Rc<RefCell<OutputBuffer>>) -> Self {
        Self {
            engine: Engine::Stub,
            name: "native",
            pending: VecDeque::new(),
            pending_first: false,
            buffer,
            desync: 0,
        }
    }

    #[must_use]
    pub fn desync(&self) -> u64 {
        self.desync
    }

    /// Two full renders before the duplex clock starts.
    ///
    /// The first large waveform is often a fresh mapping. Freeing it raises
    /// the process malloc threshold, and the second render is the one the
    /// heap keeps. Doing both here, on a line at least as long as a soak
    /// reply, means a later utterance of the same shape does not extend the
    /// resident set after the warm sample. The samples are discarded.
    fn prime(&mut self) {
        let line = "The kettle is on the hob. The kettle is on the hob.";
        drop(self.render(line));
        drop(self.render(line));
    }

    fn render(&mut self, text: &str) -> Vec<i16> {
        match &mut self.engine {
            Engine::Flite(tts) => {
                let spoken = flite_pcm(tts, text);
                if spoken.is_empty() {
                    stub_pcm(text)
                } else {
                    spoken
                }
            }
            Engine::Stub => stub_pcm(text),
        }
    }
}

impl Mouth for SoakMouth {
    fn id(&self) -> &str {
        self.name
    }

    fn start(&mut self, line: &SpokenLine) {
        self.pending.clear();
        self.buffer.borrow_mut().clear_pending();
        let pcm = self.render(&line.text);
        let frames = split_frames(&pcm);
        let mut queued = Vec::with_capacity(frames.len().saturating_mul(FRAME_SAMPLES));
        for frame in &frames {
            queued.extend_from_slice(frame);
        }
        self.buffer.borrow_mut().enqueue(&queued);
        self.pending = VecDeque::from(frames);
        self.pending_first = true;
    }

    fn pull(&mut self) -> MouthPull {
        let Some(samples) = self.pending.pop_front() else {
            return MouthPull::Done;
        };
        let mut scratch = [0i16; FRAME_SAMPLES];
        if !self.buffer.borrow_mut().dequeue_frame(&mut scratch) {
            self.desync = self.desync.saturating_add(1);
        }
        let first = self.pending_first;
        self.pending_first = false;
        MouthPull::Frame(AudioFrame {
            samples,
            sample_rate: SAMPLE_RATE,
            first,
        })
    }

    fn cancel(&mut self) {
        self.pending.clear();
        self.pending_first = false;
        self.buffer.borrow_mut().clear_pending();
    }

    fn speaking(&self) -> bool {
        !self.pending.is_empty()
    }
}

fn split_frames(pcm: &[i16]) -> Vec<Vec<i16>> {
    let mut frames = Vec::new();
    let mut offset = 0;
    if pcm.is_empty() {
        frames.push(vec![0i16; FRAME_SAMPLES]);
        return frames;
    }
    while offset < pcm.len() {
        let mut frame = vec![0i16; FRAME_SAMPLES];
        let n = (pcm.len() - offset).min(FRAME_SAMPLES);
        frame[..n].copy_from_slice(&pcm[offset..offset + n]);
        frames.push(frame);
        offset += FRAME_SAMPLES;
    }
    frames
}

fn stub_pcm(text: &str) -> Vec<i16> {
    let mut pcm = vec![0i16; FRAME_SAMPLES * STUB_FRAMES];
    let step = 220.0 / (SAMPLE_RATE as f32);
    let mut phase = (text.len() % 5) as f32 * 0.1;
    for sample in &mut pcm {
        let x = (phase * std::f32::consts::TAU).sin();
        *sample = (x * 6000.0) as i16;
        phase = (phase + step) % 1.0;
    }
    pcm
}

fn flite_pcm(tts: &mut FliteTts, text: &str) -> Vec<i16> {
    let text = text.trim();
    if text.is_empty() {
        return Vec::new();
    }
    match tts.synthesize(text, LanguageId::En) {
        Ok(audio) => to_s16(&audio.samples, audio.sample_rate),
        Err(_) => Vec::new(),
    }
}

fn to_s16(samples: &[f32], rate: u32) -> Vec<i16> {
    if samples.is_empty() || rate == 0 {
        return Vec::new();
    }
    let dst = u64::from(SAMPLE_RATE);
    let src_rate = u64::from(rate);
    let out_len = (samples.len() as u64).saturating_mul(dst) / src_rate;
    if out_len == 0 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(out_len as usize);
    for index in 0..out_len {
        let src = (index.saturating_mul(src_rate) / dst) as usize;
        let src = src.min(samples.len() - 1);
        let sample = samples[src];
        let sample = if sample.is_finite() {
            sample.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        out.push((f64::from(sample) * 32767.0).round() as i16);
    }
    out
}
