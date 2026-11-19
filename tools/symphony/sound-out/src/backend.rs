// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! One trait for a cloner. Adapters stream 24 kHz frames. A backend that
//! only has a whole utterance still slices it, so the mixer can start on
//! the first frame.

use serde::{Deserialize, Serialize};

/// Symphony bus. 24 kHz, 80 ms, 1920 samples. Same frame as the heartbeat.
pub const SAMPLE_RATE: u32 = 24_000;
pub const SAMPLES_PER_FRAME: usize = 1_920;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Capabilities {
    pub realtime_on_cpu: bool,
    pub streaming: bool,
    pub weights_used: bool,
    pub stub: bool,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PcmChunk {
    pub index: u32,
    pub samples: Vec<f64>,
    pub last: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SpeakMeta {
    pub backend: String,
    pub weights_used: bool,
    pub stub: bool,
    pub realtime_on_cpu: bool,
    /// `galion-clone-controls:` line from the Chatterbox worker, when it ran.
    pub controls_echo: Option<String>,
    pub note: String,
}

pub trait ClonerBackend {
    fn id(&self) -> &'static str;
    fn capabilities(&self) -> Capabilities;

    fn speak_stream(
        &mut self,
        request: &crate::request::CloneRequest,
        on_chunk: &mut dyn FnMut(PcmChunk) -> Result<(), String>,
    ) -> Result<SpeakMeta, String>;

    fn speak(
        &mut self,
        request: &crate::request::CloneRequest,
    ) -> Result<(Vec<f64>, SpeakMeta), String> {
        let mut samples = Vec::new();
        let meta = self.speak_stream(request, &mut |chunk| {
            samples.extend_from_slice(&chunk.samples);
            Ok(())
        })?;
        Ok((samples, meta))
    }
}

/// Hand a buffer to `on_chunk` in 80 ms pieces. The last piece may be short.
pub fn emit_frames(
    samples: &[f64],
    on_chunk: &mut dyn FnMut(PcmChunk) -> Result<(), String>,
) -> Result<(), String> {
    if samples.is_empty() {
        return on_chunk(PcmChunk {
            index: 0,
            samples: Vec::new(),
            last: true,
        });
    }
    let mut index = 0u32;
    let mut offset = 0;
    while offset < samples.len() {
        let end = (offset + SAMPLES_PER_FRAME).min(samples.len());
        let last = end == samples.len();
        on_chunk(PcmChunk {
            index,
            samples: samples[offset..end].to_vec(),
            last,
        })?;
        offset = end;
        index = index.saturating_add(1);
    }
    Ok(())
}
