// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Voice activity for the smoke.
//!
//! The energy gate is the fallback from `voice-stack/vad`: floor −60 dB,
//! +12 dB margin, clamp −48 dB. With the `silero` feature, the same 20 ms
//! frames also go through Silero when `silero.lock` downloads and matches.
//! A missing model stays on this gate and is not reported as Silero.

/// 20 ms at 16 kHz. Same hop as `galion-sam-vad`.
pub const FRAME_SAMPLES: usize = 320;

#[derive(Debug, Clone)]
pub struct Energy {
    floor_db: f64,
    on_db: f64,
}

impl Default for Energy {
    fn default() -> Self {
        Self {
            floor_db: -60.0,
            on_db: 12.0,
        }
    }
}

impl Energy {
    /// `true` when the frame sits above the adaptive floor.
    pub fn push_i16(&mut self, frame: &[i16]) -> bool {
        let db = rms_db(frame);
        let voiced = db > (self.floor_db + self.on_db).max(-48.0);
        if !voiced {
            self.floor_db = 0.95 * self.floor_db + 0.05 * db.max(-90.0);
        }
        voiced
    }
}

pub fn rms_db(samples: &[i16]) -> f64 {
    if samples.is_empty() {
        return -120.0;
    }
    let energy = samples
        .iter()
        .map(|sample| {
            let unit = f64::from(*sample) / 32768.0;
            unit * unit
        })
        .sum::<f64>()
        / samples.len() as f64;
    10.0 * energy.max(1e-12).log10()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameCounts {
    pub voiced: u32,
    pub quiet: u32,
}

/// Both gates on one buffer. Silero fields stay empty when that model did not run.
#[derive(Clone, Debug)]
pub struct Score {
    pub backend: &'static str,
    pub voiced: u32,
    pub quiet: u32,
    pub energy_voiced: u32,
    pub energy_quiet: u32,
    pub silero_voiced: Option<u32>,
    pub silero_quiet: Option<u32>,
    pub frame_cpu_p50_ns: Option<u64>,
    pub frame_cpu_p95_ns: Option<u64>,
    pub model_cpu_p50_ns: Option<u64>,
    pub model_cpu_p95_ns: Option<u64>,
    pub warm_cpu_ns: Option<u64>,
    pub sha256: Option<String>,
    pub fallback: Option<String>,
}

/// Energy always. Silero when the feature is on and the official model loads.
pub fn score(pcm: &[i16]) -> Score {
    let energy = count_frames(pcm);
    #[cfg(feature = "silero")]
    {
        match crate::silero::ensure_model() {
            Ok(path) => match crate::silero::score_file(pcm, &path) {
                Ok(times) => Score {
                    backend: "silero",
                    voiced: times.counts.voiced,
                    quiet: times.counts.quiet,
                    energy_voiced: energy.voiced,
                    energy_quiet: energy.quiet,
                    silero_voiced: Some(times.counts.voiced),
                    silero_quiet: Some(times.counts.quiet),
                    frame_cpu_p50_ns: times.frame_cpu_p50_ns,
                    frame_cpu_p95_ns: times.frame_cpu_p95_ns,
                    model_cpu_p50_ns: times.model_cpu_p50_ns,
                    model_cpu_p95_ns: times.model_cpu_p95_ns,
                    warm_cpu_ns: times.warm_cpu_ns,
                    sha256: Some(times.sha256),
                    fallback: None,
                },
                Err(err) => energy_score(energy, Some(err)),
            },
            Err(err) => energy_score(energy, Some(err)),
        }
    }
    #[cfg(not(feature = "silero"))]
    {
        energy_score(energy, None)
    }
}

fn energy_score(energy: FrameCounts, fallback: Option<String>) -> Score {
    Score {
        backend: "energy",
        voiced: energy.voiced,
        quiet: energy.quiet,
        energy_voiced: energy.voiced,
        energy_quiet: energy.quiet,
        silero_voiced: None,
        silero_quiet: None,
        frame_cpu_p50_ns: None,
        frame_cpu_p95_ns: None,
        model_cpu_p50_ns: None,
        model_cpu_p95_ns: None,
        warm_cpu_ns: None,
        sha256: None,
        fallback,
    }
}

/// Count 20 ms frames with the energy gate. A short tail is not scored.
pub fn count_frames(pcm: &[i16]) -> FrameCounts {
    let mut energy = Energy::default();
    let mut counts = FrameCounts {
        voiced: 0,
        quiet: 0,
    };
    for frame in pcm.chunks_exact(FRAME_SAMPLES) {
        if energy.push_i16(frame) {
            counts.voiced = counts.voiced.saturating_add(1);
        } else {
            counts.quiet = counts.quiet.saturating_add(1);
        }
    }
    counts
}

/// An 80 ms duplex frame is voiced when any of its 20 ms parts is voiced.
pub fn eighty_ms_voiced(energy: &mut Energy, frame: &[i16]) -> bool {
    frame
        .chunks_exact(FRAME_SAMPLES)
        .any(|part| energy.push_i16(part))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(n: usize, amp: i16) -> Vec<i16> {
        (0..n)
            .map(|i| ((i as f64 * 0.4).sin() * f64::from(amp)) as i16)
            .collect()
    }

    #[test]
    fn a_loud_tone_is_voice_and_silence_is_not() {
        let mut energy = Energy::default();
        assert!(energy.push_i16(&tone(FRAME_SAMPLES, 8000)));
        assert!(!energy.push_i16(&vec![0i16; FRAME_SAMPLES]));
        let counts = count_frames(&tone(FRAME_SAMPLES * 2, 8000));
        assert_eq!(counts.voiced, 2);
        assert_eq!(counts.quiet, 0);
    }
}
