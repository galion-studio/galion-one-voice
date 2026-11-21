// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! `galion-samantha-ws/1` helpers. Text frames are JSON. Binary frames are
//! the header below plus PCM16. No token is written into a URL by this module.

use crate::{Pad, SAMPLE_RATE_HZ};

pub const SCHEMA: &str = "galion-samantha-ws/1";
pub const HEADER_LEN: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum AudioDir {
    Mic = 0,
    Speaker = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum RateCode {
    Hz24k = 0,
    Hz16k = 1,
}

impl RateCode {
    pub fn hz(self) -> u32 {
        match self {
            RateCode::Hz24k => SAMPLE_RATE_HZ,
            RateCode::Hz16k => crate::ASR_SAMPLE_RATE_HZ,
        }
    }

    pub fn from_hz(hz: u32) -> Option<Self> {
        match hz {
            24_000 => Some(RateCode::Hz24k),
            16_000 => Some(RateCode::Hz16k),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WsError {
    Short,
    Magic,
    Version,
    Dir,
    Rate,
    OddPcm,
    Json,
    NonFinite,
}

impl std::fmt::Display for WsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            WsError::Short => "audio frame shorter than the header",
            WsError::Magic => "audio frame magic is not GS",
            WsError::Version => "audio frame version is not 1",
            WsError::Dir => "audio frame direction is not 0 or 1",
            WsError::Rate => "audio frame rate code is not 0 or 1",
            WsError::OddPcm => "PCM byte length is odd",
            WsError::Json => "JSON value rejected",
            WsError::NonFinite => "mood value is not finite",
        })
    }
}

impl std::error::Error for WsError {}

/// Binary layout, little-endian:
/// `G S version=1 dir rate reserved=0 seq:u16` then s16le mono PCM.
pub fn encode_pcm(dir: AudioDir, rate: RateCode, seq: u16, pcm: &[i16]) -> Vec<u8> {
    let mut out = Vec::with_capacity(HEADER_LEN + pcm.len() * 2);
    out.extend_from_slice(&[b'G', b'S', 1, dir as u8, rate as u8, 0]);
    out.extend_from_slice(&seq.to_le_bytes());
    for s in pcm {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

pub fn decode_pcm(frame: &[u8]) -> Result<(AudioDir, RateCode, u16, Vec<i16>), WsError> {
    if frame.len() < HEADER_LEN {
        return Err(WsError::Short);
    }
    if frame[0] != b'G' || frame[1] != b'S' {
        return Err(WsError::Magic);
    }
    if frame[2] != 1 {
        return Err(WsError::Version);
    }
    let dir = match frame[3] {
        0 => AudioDir::Mic,
        1 => AudioDir::Speaker,
        _ => return Err(WsError::Dir),
    };
    let rate = match frame[4] {
        0 => RateCode::Hz24k,
        1 => RateCode::Hz16k,
        _ => return Err(WsError::Rate),
    };
    if frame.len() % 2 != 0 {
        return Err(WsError::OddPcm);
    }
    let seq = u16::from_le_bytes([frame[6], frame[7]]);
    let pcm = frame[HEADER_LEN..]
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect();
    Ok((dir, rate, seq, pcm))
}

fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn fmt_f32(x: f32) -> Result<String, WsError> {
    if !x.is_finite() {
        return Err(WsError::NonFinite);
    }
    Ok(format!("{x}"))
}

pub fn pad_json(pad: Pad) -> Result<String, WsError> {
    Ok(format!(
        "{{\"arousal\":{},\"valence\":{},\"dominance\":{}}}",
        fmt_f32(pad.arousal)?,
        fmt_f32(pad.valence)?,
        fmt_f32(pad.dominance)?
    ))
}

pub fn welcome_json() -> String {
    format!(
        "{{\"schema\":\"{SCHEMA}\",\"type\":\"welcome\",\"frame_ms\":{},\"sample_rate\":{},\"dot\":\"idle\"}}",
        crate::FRAME_MS,
        SAMPLE_RATE_HZ
    )
}

pub fn error_json(error: &str) -> String {
    format!(
        "{{\"schema\":\"{SCHEMA}\",\"type\":\"error\",\"error\":\"{}\"}}",
        escape(error)
    )
}

/// `source` is `brain` after a real tick, or `resting` when the brain crate
/// is not linked. Equal numbers do not decide which one it is.
pub fn mood_json(t_ms: u32, source: &str, pad: Pad) -> Result<String, WsError> {
    let source = if source == "brain" {
        "brain"
    } else {
        "resting"
    };
    Ok(format!(
        "{{\"schema\":\"{SCHEMA}\",\"type\":\"mood\",\"t_ms\":{t_ms},\"source\":\"{source}\",\"pad\":{}}}",
        pad_json(pad)?
    ))
}

pub fn dot_json(state: &str, level: f32) -> Result<String, WsError> {
    let state = match state {
        "listening" => "listening",
        "speaking" => "speaking",
        _ => "idle",
    };
    Ok(format!(
        "{{\"schema\":\"{SCHEMA}\",\"type\":\"dot\",\"state\":\"{state}\",\"level\":{}}}",
        fmt_f32(level)?
    ))
}

pub fn vad_json(event: &str, t_ms: u32) -> String {
    let event = if event == "start" { "start" } else { "end" };
    format!("{{\"schema\":\"{SCHEMA}\",\"type\":\"vad\",\"event\":\"{event}\",\"t_ms\":{t_ms}}}")
}

/// One `turn` text frame. `intent_json` and `arm_json` are each one JSON
/// value, passed through. This struct does not name their fields.
pub struct TurnJson<'a> {
    pub reply: &'a str,
    pub intent_json: &'a str,
    pub arm_json: &'a str,
    pub propensity: f32,
    pub evidence: &'a [String],
    pub sources: &'a [String],
    pub violations: usize,
    pub lookup_us: u32,
    pub took_us: u32,
    pub pad: Pad,
}

pub fn turn_json(turn: &TurnJson<'_>) -> Result<String, WsError> {
    let intent = one_json_value(turn.intent_json)?;
    let arm = one_json_value(turn.arm_json)?;
    let evidence = string_array(turn.evidence);
    let sources = string_array(turn.sources);
    Ok(format!(
        "{{\"schema\":\"{SCHEMA}\",\"type\":\"turn\",\"reply\":\"{}\",\"intent\":{intent},\"arm\":{arm},\"propensity\":{},\"evidence\":{evidence},\"sources\":{sources},\"violations\":{},\"lookup_us\":{},\"took_us\":{},\"mood\":{}}}",
        escape(turn.reply),
        fmt_f32(turn.propensity)?,
        turn.violations,
        turn.lookup_us,
        turn.took_us,
        pad_json(turn.pad)?
    ))
}

fn string_array(items: &[String]) -> String {
    let mut out = String::from("[");
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push('"');
        out.push_str(&escape(item));
        out.push('"');
    }
    out.push(']');
    out
}

