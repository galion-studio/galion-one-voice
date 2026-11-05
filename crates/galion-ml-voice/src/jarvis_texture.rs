// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Jarvis hub texture as a separate toggleable layer.
//!
//! Clicks, alarms, and virtual ambience are a layer beside the speech
//! asset. [`apply_hub_toggle`] mixes that layer on or off. Off copies the
//! asset. It does not denoise it, and it does not write either buffer.
//!
//! [`eval_hub_toggle`] checks energy bands against the clean-speech
//! baseline. The rule is `docs/galion-voice-v2/symphony/JARVIS-TEXTURE.md`.
//! Nothing here opens a wav.

/// High-pass used to isolate click transients, in hertz.
pub const CLICK_HIGHPASS_HZ: f32 = 6_000.0;
/// Envelope follower on the click residual, in hertz.
pub const CLICK_ENVELOPE_HZ: f32 = 30.0;
/// A sample is a click when the residual exceeds the envelope by this factor.
pub const CLICK_RATIO: f32 = 8.0;
/// Absolute residual floor so a quiet tone onset is not counted as a click.
pub const CLICK_FLOOR: f32 = 0.02;
/// Alarm band centre, in hertz.
pub const ALARM_HZ: f32 = 3_200.0;
/// Alarm band Q. Bandwidth is `ALARM_HZ / ALARM_Q`.
pub const ALARM_Q: f32 = 8.0;
/// Low-pass that holds the virtual-ambience bed, in hertz.
pub const AMBIENCE_LOWPASS_HZ: f32 = 250.0;
/// Frame length for the ambience bed, in milliseconds.
pub const FRAME_MS: u32 = 20;
/// Quiet-frame cutoff. Frames at or below this percentile of frame RMS carry the bed.
pub const AMBIENCE_PERCENTILE: f64 = 0.40;
/// On-mix band energy must clear the baseline by this fraction of the layer band.
pub const KEEP_RATIO: f64 = 0.70;
/// Each hub character must be at least this share of the layer's total energy.
pub const LAYER_SHARE_MIN: f64 = 0.01;
/// Off-state band energy may differ from the baseline by this fraction of the larger band.
pub const OFF_BAND_SLACK: f64 = 0.02;
/// Off-state PCM must match the speech asset within this absolute sample slack.
pub const OFF_SAMPLE_SLACK: f32 = 1e-5;
/// Below this rate the click band and the alarm band do not both fit.
pub const MIN_SAMPLE_RATE: u32 = 16_000;
/// Shorter than this, the band check is not a measurement.
pub const MIN_DURATION_SECS: f32 = 0.4;
/// Wire field. Absent means on.
pub const TOGGLE_KEY: &str = "hub_texture";
/// The only persona this layer mixes.
pub const TOGGLE_PERSONA: &str = "jarvis";

/// Normalised token and the class of cleaner it names.
/// A cleaner is not an off switch. The markdown forbid-list reprints this table.
pub static FORBIDDEN_CLEAN: &[(&str, &str)] = &[
    ("deepfilternet", "neural denoise"),
    ("deepfilter", "neural denoise"),
    ("rnnoise", "neural denoise"),
    ("facebookdenoiser", "neural denoise"),
    ("resembleenhance", "enhance"),
    ("podcastenhance", "enhance"),
    ("dialoguenhance", "enhance"),
    ("speechenhance", "enhance"),
    ("voiceenhance", "enhance"),
    ("enhancevoice", "enhance"),
    ("denoiser", "denoise"),
    ("denoise", "denoise"),
    ("noisereduce", "spectral denoise"),
    ("noisereduction", "spectral denoise"),
    ("noisered", "spectral denoise"),
    ("spectralsubtraction", "spectral denoise"),
    ("spectralsubtract", "spectral denoise"),
    ("spectralgate", "spectral gate"),
    ("wiener", "spectral denoise"),
    ("afftdn", "ffmpeg denoise"),
    ("anlmdn", "ffmpeg denoise"),
    ("arnndn", "ffmpeg denoise"),
    ("auphonic", "hosted clean"),
    ("krisp", "hosted denoise"),
    ("izotope", "restoration"),
    ("audiorestoration", "restoration"),
    ("declick", "click removal"),
    ("decrackle", "crackle removal"),
    ("dehum", "hum removal"),
    ("dereverb", "room removal"),
    ("deroom", "room removal"),
    ("voiceisolation", "voice isolation"),
    ("vocalisolation", "voice isolation"),
    ("isolatevoice", "voice isolation"),
    ("isolatevocal", "voice isolation"),
    ("stemseparat", "source separation"),
    ("demucs", "source separation"),
    ("spleeter", "source separation"),
    ("noisegate", "noise gate"),
    ("noisegating", "noise gate"),
    ("expandergate", "noise gate"),
    ("stripsilence", "silence strip"),
    ("silenceremove", "silence strip"),
    ("removesilence", "silence strip"),
    ("vadtrim", "silence strip"),
    ("removenoise", "noise removal"),
    ("removebackground", "background removal"),
    ("backgroundremoval", "background removal"),
    ("cleanreference", "clean prep"),
    ("cleanaudio", "clean prep"),
    ("cleanvoice", "clean prep"),
    ("audioclean", "clean prep"),
    ("highpass", "band cut"),
    ("lowpass", "band cut"),
    ("highcut", "band cut"),
    ("lowcut", "band cut"),
];

