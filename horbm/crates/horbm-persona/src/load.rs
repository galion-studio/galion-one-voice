// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Persona file. TOML in, a fixed [`Persona`] out. Allocates at boot.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::style::hook;
use crate::{
    pack_bytes, Boundary, Persona, PersonaError, StyleHook, TraitSlot, BOUNDARY_BYTES,
    BOUNDARY_SLOTS, SCHEMA, TOKEN_BYTES, TRAIT_SLOTS, UNKNOWN_LINE, VERSION,
};

pub(crate) fn load(path: &Path) -> Result<Persona, PersonaError> {
    let text = fs::read_to_string(path).map_err(|_| PersonaError::Unreadable)?;
    parse(&text)
}

pub(crate) fn parse(text: &str) -> Result<Persona, PersonaError> {
    refuse_markers(text)?;
    // The parser's own sentence is not part of the boot error.
    let raw: RawPersona = toml::from_str(text)
        .map_err(|_| PersonaError::Invalid("persona file does not match the schema"))?;
    check_header(&raw)?;
    let id = pack_name(&raw.id, "id is not a short token")?;
    let voice = pack_name(&raw.voice, "voice is not a short token")?;
    let (traits, trait_len) = pack_traits(&raw.traits)?;
    check_honesty(&raw.honesty)?;
    let (boundaries, boundary_len) = pack_boundaries(raw.boundaries.forbid)?;
    let style = pack_style(&raw.style)?;
    let axes = pack_baseline(&raw.baseline)?;
    let care = pack_word(&raw.inner.care)?;
    let want = pack_word(&raw.inner.want)?;
    let expect = pack_word(&raw.inner.expect)?;
    Ok(Persona {
        id,
        voice,
        traits,
        trait_len,
        boundaries,
        boundary_len,
        style,
        valence: axes.valence,
        arousal: axes.arousal,
        dominance: axes.dominance,
        approach: axes.approach,
        pitch_centre_hz: axes.pitch,
        care,
        want,
        expect,
    })
}

pub(crate) fn to_toml(persona: &Persona) -> String {
    let mut out = String::new();
    out.push_str("schema = \"");
    out.push_str(SCHEMA);
    out.push_str("\"\nversion = ");
    out.push_str(&VERSION.to_string());
    out.push_str("\nid = \"");
    out.push_str(persona.id());
    out.push_str("\"\nvoice = \"");
    out.push_str(persona.voice());
    out.push_str("\"\n\n[traits]\n");
    let mut i = 0;
    while let Some((key, value)) = persona.trait_at(i) {
        out.push_str(key);
        out.push_str(" = ");
        out.push_str(&fmt_f32(value));
        out.push('\n');
        i += 1;
    }
    out.push_str("\n[honesty]\nnever_fabricate = true\nunknown = \"");
    out.push_str(UNKNOWN_LINE);
    out.push_str("\"\n\n[boundaries]\nforbid = [");
    let mut b = 0;
    while let Some(token) = persona.boundary_at(b) {
        if b > 0 {
            out.push_str(", ");
        }
        out.push('"');
        out.push_str(token);
        out.push('"');
        b += 1;
    }
    let style = persona.style();
    out.push_str("]\n\n[style]\nbrevity = ");
    out.push_str(&fmt_f32(style.brevity));
    out.push_str("\nwarmth = ");
    out.push_str(&fmt_f32(style.warmth));
    out.push_str("\nmax_sentences = ");
    out.push_str(&style.max_sentences.to_string());
    out.push_str("\n\n[baseline]\nvalence = ");
    out.push_str(&fmt_f32(persona.valence()));
    out.push_str("\narousal = ");
    out.push_str(&fmt_f32(persona.arousal()));
    out.push_str("\ndominance = ");
    out.push_str(&fmt_f32(persona.dominance()));
    out.push_str("\napproach = ");
    out.push_str(&fmt_f32(persona.approach()));
    out.push_str("\npitch_centre_hz = ");
    out.push_str(&fmt_f32(persona.pitch_centre_hz()));
    out.push_str("\n\n[inner]\ncare = \"");
    out.push_str(persona.care());
    out.push_str("\"\nwant = \"");
    out.push_str(persona.want());
    out.push_str("\"\nexpect = \"");
    out.push_str(persona.expect_token());
    out.push_str("\"\n");
    out
}