/// Accept one JSON value and nothing after it. Objects and arrays are not
/// interpreted; they only have to be balanced and string-safe.
pub fn one_json_value(input: &str) -> Result<&str, WsError> {
    let s = input.trim();
    let b = s.as_bytes();
    let mut i = 0;
    parse_value(b, &mut i)?;
    skip_ws(b, &mut i);
    if i != b.len() {
        return Err(WsError::Json);
    }
    Ok(s)
}

fn skip_ws(b: &[u8], i: &mut usize) {
    while *i < b.len() && matches!(b[*i], b' ' | b'\n' | b'\r' | b'\t') {
        *i += 1;
    }
}

fn parse_value(b: &[u8], i: &mut usize) -> Result<(), WsError> {
    skip_ws(b, i);
    if *i >= b.len() {
        return Err(WsError::Json);
    }
    match b[*i] {
        b'n' => eat(b, i, b"null"),
        b't' => eat(b, i, b"true"),
        b'f' => eat(b, i, b"false"),
        b'"' => parse_string(b, i),
        b'{' => parse_container(b, i, b'{', b'}'),
        b'[' => parse_container(b, i, b'[', b']'),
        b'-' | b'0'..=b'9' => parse_number(b, i),
        _ => Err(WsError::Json),
    }
}

fn eat(b: &[u8], i: &mut usize, lit: &[u8]) -> Result<(), WsError> {
    if b[*i..].starts_with(lit) {
        *i += lit.len();
        Ok(())
    } else {
        Err(WsError::Json)
    }
}

fn parse_string(b: &[u8], i: &mut usize) -> Result<(), WsError> {
    *i += 1;
    while *i < b.len() {
        match b[*i] {
            b'"' => {
                *i += 1;
                return Ok(());
            }
            b'\\' => {
                *i += 1;
                if *i >= b.len() {
                    return Err(WsError::Json);
                }
                *i += 1;
            }
            c if c < 0x20 => return Err(WsError::Json),
            _ => *i += 1,
        }
    }
    Err(WsError::Json)
}

