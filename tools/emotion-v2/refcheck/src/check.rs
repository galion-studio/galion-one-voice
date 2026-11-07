// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! PASS/FAIL a decoded take against the capture spec.

use serde::Serialize;

use crate::dsp::{self};
use crate::spec::{self, Spec, CONSENT, PERSONA, STANDARD};
use crate::wav::{self, Pcm};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Emotion,
    Calibration,
}

impl Role {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "emotion" => Ok(Role::Emotion),
            "calibration" => Ok(Role::Calibration),
            _ => Err("role is emotion or calibration".into()),
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Emotion => "emotion",
            Role::Calibration => "calibration",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Check {
    pub name: &'static str,
    pub pass: bool,
    pub value: Option<f64>,
    pub limit: String,
    pub unit: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub tool: &'static str,
    pub standard: String,
    pub role: String,
    pub wav: String,
    pub spec: String,
    pub verdict: String,
    pub checks: Vec<Check>,
}

impl Report {
    pub fn passed(&self) -> bool {
        self.verdict == "PASS"
    }
}

fn check(
    name: &'static str,
    pass: bool,
    value: Option<f64>,
    limit: String,
    unit: &str,
    detail: impl Into<String>,
) -> Check {
    Check {
        name,
        pass,
        value,
        limit,
        unit: unit.to_string(),
        detail: detail.into(),
    }
}

pub fn check_wav_bytes(
    bytes: &[u8],
    spec: &Spec,
    role: Role,
    wav_name: &str,
    spec_name: &str,
) -> Result<Report, String> {
    let pcm = wav::parse_wav(bytes)?;
    Ok(finish(check_pcm(&pcm, spec, role), wav_name, spec_name))
}

pub fn check_pcm(pcm: &Pcm, spec: &Spec, role: Role) -> Report {
    let mut checks = Vec::new();
    checks.push(spec_identity(spec));

    if pcm.samples.iter().any(|s| !s.is_finite()) {
        checks.push(check(
            "finite",
            false,
            None,
            "finite".into(),
            "",
            "a sample is NaN or infinite",
        ));
        return assemble(checks, role);
    }

    checks.push(check(
        "sample_rate",
        pcm.sample_rate == spec.format.sample_rate_hz,
        Some(pcm.sample_rate as f64),
        spec.format.sample_rate_hz.to_string(),
        "Hz",
        "",
    ));

    let depth_ok = pcm.bits_per_sample == spec.format.bit_depth
        && pcm.kind.as_str() == spec.format.sample_format;
    checks.push(check(
        "bit_depth",
        depth_ok,
        Some(pcm.bits_per_sample as f64),
        format!("{} {}", spec.format.bit_depth, spec.format.sample_format),
        "bit",
        format!("file is {}-bit {}", pcm.bits_per_sample, pcm.kind.as_str()),
    ));

    checks.push(check(
        "channels",
        pcm.channels == spec.format.channels,
        Some(pcm.channels as f64),
        spec.format.channels.to_string(),
        "ch",
        if pcm.channels == 1 {
            "mono"
        } else {
            "downmix is reported; the channel count still fails"
        },
    ));

    let dur = dsp::duration_s(pcm.samples.len(), pcm.sample_rate);
    let (dmin, dmax) = match role {
        Role::Emotion => (spec.duration.min_s, spec.duration.max_s),
        Role::Calibration => (spec.calibration.min_s, spec.calibration.max_s),
    };
    checks.push(check(
        "duration",
        dur >= dmin && dur <= dmax,
        Some(dur),
        format!("{dmin:.1}..{dmax:.1}"),
        "s",
        match role {
            Role::Emotion => "emotion take, quiet ends included",
            Role::Calibration => "calibration take",
        },
    ));

    let lufs = dsp::integrated_lufs(
        &pcm.samples,
        pcm.sample_rate,
        spec.loudness.block_ms,
        spec.loudness.hop_ms,
        spec.loudness.absolute_gate_lufs,
        spec.loudness.relative_gate_lu,
    );
    let lufs_limit = format!(
        "{:.1} ± {:.1}",
        spec.loudness.target_lufs, spec.loudness.tolerance_lu
    );
    match lufs {
        Some(v) => checks.push(check(
            "integrated_lufs",
            (v - spec.loudness.target_lufs).abs() <= spec.loudness.tolerance_lu,
            Some(v),
            lufs_limit,
            "LUFS",
            "mono BS.1770-4, channel weight 1",
        )),
        None => checks.push(check(
            "integrated_lufs",
            false,
            None,
            lufs_limit,
            "LUFS",
            "no block above the absolute gate",
        )),
    }

    let tp = dsp::true_peak_dbtp(&pcm.samples);
    checks.push(check(
        "true_peak",
        tp <= spec.true_peak.max_dbtp,
        Some(tp),
        format!("<= {:.1}", spec.true_peak.max_dbtp),
        "dBTP",
        "4× windowed-sinc",
    ));

    match dsp::noise_floor_dbfs(&pcm.samples, pcm.sample_rate, &spec.noise_floor) {
        Some(v) => checks.push(check(
            "noise_floor",
            v <= spec.noise_floor.max_dbfs,
            Some(v),
            format!("<= {:.0}", spec.noise_floor.max_dbfs),
            "dBFS",
            format!("quietest {} ms window", spec.noise_floor.window_ms),
        )),
        None => checks.push(check(
            "noise_floor",
            false,
            None,
            format!("<= {:.0}", spec.noise_floor.max_dbfs),
            "dBFS",
            "shorter than one noise-floor window",
        )),
    }

    checks.push(check(
        "clipping",
        pcm.clipped_samples <= spec.clipping.max_clipped_samples,
        Some(pcm.clipped_samples as f64),
        format!("<= {}", spec.clipping.max_clipped_samples),
        "samples",
        "full-scale codes, every channel",
    ));

    let dc = dsp::dc_offset_dbfs(&pcm.samples);
    checks.push(check(
        "dc_offset",
        dc <= spec.dc_offset.max_dbfs,
        Some(dc),
        format!("<= {:.0}", spec.dc_offset.max_dbfs),
        "dBFS",
        "20 log10 of the absolute mean",
    ));

    match dsp::estimate_rt60(&pcm.samples, pcm.sample_rate, &spec.reverb) {
        Ok(v) => checks.push(check(
            "reverb",
            v <= spec.reverb.max_rt60_s,
            Some(v),
            format!("<= {:.2}", spec.reverb.max_rt60_s),
            "s",
            "Schroeder T20 on the tail after the sustain",
        )),
        Err(e) => checks.push(check(
            "reverb",
            false,
            None,
            format!("<= {:.2}", spec.reverb.max_rt60_s),
            "s",
            e,
        )),
    }

    match role {
        Role::Emotion => match dsp::spectral_tilt(
            &pcm.samples,
            pcm.sample_rate,
            spec.reference_hz,
            &spec.spectral,
        ) {
            Ok((slope, bands)) => checks.push(check(
                "spectral_tilt",
                (slope - spec.spectral.tilt_db_per_octave).abs() <= spec.spectral.tolerance_db,
                Some(slope),
                format!(
                    "{:.1} ± {:.1}",
                    spec.spectral.tilt_db_per_octave, spec.spectral.tolerance_db
                ),
                "dB/octave",
                bands,
            )),
            Err(e) => checks.push(check(
                "spectral_tilt",
                false,
                None,
                format!(
                    "{:.1} ± {:.1}",
                    spec.spectral.tilt_db_per_octave, spec.spectral.tolerance_db
                ),
                "dB/octave",
                e,
            )),
        },
        Role::Calibration => match dsp::dominant_hz(&pcm.samples, pcm.sample_rate) {
            Ok(hz) => checks.push(check(
                "calibration_tone",
                (hz - spec.calibration.tone_hz).abs() <= spec.calibration.tolerance_hz,
                Some(hz),
                format!(
                    "{:.0} ± {:.0}",
                    spec.calibration.tone_hz, spec.calibration.tolerance_hz
                ),
                "Hz",
                "dominant frequency",
            )),
            Err(e) => checks.push(check(
                "calibration_tone",
                false,
                None,
                format!(
                    "{:.0} ± {:.0}",
                    spec.calibration.tone_hz, spec.calibration.tolerance_hz
                ),
                "Hz",
                e,
            )),
        },
    }

    assemble(checks, role)
}

/// Check a file on disk. The WAV is opened read-only.
pub fn check_file(wav: &Path, spec_path: &Path, role: Role) -> Result<Report, String> {
    let loaded = spec::load_spec(spec_path)?;
    let pcm = wav::read_wav(wav)?;
    let mut report = check_pcm(&pcm, &loaded, role);
    report.wav = wav.display().to_string();
    report.spec = spec_path.display().to_string();
    Ok(report)
}

fn spec_identity(spec: &Spec) -> Check {
    let mut problems = Vec::new();
    if spec.standard != STANDARD {
        problems.push(format!("standard is {}", spec.standard));
    }
    if spec.persona != PERSONA {
        problems.push(format!("persona is {}", spec.persona));
    }
    if spec.consent != CONSENT {
        problems.push(format!("consent is {}", spec.consent));
    }
    if (spec.reference_hz - 432.0).abs() > 1e-6 {
        problems.push(format!("reference_hz is {}", spec.reference_hz));
    }
    if (spec.calibration.tone_hz - 432.0).abs() > 1e-6 {
        problems.push(format!(
            "calibration.tone_hz is {}",
            spec.calibration.tone_hz
        ));
    }
    check(
        "spec_identity",
        problems.is_empty(),
        Some(spec.reference_hz),
        STANDARD.into(),
        "Hz",
        if problems.is_empty() {
            "432 Hz standard, Samantha, owner recordings only".into()
        } else {
            problems.join("; ")
        },
    )
}

fn assemble(checks: Vec<Check>, role: Role) -> Report {
    finish_with(checks, role, "", "")
}

fn finish(mut report: Report, wav: &str, spec_name: &str) -> Report {
    report.wav = wav.to_string();
    report.spec = spec_name.to_string();
    report
}

fn finish_with(checks: Vec<Check>, role: Role, wav: &str, spec_name: &str) -> Report {
    let pass = checks.iter().all(|c| c.pass);
    Report {
        tool: "refcheck",
        standard: STANDARD.into(),
        role: role.as_str().into(),
        wav: wav.into(),
        spec: spec_name.into(),
        verdict: if pass { "PASS" } else { "FAIL" }.into(),
        checks,
    }
}

pub fn to_json(report: &Report) -> String {
    serde_json::to_string_pretty(report).unwrap_or_else(|_| "{\"verdict\":\"ERROR\"}".into())
}