fn refuse_markers(text: &str) -> Result<(), PersonaError> {
    let lower = text.to_ascii_lowercase();
    if lower.contains('@')
        || lower.contains("http://")
        || lower.contains("https://")
        || lower.contains(".wav")
        || lower.contains(".mp3")
        || lower.contains(".flac")
        || lower.contains(".ogg")
        || lower.contains(".pcm")
        || has_ipv4(lower.as_bytes())
        || has_long_hex(lower.as_bytes())
    {
        return Err(PersonaError::Invalid(
            "persona text contains a secret-shaped marker",
        ));
    }
    Ok(())
}

fn has_ipv4(bytes: &[u8]) -> bool {
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() && (i == 0 || !bytes[i - 1].is_ascii_digit()) {
            if let Some(end) = ipv4_end(bytes, i) {
                if end == bytes.len() || !bytes[end].is_ascii_digit() {
                    return true;
                }
            }
        }
        i += 1;
    }
    false
}

fn ipv4_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut p = start;
    for part in 0..4 {
        if part > 0 {
            if p >= bytes.len() || bytes[p] != b'.' {
                return None;
            }
            p += 1;
        }
        if p >= bytes.len() || !bytes[p].is_ascii_digit() {
            return None;
        }
        let mut val: u16 = 0;
        let mut digits = 0u8;
        while p < bytes.len() && bytes[p].is_ascii_digit() {
            digits = digits.saturating_add(1);
            if digits > 3 {
                return None;
            }
            val = val * 10 + u16::from(bytes[p] - b'0');
            if val > 255 {
                return None;
            }
            p += 1;
        }
        if digits == 0 {
            return None;
        }
    }
    Some(p)
}

fn has_long_hex(bytes: &[u8]) -> bool {
    let mut run = 0u32;
    for b in bytes {
        if b.is_ascii_hexdigit() {
            run += 1;
            if run >= 32 {
                return true;
            }
        } else {
            run = 0;
        }
    }
    false
}

fn check_header(raw: &RawPersona) -> Result<(), PersonaError> {
    if raw.schema != SCHEMA {
        return Err(PersonaError::Invalid(
            "schema must be galion-horbm-persona/1",
        ));
    }
    if raw.version != VERSION {
        return Err(PersonaError::Invalid("version must be 1"));
    }
    Ok(())
}

fn check_honesty(raw: &RawHonesty) -> Result<(), PersonaError> {
    if !raw.never_fabricate {
        return Err(PersonaError::Invalid("never_fabricate must be true"));
    }
    if raw.unknown != UNKNOWN_LINE {
        return Err(PersonaError::Invalid("unknown line must be I don't know."));
    }
    Ok(())
}

fn pack_traits(
    traits: &BTreeMap<String, f32>,
) -> Result<([TraitSlot; TRAIT_SLOTS], u8), PersonaError> {
    if traits.is_empty() || traits.len() > TRAIT_SLOTS {
        return Err(PersonaError::Invalid(
            "traits must be 1 to 8 finite values from 0 to 1",
        ));
    }
    let mut out = [TraitSlot::EMPTY; TRAIT_SLOTS];
    for (i, (key, value)) in traits.iter().enumerate() {
        if !name_ok(key) {
            return Err(PersonaError::Invalid("trait name is not a short token"));
        }
        if !in_range(*value, 0.0, 1.0) {
            return Err(PersonaError::Invalid(
                "trait value must be a finite number from 0 to 1",
            ));
        }
        out[i] = TraitSlot::new(key.as_bytes(), canon(*value));
    }
    Ok((out, u8::try_from(traits.len()).unwrap_or(0)))
}

fn pack_boundaries(
    mut forbid: Vec<String>,
) -> Result<([Boundary; BOUNDARY_SLOTS], u8), PersonaError> {
    if forbid.len() > BOUNDARY_SLOTS {
        return Err(PersonaError::Invalid("too many boundary tokens"));
    }
    for token in &forbid {
        if !boundary_ok(token) {
            return Err(PersonaError::Invalid("boundary token is not a short token"));
        }
    }
    forbid.sort();
    for pair in forbid.windows(2) {
        if pair[0] == pair[1] {
            return Err(PersonaError::Invalid("duplicate boundary token"));
        }
    }
    let mut out = [Boundary::EMPTY; BOUNDARY_SLOTS];
    for (i, token) in forbid.iter().enumerate() {
        out[i] = Boundary::new(token.as_bytes());
    }
    Ok((out, u8::try_from(forbid.len()).unwrap_or(0)))
}