/// Preparation that is not a clean. A name that also contains a forbidden
/// token stays forbidden. Unknown names still face the energy-band check.
pub static ALLOWED_PREP: &[&str] = &[
    "downmix",
    "mono",
    "dcblock",
    "dcblocker",
    "gain",
    "peaknormalize",
    "peaknorm",
    "normalize",
    "resample",
    "sincresample",
];

/// On mixes the hub layer. Off returns the speech asset.
/// Default is on: a missing flag is not a clean.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextureToggle {
    pub enabled: bool,
}

impl Default for TextureToggle {
    fn default() -> Self {
        Self { enabled: true }
    }
}

impl TextureToggle {
    /// `None` stays on.
    pub fn from_flag(enabled: Option<bool>) -> Self {
        Self {
            enabled: enabled.unwrap_or(true),
        }
    }
}

/// Letters and digits only, ASCII lower case.
pub fn normalize_stage(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Longest forbidden token contained in `name`, if any.
pub fn match_forbidden_stage(name: &str) -> Option<(&'static str, &'static str)> {
    let n = normalize_stage(name);
    if n.is_empty() {
        return None;
    }
    let mut best: Option<(&'static str, &'static str)> = None;
    let mut best_len = 0usize;
    for &(token, class) in FORBIDDEN_CLEAN {
        if token.len() > best_len && n.contains(token) {
            best = Some((token, class));
            best_len = token.len();
        }
    }
    best
}

/// Parse one toggle line.
///
/// ```text
/// {"persona":"jarvis","hub_texture":true}
/// {"persona":"jarvis","hub_texture":false}
/// ```
///
/// A missing `hub_texture` field is on. A denoise or clean key is an error.
/// `persona`, when present, is `jarvis`.
pub fn parse_toggle_line(line: &str) -> Result<TextureToggle, TextureFailure> {
    let trimmed = line.trim();
    if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
        return Err(TextureFailure::ToggleLine);
    }
    if let Some((token, class)) = match_forbidden_stage(trimmed) {
        return Err(TextureFailure::ForbiddenStage {
            name: trimmed.to_string(),
            token,
            class,
        });
    }
    if let Some(persona) = json_string_field(trimmed, "persona")? {
        if persona != TOGGLE_PERSONA {
            return Err(TextureFailure::PersonaMismatch);
        }
    }
    let enabled = json_bool_field(trimmed, TOGGLE_KEY)?.unwrap_or(true);
    Ok(TextureToggle { enabled })
}

/// Mix the hub layer. Reads both slices. Writes neither.
///
/// Off is `asset.to_vec()`. On is `asset[i] + layer[i]`. Lengths and rates match.
pub fn apply_hub_toggle(
    asset: &[f32],
    asset_rate: u32,
    layer: &[f32],
    layer_rate: u32,
    toggle: TextureToggle,
) -> Result<Vec<f32>, TextureFailure> {
    if asset_rate < MIN_SAMPLE_RATE {
        return Err(TextureFailure::RateTooLow {
            sample_rate: asset_rate,
        });
    }
    if asset_rate != layer_rate {
        return Err(TextureFailure::RateMismatch {
            reference: asset_rate,
            candidate: layer_rate,
        });
    }
    if asset.is_empty() || layer.is_empty() {
        return Err(TextureFailure::TooShort {
            which: "mix",
            samples: asset.len(),
        });
    }
    if asset.len() != layer.len() {
        return Err(TextureFailure::DurationMismatch {
            reference: asset.len(),
            candidate: layer.len(),
        });
    }
    if asset.iter().any(|s| !s.is_finite()) || layer.iter().any(|s| !s.is_finite()) {
        return Err(TextureFailure::NonFinite);
    }
    if !toggle.enabled {
        return Ok(asset.to_vec());
    }
    Ok(asset.iter().zip(layer.iter()).map(|(a, l)| a + l).collect())
}

