// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! Who the speaker is. The moment step never writes this.

use crate::axes::{APPROACH, AROUSAL, AXES, DOMINANCE, VALENCE};

/// Identity of the voice at rest. Copied into the core once.
///
/// The numbers on [`VoiceBaseline::standin`] are fixture knobs for the
/// closed eval. They are not a measured person and not a recording.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VoiceBaseline {
    pub persona: [u8; 16],
    pub voice: [u8; 16],
    pub pos: [f32; AXES],
    /// Design centre in hertz. Not a measured fundamental.
    pub pitch_centre_hz: f32,
}

impl VoiceBaseline {
    /// Fixed speaker used when no stand-in directory is loaded.
    ///
    /// Pitch centre 216 Hz is the Galion One design centre in the emotion-v2
    /// foundation spec (432 × 1/2). It is not a sampled voice.
    pub const fn standin() -> Self {
        Self {
            persona: *b"standin-speaker\0",
            voice: *b"steady-rest\0\0\0\0\0",
            pos: [0.15, 0.35, 0.48, 0.05],
            pitch_centre_hz: 216.0,
        }
    }

    pub fn persona_id(&self) -> &str {
        cstr(&self.persona)
    }

    pub fn voice_id(&self) -> &str {
        cstr(&self.voice)
    }

    pub fn valence(&self) -> f32 {
        self.pos[VALENCE]
    }
    pub fn arousal(&self) -> f32 {
        self.pos[AROUSAL]
    }
    pub fn dominance(&self) -> f32 {
        self.pos[DOMINANCE]
    }
    pub fn approach(&self) -> f32 {
        self.pos[APPROACH]
    }

    /// Bit pattern of the fields the moment must not move.
    pub fn identity_bits(&self) -> BaselineBits {
        BaselineBits {
            persona: self.persona,
            voice: self.voice,
            pos: [
                self.pos[0].to_bits(),
                self.pos[1].to_bits(),
                self.pos[2].to_bits(),
                self.pos[3].to_bits(),
            ],
            pitch: self.pitch_centre_hz.to_bits(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BaselineBits {
    pub persona: [u8; 16],
    pub voice: [u8; 16],
    pub pos: [u32; AXES],
    pub pitch: u32,
}

fn cstr(bytes: &[u8; 16]) -> &str {
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    core::str::from_utf8(&bytes[..end]).unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standin_ids_fit_the_fixed_slot() {
        let b = VoiceBaseline::standin();
        assert_eq!(b.persona_id(), "standin-speaker");
        assert_eq!(b.voice_id(), "steady-rest");
        assert_eq!(b.pitch_centre_hz, 216.0);
        assert!(b.persona.len() == 16 && b.voice.len() == 16);
    }
}
