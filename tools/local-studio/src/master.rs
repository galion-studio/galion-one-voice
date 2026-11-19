// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Delivery master: integrated loudness -16 LUFS, true peak at or below -1 dBTP.
//!
//! The in-process meter follows ITU-R BS.1770-4 for mono audio (K-weighting,
//! 400 ms blocks, absolute gate, relative gate, channel weight 1). True peak
//! is a 4× windowed-sinc. The same filter arrangement is used by the Emotion
//! V2 capture checker in this repository.
//!
//! When ffmpeg is selected, `loudnorm` does the gain with `linear=true` so
//! the bed stays a fixed distance under the voice. The in-process meter
//! checks the file ffmpeg wrote.

use crate::audio::{read_wav_mono, write_wav_mono};
use crate::command::{tail, TempDir};
use serde_json::Value;
use std::path::Path;
use std::process::Command;

const PI: f64 = std::f64::consts::PI;

#[derive(Debug, Clone)]
pub struct Mastered {
    pub samples: Vec<f32>,
    pub lufs: f64,
    pub true_peak_dbtp: f64,
    pub limited_by: String,
    pub engine: String,
}

pub fn apply(
    samples: &[f32],
    sample_rate: u32,
    target_lufs: f64,
    ceiling_dbtp: f64,
) -> Result<Mastered, String> {
    let measured = integrated_lufs(samples, sample_rate)?.ok_or_else(|| {
        "integrated loudness needs 400 ms above the -70 LUFS absolute gate".to_string()
    })?;
    let peak = true_peak_dbtp(samples);
    let loudness_gain_db = target_lufs - measured;
    let peak_gain_db = ceiling_dbtp - peak;
    let gain_db = loudness_gain_db.min(peak_gain_db);
    let limited_by = if peak_gain_db + 0.05 < loudness_gain_db {
        "true-peak"
    } else {
        "loudness"
    };
    let mut out = scale(samples, 10f64.powf(gain_db / 20.0));
    let mut peak_after = true_peak_dbtp(&out);
    if peak_after > ceiling_dbtp + 0.02 {
        let trim = 10f64.powf((ceiling_dbtp - peak_after) / 20.0);
        out = scale(&out, trim);
        peak_after = true_peak_dbtp(&out);
    }
    let lufs = integrated_lufs(&out, sample_rate)?
        .ok_or_else(|| "mastered audio dropped below the loudness gate".to_string())?;
    Ok(Mastered {
        samples: out,
        lufs,
        true_peak_dbtp: peak_after,
        limited_by: limited_by.into(),
        engine: "in-process".into(),
    })
}

pub fn apply_ffmpeg(
    ffmpeg: &Path,
    samples: &[f32],
    sample_rate: u32,
    target_lufs: f64,
    ceiling_dbtp: f64,
) -> Result<Mastered, String> {
    let dir = TempDir::new("galion-studio-master").map_err(|err| err.message().to_string())?;
    let input = dir.0.join("in.wav");
    let output = dir.0.join("out.wav");
    write_wav_mono(&input, samples, sample_rate)?;
    let first = loudnorm_pass(
        ffmpeg,
        &input,
        &loudnorm_measure_filter(target_lufs, ceiling_dbtp),
        None,
    )?;
    let measured = parse_loudnorm(&first)?;
    let second = loudnorm_pass(
        ffmpeg,
        &input,
        &loudnorm_apply_filter(target_lufs, ceiling_dbtp, &measured),
        Some(&output),
    )?;
    let _ = second;
    let (mut mastered, sr) = read_wav_mono(&output)?;
    if sr != sample_rate {
        mastered = crate::audio::resample_linear(&mastered, sr, sample_rate);
    }
    let mut peak = true_peak_dbtp(&mastered);
    if peak > ceiling_dbtp + 0.02 {
        let trim = 10f64.powf((ceiling_dbtp - peak) / 20.0);
        mastered = scale(&mastered, trim);
        peak = true_peak_dbtp(&mastered);
    }
    let lufs = integrated_lufs(&mastered, sample_rate)?.unwrap_or(f64::NAN);
    let limited_by = if peak > ceiling_dbtp - 0.4 && lufs + 0.4 < target_lufs {
        "true-peak"
    } else {
        "loudness"
    };
    Ok(Mastered {
        samples: mastered,
        lufs,
        true_peak_dbtp: peak,
        limited_by: limited_by.into(),
        engine: "ffmpeg".into(),
    })
}