fn parse_number(b: &[u8], i: &mut usize) -> Result<(), WsError> {
    let start = *i;
    if b[*i] == b'-' {
        *i += 1;
    }
    if *i >= b.len() || !b[*i].is_ascii_digit() {
        return Err(WsError::Json);
    }
    while *i < b.len() && b[*i].is_ascii_digit() {
        *i += 1;
    }
    if *i < b.len() && b[*i] == b'.' {
        *i += 1;
        if *i >= b.len() || !b[*i].is_ascii_digit() {
            return Err(WsError::Json);
        }
        while *i < b.len() && b[*i].is_ascii_digit() {
            *i += 1;
        }
    }
    if *i < b.len() && (b[*i] == b'e' || b[*i] == b'E') {
        *i += 1;
        if *i < b.len() && (b[*i] == b'+' || b[*i] == b'-') {
            *i += 1;
        }
        if *i >= b.len() || !b[*i].is_ascii_digit() {
            return Err(WsError::Json);
        }
        while *i < b.len() && b[*i].is_ascii_digit() {
            *i += 1;
        }
    }
    if *i == start {
        return Err(WsError::Json);
    }
    Ok(())
}

fn parse_container(b: &[u8], i: &mut usize, open: u8, close: u8) -> Result<(), WsError> {
    *i += 1;
    skip_ws(b, i);
    if *i < b.len() && b[*i] == close {
        *i += 1;
        return Ok(());
    }
    loop {
        if open == b'{' {
            skip_ws(b, i);
            if *i >= b.len() || b[*i] != b'"' {
                return Err(WsError::Json);
            }
            parse_string(b, i)?;
            skip_ws(b, i);
            if *i >= b.len() || b[*i] != b':' {
                return Err(WsError::Json);
            }
            *i += 1;
            parse_value(b, i)?;
        } else {
            parse_value(b, i)?;
        }
        skip_ws(b, i);
        if *i >= b.len() {
            return Err(WsError::Json);
        }
        match b[*i] {
            b',' => {
                *i += 1;
            }
            c if c == close => {
                *i += 1;
                return Ok(());
            }
            _ => return Err(WsError::Json),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pcm_roundtrip_uses_integers_not_a_recording() {
        let pcm = [0, 1, -1, 32767];
        let bytes = encode_pcm(AudioDir::Speaker, RateCode::Hz24k, 7, &pcm);
        let (dir, rate, seq, back) = decode_pcm(&bytes).unwrap();
        assert_eq!(dir, AudioDir::Speaker);
        assert_eq!(rate.hz(), 24_000);
        assert_eq!(seq, 7);
        assert_eq!(back, pcm);
        assert!(decode_pcm(&[0, 1]).is_err());
    }

    #[test]
    fn welcome_and_resting_mood_match_the_schema() {
        assert_eq!(
            welcome_json(),
            "{\"schema\":\"galion-samantha-ws/1\",\"type\":\"welcome\",\"frame_ms\":80,\"sample_rate\":24000,\"dot\":\"idle\"}"
        );
        let mood = mood_json(0, "resting", Pad::RESTING).unwrap();
        assert!(mood.contains("\"source\":\"resting\""));
        assert!(mood.contains("\"valence\":0.25"));
        assert!(pad_json(Pad {
            arousal: f32::NAN,
            valence: 0.0,
            dominance: 0.0
        })
        .is_err());
    }

    #[test]
    fn turn_rejects_a_broken_intent_and_escapes_the_reply() {
        assert!(one_json_value("null").is_ok());
        assert!(one_json_value("{\"k\":1}").is_ok());
        assert!(one_json_value("{\"a\":[1,{\"b\":null}]}").is_ok());
        assert!(one_json_value("{1:2}").is_err());
        assert!(one_json_value("null true").is_err());
        let sources = ["card-1".to_string()];
        let line = turn_json(&TurnJson {
            reply: "He said \"hi\"",
            intent_json: "null",
            arm_json: "null",
            propensity: 0.5,
            evidence: &[],
            sources: &sources,
            violations: 0,
            lookup_us: 12,
            took_us: 40,
            pad: Pad::RESTING,
        })
        .unwrap();
        assert!(line.contains("He said \\\"hi\\\""));
        assert!(line.contains("\"sources\":[\"card-1\"]"));
        assert!(turn_json(&TurnJson {
            reply: "x",
            intent_json: "not-json",
            arm_json: "null",
            propensity: 0.0,
            evidence: &[],
            sources: &[],
            violations: 0,
            lookup_us: 0,
            took_us: 0,
            pad: Pad::RESTING,
        })
        .is_err());
    }
}