fn pack_style(raw: &RawStyle) -> Result<StyleHook, PersonaError> {
    if !in_range(raw.brevity, 0.0, 1.0) || !in_range(raw.warmth, 0.0, 1.0) {
        return Err(PersonaError::Invalid("style value is outside its range"));
    }
    if !(1..=4).contains(&raw.max_sentences) {
        return Err(PersonaError::Invalid("max_sentences must be 1 to 4"));
    }
    Ok(hook(
        canon(raw.brevity),
        canon(raw.warmth),
        raw.max_sentences,
    ))
}

struct Axes {
    valence: f32,
    arousal: f32,
    dominance: f32,
    approach: f32,
    pitch: f32,
}

fn pack_baseline(raw: &RawBaseline) -> Result<Axes, PersonaError> {
    if !in_range(raw.valence, -1.0, 1.0)
        || !in_range(raw.arousal, 0.0, 1.0)
        || !in_range(raw.dominance, 0.0, 1.0)
        || !in_range(raw.approach, -1.0, 1.0)
        || !in_range(raw.pitch_centre_hz, 1.0, 2000.0)
    {
        return Err(PersonaError::Invalid("baseline value is outside its range"));
    }
    Ok(Axes {
        valence: canon(raw.valence),
        arousal: canon(raw.arousal),
        dominance: canon(raw.dominance),
        approach: canon(raw.approach),
        pitch: canon(raw.pitch_centre_hz),
    })
}

fn pack_word(s: &str) -> Result<[u8; TOKEN_BYTES], PersonaError> {
    if !inner_ok(s) {
        return Err(PersonaError::Invalid(
            "inner token must be 1 to 16 lowercase letters",
        ));
    }
    Ok(pack_bytes(s.as_bytes()))
}

pub(crate) fn pack_name(s: &str, reason: &'static str) -> Result<[u8; TOKEN_BYTES], PersonaError> {
    if !name_ok(s) {
        return Err(PersonaError::Invalid(reason));
    }
    Ok(pack_bytes(s.as_bytes()))
}

fn name_ok(s: &str) -> bool {
    token_ok(s, TOKEN_BYTES)
}

fn boundary_ok(s: &str) -> bool {
    token_ok(s, BOUNDARY_BYTES)
}

fn token_ok(s: &str, max: usize) -> bool {
    let b = s.as_bytes();
    (1..=max).contains(&b.len())
        && b[0].is_ascii_lowercase()
        && b[1..]
            .iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
}

fn inner_ok(s: &str) -> bool {
    let b = s.as_bytes();
    (1..=TOKEN_BYTES).contains(&b.len()) && b.iter().all(|c| c.is_ascii_lowercase())
}

fn in_range(v: f32, lo: f32, hi: f32) -> bool {
    v.is_finite() && v >= lo && v <= hi
}

fn canon(v: f32) -> f32 {
    if v == 0.0 {
        0.0
    } else {
        v
    }
}

fn fmt_f32(v: f32) -> String {
    if v == 0.0 {
        return "0.0".to_string();
    }
    for prec in 1..=9 {
        let s = format!("{v:.prec$}");
        if let Ok(wide) = s.parse::<f64>() {
            if (wide as f32).to_bits() == v.to_bits() {
                return decimal(s);
            }
        }
    }
    decimal(format!("{v:.9}"))
}

fn decimal(s: String) -> String {
    if s.contains('.') || s.contains('e') || s.contains('E') {
        s
    } else {
        format!("{s}.0")
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPersona {
    schema: String,
    version: u32,
    id: String,
    voice: String,
    traits: BTreeMap<String, f32>,
    honesty: RawHonesty,
    boundaries: RawBoundaries,
    style: RawStyle,
    baseline: RawBaseline,
    inner: RawInner,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawHonesty {
    never_fabricate: bool,
    unknown: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBoundaries {
    forbid: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStyle {
    brevity: f32,
    warmth: f32,
    max_sentences: u8,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBaseline {
    valence: f32,
    arousal: f32,
    dominance: f32,
    approach: f32,
    pitch_centre_hz: f32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawInner {
    care: String,
    want: String,
    expect: String,
}