pub fn loudnorm_measure_filter(target_lufs: f64, ceiling_dbtp: f64) -> String {
    format!("loudnorm=I={target_lufs}:TP={ceiling_dbtp}:LRA=11:print_format=json")
}

pub fn loudnorm_apply_filter(
    target_lufs: f64,
    ceiling_dbtp: f64,
    measured: &LoudnormMeasure,
) -> String {
    format!(
        "loudnorm=I={target_lufs}:TP={ceiling_dbtp}:LRA=11:measured_I={}:measured_LRA={}:measured_TP={}:measured_thresh={}:offset={}:linear=true:print_format=json",
        measured.input_i, measured.input_lra, measured.input_tp, measured.input_thresh, measured.target_offset
    )
}

#[derive(Debug, Clone)]
pub struct LoudnormMeasure {
    pub input_i: f64,
    pub input_tp: f64,
    pub input_lra: f64,
    pub input_thresh: f64,
    pub target_offset: f64,
}

pub fn parse_loudnorm(text: &str) -> Result<LoudnormMeasure, String> {
    let start = text.rfind('{').ok_or("ffmpeg loudnorm returned no JSON")?;
    let end = text[start..]
        .find('}')
        .map(|i| start + i + 1)
        .ok_or("ffmpeg loudnorm JSON is truncated")?;
    let value: Value = serde_json::from_str(&text[start..end])
        .map_err(|err| format!("ffmpeg loudnorm JSON: {err}"))?;
    Ok(LoudnormMeasure {
        input_i: json_f64(&value, "input_i")?,
        input_tp: json_f64(&value, "input_tp")?,
        input_lra: json_f64(&value, "input_lra")?,
        input_thresh: json_f64(&value, "input_thresh")?,
        target_offset: json_f64(&value, "target_offset")?,
    })
}

fn json_f64(value: &Value, key: &str) -> Result<f64, String> {
    let field = value
        .get(key)
        .ok_or_else(|| format!("ffmpeg loudnorm JSON is missing {key}"))?;
    let number = match field {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    };
    number
        .filter(|n| n.is_finite())
        .ok_or_else(|| format!("ffmpeg loudnorm field {key} is not a number"))
}

fn loudnorm_pass(
    ffmpeg: &Path,
    input: &Path,
    filter: &str,
    dest: Option<&Path>,
) -> Result<String, String> {
    let mut cmd = Command::new(ffmpeg);
    cmd.arg("-hide_banner")
        .arg("-y")
        .arg("-i")
        .arg(input)
        .arg("-af")
        .arg(filter);
    if let Some(dest) = dest {
        cmd.arg("-c:a").arg("pcm_s16le").arg(dest);
    } else {
        cmd.arg("-f").arg("null").arg("-");
    }
    let output = cmd
        .output()
        .map_err(|err| format!("{}: {err}", ffmpeg.display()))?;
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    if !output.status.success() {
        return Err(format!("ffmpeg loudnorm failed: {}", tail(&stderr, 500)));
    }
    Ok(stderr + &stdout)
}

fn scale(samples: &[f32], gain: f64) -> Vec<f32> {
    let gain = gain as f32;
    samples.iter().map(|sample| sample * gain).collect()
}

pub fn db10(power: f64) -> f64 {
    if power <= 1e-20 {
        -200.0
    } else {
        10.0 * power.log10()
    }
}

pub fn db20(amp: f64) -> f64 {
    if amp <= 1e-12 {
        -240.0
    } else {
        20.0 * amp.log10()
    }
}

struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    z1: f64,
    z2: f64,
}

impl Biquad {
    fn new(b: [f64; 3], a: [f64; 3]) -> Self {
        Self {
            b0: b[0],
            b1: b[1],
            b2: b[2],
            a1: a[1],
            a2: a[2],
            z1: 0.0,
            z2: 0.0,
        }
    }

