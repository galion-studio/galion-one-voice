// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! The profile file. One schema, one writer, stable bytes.
//!
//! Floats are decimal-rounded in the writer. Times come from sample counts.
//! The same samples and the same moment list produce the same bytes.

use crate::emotion::{MomentFrame, MOMENT_HOOK, MOMENT_SCHEMA, MOMENT_TICK_MS};
use crate::vad::VadSummary;

pub const SCHEMA: &str = "galion-preclone/1";
pub const STAGE: &str = "pre-clone-analysis";

#[derive(Clone, Debug, PartialEq)]
pub struct Request<'a> {
    pub samples: &'a [f32],
    pub sample_rate_hz: u32,
    pub target_f0_hz: Option<f64>,
    pub moments: &'a [MomentFrame],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SegmentKind {
    Target,
    Other,
    Music,
    Noise,
}

impl SegmentKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Target => "target",
            Self::Other => "other",
            Self::Music => "music",
            Self::Noise => "noise",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Segment {
    pub start_sample: usize,
    pub end_sample: usize,
    pub kind: SegmentKind,
    pub kept: bool,
    pub f0_hz: f64,
    pub snr_db: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VoiceProfile {
    pub f0_hz: f64,
    pub f1_hz: f64,
    pub f2_hz: f64,
    pub f3_hz: f64,
    pub spectral_centroid_hz: f64,
    pub hnr_db: f64,
    pub voiced_fraction: f64,
    pub kept_samples: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EmotionProfile {
    pub attached: bool,
    pub frames: Vec<MomentFrame>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnvironmentProfile {
    pub noise_floor_dbfs: Option<f64>,
    pub active_level_dbfs: Option<f64>,
    pub snr_db: Option<f64>,
    pub rt60_s: Option<f64>,
    pub rt60_note: String,
    pub noise_tilt_db_per_octave: Option<f64>,
    pub noise_color: String,
    pub music_bed: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Profile {
    pub sample_rate_hz: u32,
    pub duration_samples: usize,
    pub voice: VoiceProfile,
    pub emotion: EmotionProfile,
    pub environment: EnvironmentProfile,
    pub energy_vad: VadSummary,
    pub segments: Vec<Segment>,
}

pub fn profile_bytes(profile: &Profile) -> Vec<u8> {
    to_string(profile).into_bytes()
}

fn to_string(p: &Profile) -> String {
    let sr = p.sample_rate_hz;
    let mut s = String::new();
    s.push_str("{\n");
    prop_str(&mut s, 1, "schema", SCHEMA, true);
    prop_str(&mut s, 1, "stage", STAGE, true);
    prop_raw(&mut s, 1, "compute", "\"cpu\"", true);
    prop_raw(
        &mut s,
        1,
        "sample_rate_hz",
        &p.sample_rate_hz.to_string(),
        true,
    );
    prop_raw(
        &mut s,
        1,
        "duration_samples",
        &p.duration_samples.to_string(),
        true,
    );
    prop_raw(
        &mut s,
        1,
        "duration_s",
        &fmt_seconds(p.duration_samples as u64, sr),
        true,
    );
    s.push_str("  \"voice\": {\n");
    prop_str(&mut s, 2, "kind", "steady-baseline", true);
    prop_num(&mut s, 2, "f0_hz", p.voice.f0_hz, true);
    prop_num(&mut s, 2, "f1_hz", p.voice.f1_hz, true);
    prop_num(&mut s, 2, "f2_hz", p.voice.f2_hz, true);
    prop_num(&mut s, 2, "f3_hz", p.voice.f3_hz, true);
    prop_num(
        &mut s,
        2,
        "spectral_centroid_hz",
        p.voice.spectral_centroid_hz,
        true,
    );
    prop_num(&mut s, 2, "hnr_db", p.voice.hnr_db, true);
    prop_num(&mut s, 2, "voiced_fraction", p.voice.voiced_fraction, true);
    prop_raw(
        &mut s,
        2,
        "kept_samples",
        &p.voice.kept_samples.to_string(),
        true,
    );
    prop_raw(
        &mut s,
        2,
        "kept_s",
        &fmt_seconds(p.voice.kept_samples as u64, sr),
        false,
    );
    s.push_str("  },\n");
    s.push_str("  \"emotion\": {\n");
    prop_str(&mut s, 2, "role", "baseline-separate-from-moment", true);
    prop_raw(&mut s, 2, "baseline_is_voice", "true", true);
    s.push_str("    \"moment\": {\n");
    prop_str(&mut s, 3, "hook", MOMENT_HOOK, true);
    prop_str(&mut s, 3, "schema", MOMENT_SCHEMA, true);
    prop_raw(&mut s, 3, "tick_ms", &MOMENT_TICK_MS.to_string(), true);
    prop_raw(
        &mut s,
        3,
        "attached",
        if p.emotion.attached { "true" } else { "false" },
        true,
    );
    prop_raw(
        &mut s,
        3,
        "frame_count",
        &p.emotion.frames.len().to_string(),
        true,
    );
    write_frames(&mut s, &p.emotion.frames);
    s.push_str("    }\n");
    s.push_str("  },\n");
    s.push_str("  \"environment\": {\n");
    prop_opt(
        &mut s,
        2,
        "noise_floor_dbfs",
        p.environment.noise_floor_dbfs,
        true,
    );
    prop_opt(
        &mut s,
        2,
        "active_level_dbfs",
        p.environment.active_level_dbfs,
        true,
    );
    prop_opt(&mut s, 2, "snr_db", p.environment.snr_db, true);
    prop_opt(&mut s, 2, "rt60_s", p.environment.rt60_s, true);
    prop_str(&mut s, 2, "rt60_note", &p.environment.rt60_note, true);
    prop_opt(
        &mut s,
        2,
        "noise_tilt_db_per_octave",
        p.environment.noise_tilt_db_per_octave,
        true,
    );
    prop_str(&mut s, 2, "noise_color", &p.environment.noise_color, true);
    prop_raw(
        &mut s,
        2,
        "music_bed",
        if p.environment.music_bed {
            "true"
        } else {
            "false"
        },
        false,
    );
    s.push_str("  },\n");
    s.push_str("  \"energy_vad\": {\n");
    prop_raw(
        &mut s,
        2,
        "frame_ms",
        &p.energy_vad.frame_ms.to_string(),
        true,
    );
    prop_num(&mut s, 2, "on_db", p.energy_vad.on_db, true);
    prop_num(&mut s, 2, "clamp_dbfs", p.energy_vad.clamp_dbfs, true);
    prop_num(
        &mut s,
        2,
        "floor_start_dbfs",
        p.energy_vad.floor_start_dbfs,
        true,
    );
    prop_raw(
        &mut s,
        2,
        "voiced_frames",
        &p.energy_vad.voiced_frames.to_string(),
        true,
    );
    prop_raw(&mut s, 2, "frames", &p.energy_vad.frames.to_string(), false);
    s.push_str("  },\n");
    s.push_str("  \"segments\": [\n");
    for (i, seg) in p.segments.iter().enumerate() {
        let comma = if i + 1 == p.segments.len() { "" } else { "," };
        s.push_str("    {\n");
        prop_raw(
            &mut s,
            3,
            "start_sample",
            &seg.start_sample.to_string(),
            true,
        );
        prop_raw(&mut s, 3, "end_sample", &seg.end_sample.to_string(), true);
        prop_raw(
            &mut s,
            3,
            "start_s",
            &fmt_seconds(seg.start_sample as u64, sr),
            true,
        );
        prop_raw(
            &mut s,
            3,
            "end_s",
            &fmt_seconds(seg.end_sample as u64, sr),
            true,
        );
        prop_str(&mut s, 3, "kind", seg.kind.as_str(), true);
        prop_raw(
            &mut s,
            3,
            "kept",
            if seg.kept { "true" } else { "false" },
            true,
        );
        prop_num(&mut s, 3, "f0_hz", seg.f0_hz, true);
        prop_num(&mut s, 3, "snr_db", seg.snr_db, false);
        s.push_str("    }");
        s.push_str(comma);
        s.push('\n');
    }
    s.push_str("  ]\n");
    s.push_str("}\n");
    s
}

fn write_frames(s: &mut String, frames: &[MomentFrame]) {
    if frames.is_empty() {
        s.push_str("      \"frames\": []\n");
        return;
    }
    s.push_str("      \"frames\": [\n");
    for (i, frame) in frames.iter().enumerate() {
        let comma = if i + 1 == frames.len() { "" } else { "," };
        s.push_str("        {\n");
        prop_raw(s, 5, "t_ms", &frame.t_ms.to_string(), true);
        s.push_str("          \"axes\": [\n");
        for (j, axis) in frame.axes.iter().enumerate() {
            let acomma = if j + 1 == frame.axes.len() { "" } else { "," };
            s.push_str("            {\"name\": \"");
            s.push_str(&escape(&axis.name));
            s.push_str("\", \"value\": ");
            s.push_str(&fmt_f64(axis.value));
            s.push('}');
            s.push_str(acomma);
            s.push('\n');
        }
        s.push_str("          ]\n");
        s.push_str("        }");
        s.push_str(comma);
        s.push('\n');
    }
    s.push_str("      ]\n");
}

fn pad(out: &mut String, indent: usize) {
    for _ in 0..indent {
        out.push_str("  ");
    }
}

fn prop_raw(out: &mut String, indent: usize, key: &str, value: &str, comma: bool) {
    pad(out, indent);
    out.push('"');
    out.push_str(key);
    out.push_str("\": ");
    out.push_str(value);
    if comma {
        out.push(',');
    }
    out.push('\n');
}

fn prop_str(out: &mut String, indent: usize, key: &str, value: &str, comma: bool) {
    prop_raw(out, indent, key, &format!("\"{}\"", escape(value)), comma);
}

fn prop_num(out: &mut String, indent: usize, key: &str, value: f64, comma: bool) {
    prop_raw(out, indent, key, &fmt_f64(value), comma);
}

fn prop_opt(out: &mut String, indent: usize, key: &str, value: Option<f64>, comma: bool) {
    match value {
        Some(v) => prop_num(out, indent, key, v, comma),
        None => prop_raw(out, indent, key, "null", comma),
    }
}

fn escape(s: &str) -> String {
    let mut o = String::new();
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if c.is_control() => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o
}

pub(crate) fn fmt_f64(x: f64) -> String {
    if !x.is_finite() {
        return "null".to_string();
    }
    let sign = if x.is_sign_negative() && x != 0.0 {
        "-"
    } else {
        ""
    };
    let scaled = (x.abs() * 1_000_000.0).round();
    if !scaled.is_finite() || scaled >= (u64::MAX as f64) {
        return "null".to_string();
    }
    let scaled = scaled as u64;
    let int = scaled / 1_000_000;
    let frac = scaled % 1_000_000;
    if frac == 0 {
        return format!("{sign}{int}");
    }
    let mut frac_s = format!("{frac:06}");
    while frac_s.ends_with('0') {
        frac_s.pop();
    }
    format!("{sign}{int}.{frac_s}")
}

fn fmt_seconds(samples: u64, sr: u32) -> String {
    if sr == 0 {
        return "0".to_string();
    }
    let sr = u128::from(sr);
    let n = u128::from(samples);
    let micros = (n * 1_000_000 + sr / 2) / sr;
    let int = micros / 1_000_000;
    let frac = (micros % 1_000_000) as u64;
    if frac == 0 {
        return format!("{int}");
    }
    let mut frac_s = format!("{frac:06}");
    while frac_s.ends_with('0') {
        frac_s.pop();
    }
    format!("{int}.{frac_s}")
}
