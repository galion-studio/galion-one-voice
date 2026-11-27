// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! `galion-voice-design/1`. One writer, stable bytes.
//!
//! Key order is fixed. Floats use the pre-clone decimal rounding. Nothing
//! in the file is a path, a sample, or a name of a real person.

use serde_json::{Map, Value};

use super::guard::{self, consent_from, SCALES};
use super::math::ACOUSTIC_WEIGHT;
use super::num::fmt;
use super::persona::{self, fits_slot};
use crate::profile::{Profile, VoiceProfile, SCHEMA as PRECLONE_SCHEMA};

pub const SCHEMA: &str = "galion-voice-design/1";
pub const STAGE: &str = "aria-blend";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceConsent {
    OwnVoice,
    FictionalReference,
}

impl SourceConsent {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OwnVoice => "own-voice",
            Self::FictionalReference => "fictional-reference",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimbreKind {
    BlendedFormant,
    ParentSteady,
}

impl TimbreKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BlendedFormant => "blended-formant",
            Self::ParentSteady => "parent-steady",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        match text {
            "blended-formant" => Some(Self::BlendedFormant),
            "parent-steady" => Some(Self::ParentSteady),
            _ => None,
        }
    }
}

/// One allowed pre-clone profile. `voice` is the steady baseline.
#[derive(Clone, Debug, PartialEq)]
pub struct SourceProfile {
    pub consent: SourceConsent,
    pub voice: VoiceProfile,
}