    fn process(&mut self, x: f64) -> f64 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }
}

fn k_coeffs(sample_rate: u32) -> ([f64; 3], [f64; 3], [f64; 3], [f64; 3]) {
    let sr = f64::from(sample_rate);
    let f0 = 1681.974450955533;
    let g_db = 3.999843853973347;
    let q = 0.7071752369554196;
    let k = (PI * f0 / sr).tan();
    let vh = 10f64.powf(g_db / 20.0);
    let vb = vh.powf(0.4996667741545416);
    let a0 = 1.0 + k / q + k * k;
    let pre_b = [
        (vh + vb * k / q + k * k) / a0,
        2.0 * (k * k - vh) / a0,
        (vh - vb * k / q + k * k) / a0,
    ];
    let pre_a = [1.0, 2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0];

    let f0 = 38.13547087602444;
    let q = 0.5003270373238773;
    let k = (PI * f0 / sr).tan();
    let a0 = 1.0 + k / q + k * k;
    let hp_b = [1.0 / a0, -2.0 / a0, 1.0 / a0];
    let hp_a = [1.0, 2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0];
    (pre_b, pre_a, hp_b, hp_a)
}

pub fn k_weight(samples: &[f32], sample_rate: u32) -> Result<Vec<f64>, String> {
    if sample_rate < 8_000 {
        return Err("K-weighting needs a sample rate of at least 8000".into());
    }
    let (pre_b, pre_a, hp_b, hp_a) = k_coeffs(sample_rate);
    let mut shelf = Biquad::new(pre_b, pre_a);
    let mut highpass = Biquad::new(hp_b, hp_a);
    let mut out = Vec::with_capacity(samples.len());
    for sample in samples {
        out.push(highpass.process(shelf.process(f64::from(*sample))));
    }
    Ok(out)
}

pub fn integrated_lufs(samples: &[f32], sample_rate: u32) -> Result<Option<f64>, String> {
    if sample_rate == 0 {
        return Err("sample rate is 0".into());
    }
    let weighted = k_weight(samples, sample_rate)?;
    let block = ((u64::from(sample_rate) * 400) / 1000) as usize;
    let hop = ((u64::from(sample_rate) * 100) / 1000).max(1) as usize;
    if block == 0 || weighted.len() < block {
        return Ok(None);
    }
    let mut power = Vec::new();
    let mut i = 0;
    while i + block <= weighted.len() {
        let mut acc = 0.0;
        for sample in &weighted[i..i + block] {
            acc += sample * sample;
        }
        power.push(acc / block as f64);
        i += hop;
    }
    let loud = |z: f64| -0.691 + db10(z);
    let absolute: Vec<f64> = power.into_iter().filter(|z| loud(*z) > -70.0).collect();
    if absolute.is_empty() {
        return Ok(None);
    }
    let mean = |xs: &[f64]| xs.iter().sum::<f64>() / xs.len() as f64;
    let ungated = loud(mean(&absolute));
    let relative: Vec<f64> = absolute
        .into_iter()
        .filter(|z| loud(*z) > ungated - 10.0)
        .collect();
    if relative.is_empty() {
        return Ok(None);
    }
    Ok(Some(loud(mean(&relative))))
}

fn sinc(x: f64) -> f64 {
    if x.abs() < 1e-8 {
        1.0
    } else {
        (PI * x).sin() / (PI * x)
    }
}

fn phase_kernel(phase: f64, half: i32) -> Vec<f64> {
    let mut kernel = Vec::with_capacity((half * 2 + 1) as usize);
    let mut sum = 0.0;
    for k in -half..=half {
        let dist = f64::from(k) - phase;
        let hann = if dist.abs() >= f64::from(half) {
            0.0
        } else {
            0.5 * (1.0 + (PI * dist / f64::from(half)).cos())
        };
        let coeff = sinc(dist) * hann;
        kernel.push(coeff);
        sum += coeff;
    }
    if sum.abs() > 1e-12 {
        for coeff in &mut kernel {
            *coeff /= sum;
        }
    }
    kernel
}

