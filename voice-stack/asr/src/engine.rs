// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

use crate::error::AsrError;
use crate::event::AsrEvent;

/// Streaming speech-to-text.
///
/// Push 16 kHz mono PCM16. Partials are snapshots of the current utterance and
/// can be revised. [`Asr::finish`] closes that utterance and leaves the engine
/// ready for the next one. Keep the final text. Do not append it to the partials.
///
/// A missing model is an error. Implementations do not invent a sentence.
pub trait Asr {
    fn push_pcm(&mut self, pcm: &[i16]) -> Result<Vec<AsrEvent>, AsrError>;
    fn finish(&mut self) -> Result<Vec<AsrEvent>, AsrError>;
    fn reset(&mut self) -> Result<(), AsrError>;
    fn name(&self) -> &str;
}
