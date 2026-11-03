// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Letter A English ASR: Vosk out of process.
//!
//! [`VoskClient`] talks NDJSON to the `galion-asr-vosk` binary. That binary
//! dlopens libvosk. This library does not link it, does not download a model,
//! and does not invent a transcript when the model is missing.
//!
//! See `docs/asr/VOSK.md`.

mod client;
mod json;
mod protocol;

pub use client::{RecogResult, VoskClient};
pub use protocol::{
    clean_transcript, encode_error, encode_final, encode_grammar, encode_partial, encode_ready,
    encode_utterance, model_dir_ok, parse_event, parse_utterance, vosk_text, vosk_words, Utterance,
    Word, WorkerEvent, DEFAULT_LIB, DEFAULT_MODEL_DIR, FRAME_SAMPLES, MAX_UTTERANCE_SAMPLES,
    PROTOCOL, SAMPLE_RATE,
};

#[cfg(test)]
mod tests {
    #[test]
    fn live_screen_does_not_default_to_interim() {
        let s = include_str!("../../../scripts/run-v1-screen.sh");
        assert!(
            !s.contains("${GALION_ASR:-interim}"),
            "live screen must not default to the length-bucket placeholder"
        );
        assert!(s.contains("export GALION_ASR=vosk"), "{s}");
        assert!(s.contains("refusing GALION_ASR=interim"), "{s}");
        assert!(s.contains("docs/asr/VOSK.md"), "{s}");
    }
}