impl SourceProfile {
    pub fn from_profile(consent: SourceConsent, profile: &Profile) -> Self {
        Self {
            consent,
            voice: profile.voice.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AgeTarget {
    pub target_years: f64,
    pub anchor_years: f64,
    pub shift_years: f64,
    pub f0_scale: f64,
    pub formant_scale: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BaselineTarget {
    pub f0_centre_hz: f64,
    pub f0_lo_hz: f64,
    pub f0_hi_hz: f64,
    pub f0_span_hz: f64,
    pub f1_hz: f64,
    pub f2_hz: f64,
    pub f3_hz: f64,
    pub spectral_centroid_hz: f64,
    pub timbre: TimbreKind,
    pub rate: f64,
    pub breathiness: f64,
    pub hnr_db: f64,
    pub voiced_fraction: f64,
}

/// The rest point a later `VoiceBaseline` can copy. Value only.
#[derive(Clone, Debug, PartialEq)]
pub struct EmotionDefault {
    pub persona_id: String,
    pub voice_id: String,
    pub valence: f64,
    pub arousal: f64,
    pub dominance: f64,
    pub approach: f64,
    pub pitch_centre_hz: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParentSlot {
    pub consent: SourceConsent,
    pub baseline: BaselineTarget,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DesignSpec {
    pub age: AgeTarget,
    pub baseline: BaselineTarget,
    pub emotion: EmotionDefault,
    pub samantha: ParentSlot,
    pub jarvis: ParentSlot,
    pub distance_samantha: f64,
    pub distance_jarvis: f64,
}

pub fn source_bytes(source: &SourceProfile) -> Vec<u8> {
    let mut s = String::new();
    s.push_str("{\n");
    prop_str(&mut s, 1, "schema", PRECLONE_SCHEMA, true);
    s.push_str("  \"source\": {\n");
    prop_str(&mut s, 2, "consent", source.consent.as_str(), false);
    s.push_str("  },\n");
    s.push_str("  \"voice\": {\n");
    prop_str(&mut s, 2, "kind", "steady-baseline", true);
    prop_num(&mut s, 2, "f0_hz", source.voice.f0_hz, true);
    prop_num(&mut s, 2, "f1_hz", source.voice.f1_hz, true);
    prop_num(&mut s, 2, "f2_hz", source.voice.f2_hz, true);
    prop_num(&mut s, 2, "f3_hz", source.voice.f3_hz, true);
    prop_num(
        &mut s,
        2,
        "spectral_centroid_hz",
        source.voice.spectral_centroid_hz,
        true,
    );
    prop_num(&mut s, 2, "hnr_db", source.voice.hnr_db, true);
    prop_num(
        &mut s,
        2,
        "voiced_fraction",
        source.voice.voiced_fraction,
        true,
    );
    prop_raw(
        &mut s,
        2,
        "kept_samples",
        &source.voice.kept_samples.to_string(),
        false,
    );
    s.push_str("  }\n");
    s.push_str("}\n");
    s.into_bytes()
}

pub fn spec_bytes(spec: &DesignSpec) -> Vec<u8> {
    let mut s = String::new();
    s.push_str("{\n");
    prop_str(&mut s, 1, "schema", SCHEMA, true);
    prop_str(&mut s, 1, "stage", STAGE, true);
    prop_raw(&mut s, 1, "compute", "\"cpu\"", true);
    prop_str(&mut s, 1, "voice_id", "aria", true);
    prop_str(&mut s, 1, "kind", "synthetic-blend", true);
    prop_raw(&mut s, 1, "not_a_person", "true", true);
    s.push_str("  \"blend\": {\n");
    prop_num(&mut s, 2, "acoustic_weight_each", ACOUSTIC_WEIGHT, false);
    s.push_str("  },\n");
    s.push_str("  \"age\": {\n");
    prop_num(&mut s, 2, "target_years", spec.age.target_years, true);
    prop_num(&mut s, 2, "anchor_years", spec.age.anchor_years, true);
    prop_num(&mut s, 2, "shift_years", spec.age.shift_years, true);
    prop_num(&mut s, 2, "f0_scale", spec.age.f0_scale, true);
    prop_num(&mut s, 2, "formant_scale", spec.age.formant_scale, false);
    s.push_str("  },\n");
    s.push_str("  \"baseline\": {\n");
    write_baseline(&mut s, 2, &spec.baseline);
    s.push_str("  },\n");
    s.push_str("  \"emotion_default\": {\n");
    prop_str(&mut s, 2, "feeds", persona::FEEDS, true);
    prop_str(&mut s, 2, "persona_id", &spec.emotion.persona_id, true);
    prop_str(&mut s, 2, "voice_id", &spec.emotion.voice_id, true);
    prop_raw(
        &mut s,
        2,
        "persona_slot_bytes",
        &persona::SLOT_BYTES.to_string(),
        true,
    );
    prop_raw(
        &mut s,
        2,
        "voice_slot_bytes",
        &persona::SLOT_BYTES.to_string(),
        true,
    );
    write_str_array(&mut s, 2, "axis_order", persona::AXIS_ORDER, true);
    prop_num(&mut s, 2, "valence", spec.emotion.valence, true);
    prop_num(&mut s, 2, "arousal", spec.emotion.arousal, true);
    prop_num(&mut s, 2, "dominance", spec.emotion.dominance, true);
    prop_num(&mut s, 2, "approach", spec.emotion.approach, true);
    prop_num(
        &mut s,
        2,
        "pitch_centre_hz",
        spec.emotion.pitch_centre_hz,
        false,
    );
    s.push_str("  },\n");
    write_str_array(&mut s, 1, "traits", persona::TRAITS, true);
    s.push_str("  \"from\": {\n");
    write_str_array(&mut s, 2, "samantha", persona::FROM_SAMANTHA, true);
    write_str_array(&mut s, 2, "jarvis", persona::FROM_JARVIS, false);
    s.push_str("  },\n");
    s.push_str("  \"parents\": {\n");
    write_parent(&mut s, "samantha", &spec.samantha, true);
    write_parent(&mut s, "jarvis", &spec.jarvis, false);
    s.push_str("  },\n");
    s.push_str("  \"distance\": {\n");
    prop_str(&mut s, 2, "metric", guard::DISTANCE_METRIC, true);
    prop_num(&mut s, 2, "minimum", guard::MIN_BASELINE_DISTANCE, true);
    prop_num(&mut s, 2, "to_samantha", spec.distance_samantha, true);
    prop_num(&mut s, 2, "to_jarvis", spec.distance_jarvis, true);
    s.push_str("    \"scales\": {\n");
    for (i, (name, value)) in SCALES.iter().enumerate() {
        prop_num(&mut s, 3, name, *value, i + 1 != SCALES.len());
    }
    s.push_str("    }\n");
    s.push_str("  }\n");
    s.push_str("}\n");
    s.into_bytes()
}

pub fn parse_source(text: &str) -> Result<SourceProfile, String> {
    let value: Value = serde_json::from_str(text).map_err(|e| format!("source: {e}"))?;
    let root = object(&value, "source", "profile")?;
    let schema = read_str(root, "schema", "source")?;
    if schema != PRECLONE_SCHEMA {
        return Err(format!("source: schema must be {PRECLONE_SCHEMA}"));
    }
    let consent = match root.get("source") {
        None => return Err("refused: consent is missing".into()),
        Some(v) => {
            let src = v
                .as_object()
                .ok_or_else(|| "source: source must be an object".to_string())?;
            match src.get("consent") {
                None => return Err("refused: consent is missing".into()),
                Some(c) => {
                    let text = c
                        .as_str()
                        .ok_or_else(|| "refused: consent is missing".to_string())?;
                    consent_from(text)?
                }
            }
        }
    };
    let voice_v = root
        .get("voice")
        .ok_or_else(|| "source: voice is missing".to_string())?;
    let voice = voice_v
        .as_object()
        .ok_or_else(|| "source: voice must be an object".to_string())?;
    Ok(SourceProfile {
        consent,
        voice: read_voice(voice)?,
    })
}

pub fn parse_spec(text: &str) -> Result<DesignSpec, String> {
    let value: Value = serde_json::from_str(text).map_err(|e| format!("voice-design: {e}"))?;
    let root = object(&value, "voice-design", "spec")?;
    expect_str(root, "schema", SCHEMA)?;
    expect_str(root, "stage", STAGE)?;
    expect_str(root, "compute", "cpu")?;
    expect_str(root, "voice_id", "aria")?;
    expect_str(root, "kind", "synthetic-blend")?;
    if !read_bool(root, "not_a_person")? {
        return Err("voice-design: not_a_person must be true".into());
    }
    let blend = child(root, "blend")?;
    let weight = read_f64(blend, "acoustic_weight_each")?;
    if (weight - ACOUSTIC_WEIGHT).abs() > 1e-9 {
        return Err("voice-design: acoustic weight is not the equal blend".into());
    }
    let age_v = child(root, "age")?;
    let age = AgeTarget {
        target_years: read_f64(age_v, "target_years")?,
        anchor_years: read_f64(age_v, "anchor_years")?,
        shift_years: read_f64(age_v, "shift_years")?,
        f0_scale: read_f64(age_v, "f0_scale")?,
        formant_scale: read_f64(age_v, "formant_scale")?,
    };
    let baseline = read_baseline(child(root, "baseline")?)?;
    if baseline.timbre != TimbreKind::BlendedFormant {
        return Err("voice-design: baseline timbre must be blended-formant".into());
    }
    let emotion = read_emotion(child(root, "emotion_default")?)?;
    if (emotion.pitch_centre_hz - baseline.f0_centre_hz).abs() > 1e-9 {
        return Err("voice-design: pitch_centre_hz is not the baseline f0 centre".into());
    }
    expect_list(root, "traits", persona::TRAITS)?;
    let from = child(root, "from")?;
    expect_list(from, "samantha", persona::FROM_SAMANTHA)?;
    expect_list(from, "jarvis", persona::FROM_JARVIS)?;
    let parents = child(root, "parents")?;
    let samantha = read_parent(child(parents, "samantha")?)?;
    let jarvis = read_parent(child(parents, "jarvis")?)?;
    let distance = child(root, "distance")?;
    expect_str(distance, "metric", guard::DISTANCE_METRIC)?;
    let minimum = read_f64(distance, "minimum")?;
    if (minimum - guard::MIN_BASELINE_DISTANCE).abs() > 1e-9 {
        return Err("voice-design: minimum is not this stage's".into());
    }
    let scales = child(distance, "scales")?;
    for (name, want) in SCALES {
        let got = read_f64(scales, name)?;
        if (got - want).abs() > 1e-9 {
            return Err(format!("voice-design: scale {name} is not this stage's"));
        }
    }
    Ok(DesignSpec {
        age,
        baseline,
        emotion,
        samantha,
        jarvis,
        distance_samantha: read_f64(distance, "to_samantha")?,
        distance_jarvis: read_f64(distance, "to_jarvis")?,
    })
}

fn read_voice(voice: &Map<String, Value>) -> Result<VoiceProfile, String> {
    fn num(voice: &Map<String, Value>, key: &str) -> Result<f64, String> {
        voice
            .get(key)
            .and_then(Value::as_f64)
            .filter(|n| n.is_finite())
            .ok_or_else(|| format!("source: {key} is not a finite number"))
    }
    let kept = match voice.get("kept_samples") {
        None => 0,
        Some(v) => {
            let n = v
                .as_u64()
                .ok_or_else(|| "source: kept_samples must be an integer".to_string())?;
            usize::try_from(n).map_err(|_| "source: kept_samples does not fit".to_string())?
        }
    };
    Ok(VoiceProfile {
        f0_hz: num(voice, "f0_hz")?,
        f1_hz: num(voice, "f1_hz")?,
        f2_hz: num(voice, "f2_hz")?,
        f3_hz: num(voice, "f3_hz")?,
        spectral_centroid_hz: num(voice, "spectral_centroid_hz")?,
        hnr_db: num(voice, "hnr_db")?,
        voiced_fraction: num(voice, "voiced_fraction")?,
        kept_samples: kept,
    })
}

fn read_baseline(obj: &Map<String, Value>) -> Result<BaselineTarget, String> {
    let timbre = TimbreKind::parse(&read_str(obj, "timbre", "voice-design")?)
        .ok_or_else(|| "voice-design: timbre is not a known kind".to_string())?;
    Ok(BaselineTarget {
        f0_centre_hz: read_f64(obj, "f0_centre_hz")?,
        f0_lo_hz: read_f64(obj, "f0_lo_hz")?,
        f0_hi_hz: read_f64(obj, "f0_hi_hz")?,
        f0_span_hz: read_f64(obj, "f0_span_hz")?,
        f1_hz: read_f64(obj, "f1_hz")?,
        f2_hz: read_f64(obj, "f2_hz")?,
        f3_hz: read_f64(obj, "f3_hz")?,
        spectral_centroid_hz: read_f64(obj, "spectral_centroid_hz")?,
        timbre,
        rate: read_f64(obj, "rate")?,
        breathiness: read_f64(obj, "breathiness")?,
        hnr_db: read_f64(obj, "hnr_db")?,
        voiced_fraction: read_f64(obj, "voiced_fraction")?,
    })
}

fn read_emotion(obj: &Map<String, Value>) -> Result<EmotionDefault, String> {
    expect_str(obj, "feeds", persona::FEEDS)?;
    let persona_id = read_str(obj, "persona_id", "voice-design")?;
    let voice_id = read_str(obj, "voice_id", "voice-design")?;
    if persona_id != persona::PERSONA_ID || voice_id != persona::VOICE_ID {
        return Err("voice-design: emotion id is not aria".into());
    }
    if !fits_slot(&persona_id) || !fits_slot(&voice_id) {
        return Err("voice-design: emotion id does not fit the 16-byte slot".into());
    }
    let persona_slot = read_u64(obj, "persona_slot_bytes")?;
    let voice_slot = read_u64(obj, "voice_slot_bytes")?;
    if persona_slot != persona::SLOT_BYTES || voice_slot != persona::SLOT_BYTES {
        return Err("voice-design: VoiceBaseline slots are 16 bytes".into());
    }
    expect_list(obj, "axis_order", persona::AXIS_ORDER)?;
    Ok(EmotionDefault {
        persona_id,
        voice_id,
        valence: read_f64(obj, "valence")?,
        arousal: read_f64(obj, "arousal")?,
        dominance: read_f64(obj, "dominance")?,
        approach: read_f64(obj, "approach")?,
        pitch_centre_hz: read_f64(obj, "pitch_centre_hz")?,
    })
}

fn read_parent(obj: &Map<String, Value>) -> Result<ParentSlot, String> {
    let consent = consent_from(&read_str(obj, "consent", "voice-design")?)?;
    let baseline = read_baseline(child(obj, "baseline")?)?;
    if baseline.timbre != TimbreKind::ParentSteady {
        return Err("voice-design: parent timbre must be parent-steady".into());
    }
    Ok(ParentSlot { consent, baseline })
}

fn write_parent(out: &mut String, name: &str, slot: &ParentSlot, comma: bool) {
    pad(out, 2);
    out.push('"');
    out.push_str(name);
    out.push_str("\": {\n");
    prop_str(out, 3, "consent", slot.consent.as_str(), true);
    out.push_str("      \"baseline\": {\n");
    write_baseline(out, 4, &slot.baseline);
    out.push_str("      }\n");
    out.push_str("    }");
    if comma {
        out.push(',');
    }
    out.push('\n');
}

fn write_baseline(out: &mut String, indent: usize, b: &BaselineTarget) {
    prop_num(out, indent, "f0_centre_hz", b.f0_centre_hz, true);
    prop_num(out, indent, "f0_lo_hz", b.f0_lo_hz, true);
    prop_num(out, indent, "f0_hi_hz", b.f0_hi_hz, true);
    prop_num(out, indent, "f0_span_hz", b.f0_span_hz, true);
    prop_num(out, indent, "f1_hz", b.f1_hz, true);
    prop_num(out, indent, "f2_hz", b.f2_hz, true);
    prop_num(out, indent, "f3_hz", b.f3_hz, true);
    prop_num(
        out,
        indent,
        "spectral_centroid_hz",
        b.spectral_centroid_hz,
        true,
    );
    prop_str(out, indent, "timbre", b.timbre.as_str(), true);
    prop_num(out, indent, "rate", b.rate, true);
    prop_num(out, indent, "breathiness", b.breathiness, true);
    prop_num(out, indent, "hnr_db", b.hnr_db, true);
    prop_num(out, indent, "voiced_fraction", b.voiced_fraction, false);
}

fn write_str_array(out: &mut String, indent: usize, key: &str, values: &[&str], comma: bool) {
    pad(out, indent);
    out.push('"');
    out.push_str(key);
    out.push_str("\": [");
    for (i, v) in values.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        out.push('"');
        out.push_str(&escape(v));
        out.push('"');
    }
    out.push(']');
    if comma {
        out.push(',');
    }
    out.push('\n');
}

fn object<'a>(v: &'a Value, prefix: &str, what: &str) -> Result<&'a Map<String, Value>, String> {
    v.as_object()
        .ok_or_else(|| format!("{prefix}: {what} must be an object"))
}

fn child<'a>(obj: &'a Map<String, Value>, key: &str) -> Result<&'a Map<String, Value>, String> {
    obj.get(key)
        .and_then(Value::as_object)
        .ok_or_else(|| format!("voice-design: {key} must be an object"))
}

fn read_str(obj: &Map<String, Value>, key: &str, prefix: &str) -> Result<String, String> {
    obj.get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("{prefix}: {key} is missing"))
}