pub fn true_peak_linear(samples: &[f32]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let mut peak = 0.0f64;
    for sample in samples {
        peak = peak.max(f64::from(*sample).abs());
    }
    let half = 32i32;
    let kernels = [
        phase_kernel(0.25, half),
        phase_kernel(0.5, half),
        phase_kernel(0.75, half),
    ];
    let n = samples.len() as i32;
    let start = half;
    let end = n - half - 1;
    if end <= start {
        return peak;
    }
    for i in start..end {
        for kernel in &kernels {
            let mut acc = 0.0;
            for (t, coeff) in kernel.iter().enumerate() {
                let idx = i + t as i32 - half;
                acc += f64::from(samples[idx as usize]) * coeff;
            }
            peak = peak.max(acc.abs());
        }
    }
    peak
}

pub fn true_peak_dbtp(samples: &[f32]) -> f64 {
    db20(true_peak_linear(samples))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::sine;

    #[test]
    fn a_quiet_sine_lands_on_minus_sixteen_lufs() {
        let sr = 24_000;
        let tone = sine(sr as usize * 2, sr, 432.0, 0.02);
        let mastered = apply(&tone, sr, -16.0, -1.0).unwrap();
        assert!((mastered.lufs + 16.0).abs() < 0.2, "{}", mastered.lufs);
        assert!(
            mastered.true_peak_dbtp <= -1.0 + 1e-6,
            "{}",
            mastered.true_peak_dbtp
        );
        assert_eq!(mastered.limited_by, "loudness");
        assert_eq!(mastered.engine, "in-process");
    }

    #[test]
    fn a_peaky_signal_stops_at_the_true_peak_ceiling() {
        let sr = 24_000u32;
        let mut tone = vec![0.0f32; sr as usize * 2];
        let step = sr as usize / 5;
        let mut i = step / 2;
        while i < tone.len() {
            tone[i] = 0.8;
            i += step;
        }
        let mastered = apply(&tone, sr, -16.0, -1.0).unwrap();
        assert!(
            mastered.true_peak_dbtp <= -0.98,
            "{}",
            mastered.true_peak_dbtp
        );
        assert!(
            mastered.true_peak_dbtp > -1.5,
            "{}",
            mastered.true_peak_dbtp
        );
        assert_eq!(mastered.limited_by, "true-peak");
        assert!(mastered.lufs < -16.0);
    }

    #[test]
    fn short_audio_has_no_integrated_loudness() {
        let tone = sine(100, 24_000, 432.0, 0.2);
        let err = apply(&tone, 24_000, -16.0, -1.0).unwrap_err();
        assert!(err.contains("400"));
    }

    #[test]
    fn loudnorm_filter_carries_the_delivery_spec() {
        let filter = loudnorm_measure_filter(-16.0, -1.0);
        assert!(filter.contains("I=-16"));
        assert!(filter.contains("TP=-1"));
        let measured = parse_loudnorm(
            "size=N/A\n{\n\"input_i\" : \"-24.50\",\n\"input_tp\" : \"-3.00\",\n\"input_lra\" : \"2.00\",\n\"input_thresh\" : \"-35.00\",\n\"target_offset\" : \"0.10\"\n}\n",
        )
        .unwrap();
        let apply = loudnorm_apply_filter(-16.0, -1.0, &measured);
        assert!(apply.contains("linear=true"));
        assert!(apply.contains("measured_I=-24.5"));
        assert!(apply.contains("offset=0.1"));
    }

    #[test]
    fn ffmpeg_master_hits_the_spec_when_the_binary_is_installed() {
        let Some(bin) = crate::command::on_path("ffmpeg") else {
            return;
        };
        let sr = 24_000;
        let tone = sine(sr as usize * 2, sr, 432.0, 0.02);
        let mastered = apply_ffmpeg(&bin, &tone, sr, -16.0, -1.0).unwrap();
        assert_eq!(mastered.engine, "ffmpeg");
        assert!((mastered.lufs + 16.0).abs() < 1.0, "lufs {}", mastered.lufs);
        assert!(
            mastered.true_peak_dbtp <= -0.95,
            "tp {}",
            mastered.true_peak_dbtp
        );
    }
}
