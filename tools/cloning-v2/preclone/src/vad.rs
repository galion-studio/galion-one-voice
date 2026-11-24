// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! Energy gate from `crates/galion-v2-live/src/vad.rs` and
//! `voice-stack/vad/src/energy.rs`.
//!
//! Floor starts at -60 dBFS, a frame is voice at +12 dB, and the threshold
//! never drops below -48 dBFS. Silence adapts the floor. This is the silence
//! detector. Speaker, music, and room decisions use the measured floor from
//! refcheck, because a noise bed louder than -48 dBFS is not digital silence.

pub const VAD_FRAME_MS: u32 = 20;
pub const VAD_FLOOR_DBFS: f64 = -60.0;
pub const VAD_ON_DB: f64 = 12.0;
pub const VAD_CLAMP_DBFS: f64 = -48.0;

#[derive(Clone, Debug, PartialEq)]
pub struct EnergyVad {
    pub floor_db: f64,
    pub on_db: f64,
}

impl Default for EnergyVad {
    fn default() -> Self {
        Self {
            floor_db: VAD_FLOOR_DBFS,
            on_db: VAD_ON_DB,
        }
    }
}

impl EnergyVad {
    /// `true` when the frame is above the adaptive floor. `frame` is linear PCM.
    pub fn push(&mut self, frame: &[f32]) -> bool {
        let db = rms_db(frame);
        let voiced = db > (self.floor_db + self.on_db).max(VAD_CLAMP_DBFS);
        if !voiced {
            self.floor_db = 0.95 * self.floor_db + 0.05 * db.max(-90.0);
        }
        voiced
    }
}

/// Same dB as `galion_v2_live::audio::rms_db` (mean square, 10 log10).
pub fn rms_db(x: &[f32]) -> f64 {
    if x.is_empty() {
        return -120.0;
    }
    let e = x
        .iter()
        .map(|s| {
            let v = *s as f64;
            v * v
        })
        .sum::<f64>()
        / x.len() as f64;
    10.0 * e.max(1e-12).log10()
}

#[derive(Clone, Debug, PartialEq)]
pub struct VadSummary {
    pub frame_ms: u32,
    pub on_db: f64,
    pub clamp_dbfs: f64,
    pub floor_start_dbfs: f64,
    pub voiced_frames: u32,
    pub frames: u32,
}

pub(crate) fn summarize(samples: &[f32], sr: u32) -> VadSummary {
    let n = crate::dsp::ms_samples(sr, VAD_FRAME_MS).max(1);
    let mut vad = EnergyVad::default();
    let mut frames = 0u32;
    let mut voiced_frames = 0u32;
    let mut i = 0;
    while i + n <= samples.len() {
        frames += 1;
        if vad.push(&samples[i..i + n]) {
            voiced_frames += 1;
        }
        i += n;
    }
    VadSummary {
        frame_ms: VAD_FRAME_MS,
        on_db: VAD_ON_DB,
        clamp_dbfs: VAD_CLAMP_DBFS,
        floor_start_dbfs: VAD_FLOOR_DBFS,
        voiced_frames,
        frames,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(n: usize, amp: i16) -> Vec<f32> {
        (0..n)
            .map(|i| ((i as f64 * 0.4).sin() * amp as f64) as f32 / 32768.0)
            .collect()
    }

    #[test]
    fn loud_tone_is_voice_and_does_not_lift_the_floor() {
        let mut e = EnergyVad::default();
        assert!(e.push(&tone(320, 8000)));
        assert!((e.floor_db - VAD_FLOOR_DBFS).abs() < 1e-9);
    }

    #[test]
    fn digital_silence_is_not_voice_and_the_floor_drops() {
        let mut e = EnergyVad::default();
        assert!(!e.push(&[0.0; 320]));
        assert!(e.floor_db < VAD_FLOOR_DBFS);
    }

    #[test]
    fn quiet_tone_stays_under_the_clamp() {
        let mut e = EnergyVad::default();
        assert!(!e.push(&tone(320, 40)));
    }
}