fn read_f64(obj: &Map<String, Value>, key: &str) -> Result<f64, String> {
    let v = obj
        .get(key)
        .ok_or_else(|| format!("voice-design: {key} is missing"))?;
    v.as_f64()
        .filter(|n| n.is_finite())
        .ok_or_else(|| format!("voice-design: {key} is not a finite number"))
}

fn read_bool(obj: &Map<String, Value>, key: &str) -> Result<bool, String> {
    obj.get(key)
        .and_then(Value::as_bool)
        .ok_or_else(|| format!("voice-design: {key} must be a bool"))
}

fn read_u64(obj: &Map<String, Value>, key: &str) -> Result<u64, String> {
    obj.get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("voice-design: {key} must be an integer"))
}

fn expect_str(obj: &Map<String, Value>, key: &str, want: &str) -> Result<(), String> {
    let got = read_str(obj, key, "voice-design")?;
    if got == want {
        Ok(())
    } else {
        Err(format!("voice-design: {key} must be {want}"))
    }
}

fn expect_list(obj: &Map<String, Value>, key: &str, want: &[&str]) -> Result<(), String> {
    let arr = obj
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("voice-design: {key} must be a list"))?;
    let got: Vec<&str> = arr.iter().filter_map(Value::as_str).collect();
    if got.len() == arr.len() && got == want {
        Ok(())
    } else {
        Err(format!("voice-design: {key} is not this stage's list"))
    }
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
    prop_raw(out, indent, key, &fmt(value), comma);
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
