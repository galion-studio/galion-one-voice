// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! The capture spec. Thresholds come from the TOML; the twelve emotion names are fixed.

use serde::Deserialize;
use std::fs;
use std::path::Path;

/// Samantha's twelve emotions, in capture order. Folder slugs are the lower case.
pub const EMOTIONS: [&str; 12] = [
    "Neutral",
    "Calm",
    "Happy",
    "Amused",
    "Excited",
    "Tender",
    "Sad",
    "Disappointed",
    "Angry",
    "Fearful",
    "Surprised",
    "Disgusted",
];

pub const STANDARD: &str = "galion-voice-v2-432";
pub const CONSENT: &str = "owner-own-recordings-only";
pub const PERSONA: &str = "samantha";

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    pub standard: String,
    pub persona: String,
    pub consent: String,
    pub reference_hz: f64,
    pub format: FormatSpec,
    pub loudness: LoudnessSpec,
    pub true_peak: TruePeakSpec,
    pub noise_floor: WindowSpec,
    pub clipping: ClippingSpec,
    pub dc_offset: DcSpec,
    pub duration: SpanSpec,
    pub calibration: CalibrationSpec,
    pub reverb: ReverbSpec,
    pub spectral: SpectralSpec,
    pub takes: TakesSpec,
    pub emotions: EmotionSpec,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormatSpec {
    pub sample_rate_hz: u32,
    pub bit_depth: u16,
    pub channels: u16,
    pub sample_format: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoudnessSpec {
    pub target_lufs: f64,
    pub tolerance_lu: f64,
    pub block_ms: u32,
    pub hop_ms: u32,
    pub absolute_gate_lufs: f64,
    pub relative_gate_lu: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TruePeakSpec {
    pub max_dbtp: f64,
    pub oversample: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowSpec {
    pub max_dbfs: f64,
    pub window_ms: u32,
    pub hop_ms: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClippingSpec {
    pub max_clipped_samples: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DcSpec {
    pub max_dbfs: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpanSpec {
    pub min_s: f64,
    pub max_s: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalibrationSpec {
    pub tone_hz: f64,
    pub tolerance_hz: f64,
    pub min_s: f64,
    pub max_s: f64,
    pub min_passing: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReverbSpec {
    pub max_rt60_s: f64,
    pub fit_start_db: f64,
    pub fit_end_db: f64,
    pub frame_ms: u32,
    pub hop_ms: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpectralSpec {
    pub tilt_db_per_octave: f64,
    pub tolerance_db: f64,
    pub octave_min: i32,
    pub octave_max: i32,
    pub fft_size: usize,
    pub active_down_db: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TakesSpec {
    pub per_emotion: u32,
    pub min_per_emotion: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmotionSpec {
    pub names: Vec<String>,
}

pub fn load_spec(path: &Path) -> Result<Spec, String> {
    let text = fs::read_to_string(path)
        .map_err(|e| format!("spec: cannot read {}: {e}", path.display()))?;
    let spec: Spec = toml::from_str(&text).map_err(|e| format!("spec: {e}"))?;
    validate(&spec)?;
    Ok(spec)
}

fn validate(spec: &Spec) -> Result<(), String> {
    if spec.emotions.names != EMOTIONS {
        return Err("spec: emotions.names must be Samantha's twelve, in capture order".into());
    }
    if spec.format.channels == 0 {
        return Err("spec: channels must be at least 1".into());
    }
    if spec.loudness.tolerance_lu < 0.0
        || spec.spectral.tolerance_db < 0.0
        || spec.calibration.tolerance_hz < 0.0
    {
        return Err("spec: tolerances must be >= 0".into());
    }
    if !(spec.duration.min_s > 0.0 && spec.duration.min_s <= spec.duration.max_s) {
        return Err("spec: emotion duration window is empty".into());
    }
    if !(spec.calibration.min_s > 0.0 && spec.calibration.min_s <= spec.calibration.max_s) {
        return Err("spec: calibration duration window is empty".into());
    }
    if spec.loudness.block_ms == 0
        || spec.loudness.hop_ms == 0
        || spec.loudness.hop_ms > spec.loudness.block_ms
    {
        return Err("spec: loudness block/hop is unusable".into());
    }
    if spec.noise_floor.window_ms == 0 || spec.noise_floor.hop_ms == 0 {
        return Err("spec: noise-floor window is unusable".into());
    }
    if spec.reverb.frame_ms == 0
        || spec.reverb.hop_ms == 0
        || spec.reverb.fit_end_db >= spec.reverb.fit_start_db
    {
        return Err("spec: reverb fit is unusable".into());
    }
    if spec.true_peak.oversample != 4 {
        return Err("spec: true peak is implemented at 4× oversampling".into());
    }
    if !spec.spectral.fft_size.is_power_of_two()
        || !(1024..=65536).contains(&spec.spectral.fft_size)
    {
        return Err("spec: spectral.fft_size must be a power of two from 1024 to 65536".into());
    }
    if spec.spectral.octave_min >= spec.spectral.octave_max
        || !(-4..=6).contains(&spec.spectral.octave_min)
        || !(-4..=6).contains(&spec.spectral.octave_max)
    {
        return Err("spec: spectral octave range is unusable".into());
    }
    if spec.takes.min_per_emotion == 0 || spec.takes.min_per_emotion > spec.takes.per_emotion {
        return Err("spec: takes minimum exceeds the number recorded".into());
    }
    Ok(())
}

pub fn slug(name: &str) -> String {
    name.to_ascii_lowercase()
}
