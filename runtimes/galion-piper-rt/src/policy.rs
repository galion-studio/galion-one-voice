// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Recorded bounds for this process.
//!
//! Piper training on own-voice clips is forbidden. This runtime is not the
//! deployed Jarvis voice.
//!
//! This binary infers a published Piper pack (`en_US-ljspeech-medium`) out of
//! process. It does not ingest clips, fine-tune, or train. The Jarvis mouth
//! in `docs/galion-voice-v2/personas/voice/jarvis.toml` is the out-of-process
//! clone worker. Piper is listed there only as fallback, and that fallback is
//! not a film voice. Own-voice consent in this tree is local zero-shot
//! conditioning; a Piper train on those clips is outside that scope.

/// Piper training on own-voice clips is forbidden.
pub const OWN_VOICE_CLIP_TRAINING_FORBIDDEN: bool = true;

/// This runtime is not the deployed Jarvis voice.
pub const IS_DEPLOYED_JARVIS_VOICE: bool = false;

/// Published pack this process is built to infer. Not a Jarvis checkpoint.
pub const VOICE_PACK: &str = "en_US-ljspeech-medium";

/// One line, both bounds. Printed only when a train flag is refused.
pub const RECORD: &str = "Piper training on own-voice clips is forbidden. This runtime is not the deployed Jarvis voice.";

/// True for flags that would start a train or fine-tune. This binary has no such path.
pub fn is_training_flag(arg: &str) -> bool {
    matches!(arg, "--train" | "--finetune" | "--fine-tune" | "--fine_tune")
}

const _: () = assert!(OWN_VOICE_CLIP_TRAINING_FORBIDDEN);
const _: () = assert!(!IS_DEPLOYED_JARVIS_VOICE);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn piper_training_on_own_voice_clips_is_forbidden() {
        assert!(OWN_VOICE_CLIP_TRAINING_FORBIDDEN);
        assert!(RECORD.contains("Piper training on own-voice clips is forbidden"));
        assert!(is_training_flag("--train"));
        assert!(is_training_flag("--finetune"));
        assert!(is_training_flag("--fine-tune"));
        assert!(is_training_flag("--fine_tune"));
        assert!(!is_training_flag("--model"));
        assert!(!is_training_flag("--json-input"));
    }

    #[test]
    fn this_runtime_is_not_the_deployed_jarvis_voice() {
        assert!(!IS_DEPLOYED_JARVIS_VOICE);
        assert_eq!(VOICE_PACK, "en_US-ljspeech-medium");
        assert_ne!(VOICE_PACK, "jarvis");
        assert_ne!(VOICE_PACK, "own-jarvis");
        assert!(RECORD.contains("not the deployed Jarvis voice"));
    }
}