/// Click, alarm, and ambience band energies.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BandEnergy {
    pub click: f64,
    pub alarm: f64,
    pub ambience: f64,
}

/// Why a toggle or a band check failed.
#[derive(Clone, Debug, PartialEq)]
pub enum TextureFailure {
    /// A denoise or clean stage was named. That is not the off switch.
    ForbiddenStage {
        name: String,
        token: &'static str,
        class: &'static str,
    },
    /// The toggle line is not the ABI object.
    ToggleLine,
    /// `persona` was present and was not `jarvis`.
    PersonaMismatch,
    /// A sample was NaN or infinite.
    NonFinite,
    /// The rate cannot carry both the click band and the alarm.
    RateTooLow { sample_rate: u32 },
    /// Asset and layer, or the check buffers, are not on one rate.
    RateMismatch { reference: u32, candidate: u32 },
    /// The buffer is shorter than the check can measure, or the mix is empty.
    TooShort { which: &'static str, samples: usize },
    /// The layer or a candidate is not sample-aligned with the asset.
    DurationMismatch { reference: usize, candidate: usize },
    /// The layer does not carry this character.
    LayerSilent { component: &'static str },
    /// Off-state PCM is not the speech asset.
    OffNotAsset { max_abs: f32 },
    /// Off-state band energy fell below the clean-speech baseline.
    TextureStripped {
        component: &'static str,
        baseline: f64,
        got: f64,
    },
    /// Off-state band energy still carries the layer.
    TextureLeaked {
        component: &'static str,
        baseline: f64,
        got: f64,
    },
    /// On-state band energy did not clear the baseline by the layer's keep.
    TextureAbsent {
        component: &'static str,
        have: f64,
        need: f64,
    },
    /// On-state total energy fell below the speech asset.
    AssetStripped { baseline: f64, got: f64 },
}

/// Band check of one toggle pair against the clean-speech asset.
#[derive(Clone, Debug, PartialEq)]
pub struct ToggleVerdict {
    pub passed: bool,
    pub failures: Vec<TextureFailure>,
    pub baseline: Option<BandEnergy>,
    pub layer: Option<BandEnergy>,
    pub off: Option<BandEnergy>,
    pub on: Option<BandEnergy>,
}

/// Energy bands of one mono buffer.
pub fn band_energy(pcm: &[f32], sample_rate: u32) -> Result<BandEnergy, TextureFailure> {
    if sample_rate < MIN_SAMPLE_RATE {
        return Err(TextureFailure::RateTooLow { sample_rate });
    }
    let min_samples = (sample_rate as f32 * MIN_DURATION_SECS) as usize;
    if pcm.len() < min_samples {
        return Err(TextureFailure::TooShort {
            which: "buffer",
            samples: pcm.len(),
        });
    }
    if pcm.iter().any(|s| !s.is_finite()) {
        return Err(TextureFailure::NonFinite);
    }
    let fl = frame_len(sample_rate);
    if fl == 0 || pcm.len() < fl * 4 {
        return Err(TextureFailure::TooShort {
            which: "buffer",
            samples: pcm.len(),
        });
    }
    Ok(BandEnergy {
        click: click_energy(pcm, sample_rate),
        alarm: energy(&bandpass(pcm, sample_rate, ALARM_HZ, ALARM_Q)),
        ambience: ambience_energy(pcm, sample_rate),
    })
}

/// Compare off and on renders with the clean-speech baseline and the hub layer.
///
/// Off must be the asset: same samples, same bands. On must lift click, alarm,
/// and ambience by at least [`KEEP_RATIO`] of the layer's band energy above
/// the baseline. A forbidden stage fails even when the PCM is a correct mix.
/// Neither call mutates `asset` or `layer`; this function only reads them.
pub fn eval_hub_toggle(
    asset: &[f32],
    sample_rate: u32,
    layer: &[f32],
    off_pcm: &[f32],
    on_pcm: &[f32],
    stages: &[&str],
) -> ToggleVerdict {
    let mut failures = Vec::new();
    for name in stages {
        if let Some((token, class)) = match_forbidden_stage(name) {
            failures.push(TextureFailure::ForbiddenStage {
                name: (*name).to_string(),
                token,
                class,
            });
        }
    }
    if layer.len() != asset.len() {
        failures.push(TextureFailure::DurationMismatch {
            reference: asset.len(),
            candidate: layer.len(),
        });
        return finish(failures, None, None, None, None);
    }
    if off_pcm.len() != asset.len() || on_pcm.len() != asset.len() {
        let candidate = if off_pcm.len() != asset.len() {
            off_pcm.len()
        } else {
            on_pcm.len()
        };
        failures.push(TextureFailure::DurationMismatch {
            reference: asset.len(),
            candidate,
        });
        return finish(failures, None, None, None, None);
    }

    let baseline = match band_energy(asset, sample_rate) {
        Ok(b) => b,
        Err(err) => {
            failures.push(label_short(err, "baseline"));
            return finish(failures, None, None, None, None);
        }
    };
    let layer_bands = match band_energy(layer, sample_rate) {
        Ok(b) => b,
        Err(err) => {
            failures.push(label_short(err, "layer"));
            return finish(failures, Some(baseline), None, None, None);
        }
    };
    let off_bands = match band_energy(off_pcm, sample_rate) {
        Ok(b) => b,
        Err(err) => {
            failures.push(label_short(err, "off"));
            return finish(failures, Some(baseline), Some(layer_bands), None, None);
        }
    };
    let on_bands = match band_energy(on_pcm, sample_rate) {
        Ok(b) => b,
        Err(err) => {
            failures.push(label_short(err, "on"));
            return finish(
                failures,
                Some(baseline),
                Some(layer_bands),
                Some(off_bands),
                None,
            );
        }
    };

    let layer_total = energy(layer);
    for (component, layer_e) in layer_parts(&layer_bands) {
        let share = if layer_total > 0.0 {
            layer_e / layer_total
        } else {
            0.0
        };
        if share < LAYER_SHARE_MIN {
            failures.push(TextureFailure::LayerSilent { component });
        }
    }

    let max_abs = max_abs_diff(asset, off_pcm);
    if max_abs > OFF_SAMPLE_SLACK {
        failures.push(TextureFailure::OffNotAsset { max_abs });
    }

    for (component, base_e, layer_e, off_e, on_e) in
        band_rows(&baseline, &layer_bands, &off_bands, &on_bands)
    {
        let scale = layer_e.max(base_e).max(1e-12);
        if off_e < base_e - OFF_BAND_SLACK * scale {
            failures.push(TextureFailure::TextureStripped {
                component,
                baseline: base_e,
                got: off_e,
            });
        } else if off_e > base_e + OFF_BAND_SLACK * scale {
            failures.push(TextureFailure::TextureLeaked {
                component,
                baseline: base_e,
                got: off_e,
            });
        }
        let need = base_e + KEEP_RATIO * layer_e;
        if on_e < need {
            failures.push(TextureFailure::TextureAbsent {
                component,
                have: on_e,
                need,
            });
        }
    }

    let asset_total = energy(asset);
    let on_total = energy(on_pcm);
    if on_total + 1e-6 < asset_total * 0.98 {
        failures.push(TextureFailure::AssetStripped {
            baseline: asset_total,
            got: on_total,
        });
    }

    finish(
        failures,
        Some(baseline),
        Some(layer_bands),
        Some(off_bands),
        Some(on_bands),
    )
}

fn layer_parts(layer: &BandEnergy) -> [(&'static str, f64); 3] {
    [
        ("click", layer.click),
        ("alarm", layer.alarm),
        ("ambience", layer.ambience),
    ]
}

fn band_rows(
    baseline: &BandEnergy,
    layer: &BandEnergy,
    off: &BandEnergy,
    on: &BandEnergy,
) -> [(&'static str, f64, f64, f64, f64); 3] {
    [
        ("click", baseline.click, layer.click, off.click, on.click),
        ("alarm", baseline.alarm, layer.alarm, off.alarm, on.alarm),
        (
            "ambience",
            baseline.ambience,
            layer.ambience,
            off.ambience,
            on.ambience,
        ),
    ]
}

fn label_short(err: TextureFailure, which: &'static str) -> TextureFailure {
    match err {
        TextureFailure::TooShort { samples, .. } => TextureFailure::TooShort { which, samples },
        other => other,
    }
}

fn finish(
    failures: Vec<TextureFailure>,
    baseline: Option<BandEnergy>,
    layer: Option<BandEnergy>,
    off: Option<BandEnergy>,
    on: Option<BandEnergy>,
) -> ToggleVerdict {
    ToggleVerdict {
        passed: failures.is_empty(),
        failures,
        baseline,
        layer,
        off,
        on,
    }
}

fn max_abs_diff(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).abs())
        .fold(0.0f32, f32::max)
}

fn json_bool_field(line: &str, key: &str) -> Result<Option<bool>, TextureFailure> {
    let pattern = format!("\"{key}\"");
    let Some(idx) = line.find(&pattern) else {
        return Ok(None);
    };
    let rest = line[idx + pattern.len()..].trim_start();
    let Some(rest) = rest.strip_prefix(':') else {
        return Err(TextureFailure::ToggleLine);
    };
    let rest = rest.trim_start();
    if rest.starts_with("true") {
        Ok(Some(true))
    } else if rest.starts_with("false") {
        Ok(Some(false))
    } else {
        Err(TextureFailure::ToggleLine)
    }
}

fn json_string_field(line: &str, key: &str) -> Result<Option<String>, TextureFailure> {
    let pattern = format!("\"{key}\"");
    let Some(idx) = line.find(&pattern) else {
        return Ok(None);
    };
    let rest = line[idx + pattern.len()..].trim_start();
    let Some(rest) = rest.strip_prefix(':') else {
        return Err(TextureFailure::ToggleLine);
    };
    let rest = rest.trim_start();
    let Some(rest) = rest.strip_prefix('"') else {
        return Err(TextureFailure::ToggleLine);
    };
    let Some(end) = rest.find('"') else {
        return Err(TextureFailure::ToggleLine);
    };
    Ok(Some(rest[..end].to_string()))
}

fn frame_len(sample_rate: u32) -> usize {
    (sample_rate as usize) * (FRAME_MS as usize) / 1000
}

fn energy(x: &[f32]) -> f64 {
    x.iter()
        .map(|s| {
            let v = *s as f64;
            v * v
        })
        .sum()
}

fn lowpass(x: &[f32], sample_rate: u32, fc: f32) -> Vec<f32> {
    let dt = 1.0 / sample_rate as f32;
    let rc = 1.0 / (2.0 * core::f32::consts::PI * fc);
    let a = dt / (rc + dt);
    let mut y = vec![0.0f32; x.len()];
    let mut acc = 0.0f32;
    for (i, &s) in x.iter().enumerate() {
        acc += a * (s - acc);
        y[i] = acc;
    }
    y
}

fn highpass(x: &[f32], sample_rate: u32, fc: f32) -> Vec<f32> {
    let lp = lowpass(x, sample_rate, fc);
    x.iter().zip(lp.iter()).map(|(s, l)| s - l).collect()
}

fn bandpass(x: &[f32], sample_rate: u32, f0: f32, q: f32) -> Vec<f32> {
    let w0 = 2.0 * core::f32::consts::PI * f0 / sample_rate as f32;
    let alpha = w0.sin() / (2.0 * q);
    let a0 = 1.0 + alpha;
    let b0 = alpha / a0;
    let b1 = 0.0f32;
    let b2 = -alpha / a0;
    let a1 = (-2.0 * w0.cos()) / a0;
    let a2 = (1.0 - alpha) / a0;
    let mut y = vec![0.0f32; x.len()];
    let mut x1 = 0.0f32;
    let mut x2 = 0.0f32;
    let mut y1 = 0.0f32;
    let mut y2 = 0.0f32;
    for (i, &xn) in x.iter().enumerate() {
        let yn = b0 * xn + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
        x2 = x1;
        x1 = xn;
        y2 = y1;
        y1 = yn;
        y[i] = yn;
    }
    y
}

fn click_energy(x: &[f32], sample_rate: u32) -> f64 {
    let hp = highpass(x, sample_rate, CLICK_HIGHPASS_HZ);
    let abs: Vec<f32> = hp.iter().copied().map(f32::abs).collect();
    let env = lowpass(&abs, sample_rate, CLICK_ENVELOPE_HZ);
    let mut mask = vec![false; hp.len()];
    for i in 0..hp.len() {
        let floor = env[i].max(CLICK_FLOOR);
        if abs[i] > CLICK_RATIO * floor {
            let a = i.saturating_sub(2);
            let b = (i + 3).min(hp.len());
            for slot in &mut mask[a..b] {
                *slot = true;
            }
        }
    }
    let mut e = 0.0f64;
    for i in 0..hp.len() {
        if mask[i] {
            let v = hp[i] as f64;
            e += v * v;
        }
    }
    e
}

fn quiet_frame_flags(x: &[f32], sample_rate: u32) -> Vec<bool> {
    let fl = frame_len(sample_rate);
    let n = x.len() / fl;
    let mut rms = Vec::with_capacity(n);
    for fi in 0..n {
        let start = fi * fl;
        rms.push((energy(&x[start..start + fl]) / fl as f64).sqrt());
    }
    let mut sorted = rms.clone();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let idx = ((sorted.len() - 1) as f64 * AMBIENCE_PERCENTILE).floor() as usize;
    let cut = sorted[idx];
    rms.into_iter().map(|r| r <= cut).collect()
}

fn ambience_energy(x: &[f32], sample_rate: u32) -> f64 {
    let lp = lowpass(x, sample_rate, AMBIENCE_LOWPASS_HZ);
    let fl = frame_len(sample_rate);
    let quiet = quiet_frame_flags(x, sample_rate);
    let mut e = 0.0f64;
    for (fi, is_quiet) in quiet.iter().enumerate() {
        if *is_quiet {
            let start = fi * fl;
            e += energy(&lp[start..start + fl]);
        }
    }
    e
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = include_str!("../../../docs/galion-voice-v2/symphony/JARVIS-TEXTURE.md");

    fn clean_speech(sample_rate: u32) -> Vec<f32> {
        let n = sample_rate as usize * 1440 / 1000;
        let mut x = vec![0.0f32; n];
        for i in 0..n {
            if in_phrase(i, sample_rate) {
                let t = i as f32 / sample_rate as f32;
                x[i] = speech_sample(t);
            }
        }
        x
    }

    fn hub_layer(sample_rate: u32) -> Vec<f32> {
        let n = sample_rate as usize * 1440 / 1000;
        let mut x = vec![0.0f32; n];
        for i in 0..n {
            let t = i as f32 / sample_rate as f32;
            if alarm_on(i, sample_rate) {
                x[i] += 0.18 * (2.0 * core::f32::consts::PI * ALARM_HZ * t).sin();
            }
            x[i] += 0.07 * (2.0 * core::f32::consts::PI * 90.0 * t).sin();
            x[i] += 0.035 * (2.0 * core::f32::consts::PI * 180.0 * t).sin();
        }
        let step = sample_rate as usize * 80 / 1000;
        let mut i = step / 2;
        while i < n {
            x[i] += 0.95;
            i += step;
        }
        x
    }

    fn in_phrase(i: usize, sample_rate: u32) -> bool {
        let period = sample_rate as usize * 480 / 1000;
        let on = sample_rate as usize * 240 / 1000;
        (i % period) < on
    }

    fn alarm_on(i: usize, sample_rate: u32) -> bool {
        if !in_phrase(i, sample_rate) {
            return false;
        }
        let period = sample_rate as usize * 120 / 1000;
        let on = sample_rate as usize * 50 / 1000;
        (i % period) < on
    }

    fn speech_sample(t: f32) -> f32 {
        let mut s = 0.0f32;
        let mut h = 1.0f32;
        while 140.0 * h <= 1_100.0 {
            s += (1.0 / h) * (2.0 * core::f32::consts::PI * 140.0 * h * t).sin();
            h += 1.0;
        }
        0.22 * s / 2.6
    }

    fn strip_clicks(x: &[f32]) -> Vec<f32> {
        let w = 8usize;
        let mut y = x.to_vec();
        if x.len() <= w * 2 {
            return y;
        }
        for i in w..x.len() - w {
            let mut abs = Vec::with_capacity(w * 2);
            for k in (i - w)..=(i + w) {
                if k != i {
                    abs.push(x[k].abs());
                }
            }
            abs.sort_by(|a, b| a.total_cmp(b));
            let med = abs[abs.len() / 2];
            if x[i].abs() > 6.0 * med.max(1e-3) {
                y[i] = 0.5 * (x[i - 1] + x[i + 1]);
            }
        }
        y
    }

    fn strip_alarm(x: &[f32], sample_rate: u32) -> Vec<f32> {
        let bp = bandpass(x, sample_rate, ALARM_HZ, ALARM_Q);
        x.iter().zip(bp.iter()).map(|(s, b)| s - b).collect()
    }

    fn strip_ambience(x: &[f32], sample_rate: u32) -> Vec<f32> {
        let quiet = quiet_frame_flags(x, sample_rate);
        let fl = frame_len(sample_rate);
        let mut y = x.to_vec();
        for (fi, is_quiet) in quiet.iter().enumerate() {
            if *is_quiet {
                let start = fi * fl;
                for s in &mut y[start..start + fl] {
                    *s = 0.0;
                }
            }
        }
        y
    }

    fn strip_baked(x: &[f32], sample_rate: u32) -> Vec<f32> {
        let clicks = strip_clicks(x);
        let alarm = strip_alarm(&clicks, sample_rate);
        strip_ambience(&alarm, sample_rate)
    }

    fn pair(sample_rate: u32) -> (Vec<f32>, Vec<f32>, Vec<f32>, Vec<f32>) {
        let asset = clean_speech(sample_rate);
        let layer = hub_layer(sample_rate);
        let off = apply_hub_toggle(
            &asset,
            sample_rate,
            &layer,
            sample_rate,
            TextureToggle { enabled: false },
        )
        .unwrap();
        let on = apply_hub_toggle(
            &asset,
            sample_rate,
            &layer,
            sample_rate,
            TextureToggle { enabled: true },
        )
        .unwrap();
        (asset, layer, off, on)
    }

    #[test]
    fn toggle_off_is_the_asset_and_toggle_on_clears_the_baseline() {
        for rate in [48_000, 24_000] {
            let asset_before = clean_speech(rate);
            let layer_before = hub_layer(rate);
            let (asset, layer, off, on) = pair(rate);
            assert_eq!(asset, asset_before);
            assert_eq!(layer, layer_before);
            assert_eq!(off, asset);
            let verdict = eval_hub_toggle(&asset, rate, &layer, &off, &on, &[]);
            assert!(verdict.passed, "{rate}: {verdict:?}");
            let baseline = verdict.baseline.unwrap();
            let on_bands = verdict.on.unwrap();
            assert!(on_bands.click > baseline.click);
            assert!(on_bands.alarm > baseline.alarm);
            assert!(on_bands.ambience > baseline.ambience);
        }
    }

    #[test]
    fn missing_flag_defaults_on_and_false_does_not_write_the_asset() {
        let rate = 48_000;
        let asset = clean_speech(rate);
        let layer = hub_layer(rate);
        let on = apply_hub_toggle(&asset, rate, &layer, rate, TextureToggle::default()).unwrap();
        let off = apply_hub_toggle(
            &asset,
            rate,
            &layer,
            rate,
            TextureToggle::from_flag(Some(false)),
        )
        .unwrap();
        assert_eq!(off, asset);
        assert!(on
            .iter()
            .zip(asset.iter())
            .any(|(a, b)| (a - b).abs() > 0.01));
    }

    #[test]
    fn stripped_mix_is_not_an_off_switch() {
        let rate = 48_000;
        let (asset, layer, _off, on) = pair(rate);
        let baked: Vec<f32> = asset.iter().zip(layer.iter()).map(|(a, l)| a + l).collect();
        let stripped = strip_baked(&baked, rate);
        let verdict = eval_hub_toggle(&asset, rate, &layer, &stripped, &on, &[]);
        assert!(
            !verdict.passed,
            "a cleaned mix was accepted as off: {verdict:?}"
        );
        assert!(
            verdict
                .failures
                .iter()
                .any(|f| matches!(f, TextureFailure::OffNotAsset { .. })),
            "{verdict:?}"
        );
    }

    #[test]
    fn forbidden_stage_fails_even_when_the_mix_is_right() {
        let rate = 48_000;
        let (asset, layer, off, on) = pair(rate);
        let stages = ["RNNoise", "sox noisered", "declick", "high-pass rumble"];
        let verdict = eval_hub_toggle(&asset, rate, &layer, &off, &on, &stages);
        assert!(!verdict.passed);
        assert_eq!(verdict.failures.len(), stages.len());
    }

    #[test]
    fn allowed_prep_names_pass_on_a_real_toggle() {
        let rate = 24_000;
        let (asset, layer, off, on) = pair(rate);
        let stages = ["downmix", "dc-block", "peak-normalize", "resample"];
        let verdict = eval_hub_toggle(&asset, rate, &layer, &off, &on, &stages);
        assert!(verdict.passed, "{verdict:?}");
    }

    #[test]
    fn toggle_line_abi() {
        let on = parse_toggle_line(r#"{"persona":"jarvis","hub_texture":true}"#).unwrap();
        assert!(on.enabled);
        let off = parse_toggle_line(r#"{"persona":"jarvis","hub_texture":false}"#).unwrap();
        assert!(!off.enabled);
        let absent = parse_toggle_line(r#"{"persona":"jarvis"}"#).unwrap();
        assert!(absent.enabled);
        let cleaned =
            parse_toggle_line(r#"{"persona":"jarvis","hub_texture":false,"denoise":true}"#);
        assert!(matches!(
            cleaned,
            Err(TextureFailure::ForbiddenStage { .. })
        ));
        let other = parse_toggle_line(r#"{"persona":"samantha","hub_texture":true}"#);
        assert!(matches!(other, Err(TextureFailure::PersonaMismatch)));
        assert!(matches!(
            parse_toggle_line("hub_texture=false"),
            Err(TextureFailure::ToggleLine)
        ));
    }

    #[test]
    fn rate_mismatch_and_short_candidate_fail() {
        let asset = clean_speech(48_000);
        let layer = hub_layer(24_000);
        let err =
            apply_hub_toggle(&asset, 48_000, &layer, 24_000, TextureToggle::default()).unwrap_err();
        assert!(matches!(
            err,
            TextureFailure::DurationMismatch { .. } | TextureFailure::RateMismatch { .. }
        ));
        let (asset, layer, off, on) = pair(48_000);
        let short = eval_hub_toggle(&asset, 48_000, &layer, &off[..100], &on, &[]);
        assert!(short
            .failures
            .iter()
            .any(|f| matches!(f, TextureFailure::DurationMismatch { .. })));
    }

    #[test]
    fn forbid_tokens_match_themselves_and_not_the_allow_list() {
        for &(token, _) in FORBIDDEN_CLEAN {
            let found = match_forbidden_stage(token);
            assert_eq!(found.map(|(t, _)| t), Some(token), "{token}");
        }
        for name in ALLOWED_PREP {
            assert!(match_forbidden_stage(name).is_none(), "{name} is forbidden");
        }
        assert_eq!(
            match_forbidden_stage("deepfilternet").unwrap().0,
            "deepfilternet"
        );
        assert_eq!(
            match_forbidden_stage("noisereduce").unwrap().0,
            "noisereduce"
        );
        assert!(match_forbidden_stage("dc-block").is_none());
    }

    #[test]
    fn doc_lists_the_abi_the_bands_and_every_forbidden_token() {
        for &(token, class) in FORBIDDEN_CLEAN {
            assert!(
                DOC.contains(&format!("`{token}`")),
                "doc missing token {token}"
            );
            assert!(DOC.contains(class), "doc missing class {class}");
        }
        for name in ALLOWED_PREP {
            assert!(
                DOC.contains(&format!("`{name}`")),
                "doc missing allow {name}"
            );
        }
        assert!(DOC.contains("6000 Hz"));
        assert!(DOC.contains("3200 Hz"));
        assert!(DOC.contains("250 Hz"));
        assert!(DOC.contains("0.70"));
        assert!(DOC.contains("hub_texture"));
        assert!(DOC.contains("TextureToggle"));
        assert!(DOC.contains("apply_hub_toggle"));
        assert!(DOC.contains("eval_hub_toggle"));
        assert!(DOC.contains("Maciej Grajczyk"));
    }
}
