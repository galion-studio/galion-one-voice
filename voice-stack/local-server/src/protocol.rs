// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! `galion-samantha-ws/1`, the socket named in the Swarm 16 architecture.
//! Text frames are JSON. Binary frames are the eight-byte `GS` header plus
//! little-endian PCM16. This file matches that layout. It does not import
//! `galion-sam-contract`, which is not on `main`.

use serde_json::Value;

use crate::audio::{FRAME_MS, SPEAKER_RATE};
use crate::reply::Pad;

pub const SCHEMA: &str = "galion-samantha-ws/1";
pub const HEADER_LEN: usize = 8;
pub const WS_PATH: &str = "/v1/samantha";
pub const DEFAULT_PORT: u16 = 7881;
pub const ONE_DOT_ORIGIN: &str = "http://127.0.0.1:7878";
pub const HELLO_DEADLINE_SECS: u64 = 5;
pub const MAX_SAMPLES: usize = 4_800;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioDir {
    Mic = 0,
    Speaker = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateCode {
    Hz24k = 0,
    Hz16k = 1,
}

impl RateCode {
    pub fn hz(self) -> u32 {
        match self {
            RateCode::Hz24k => 24_000,
            RateCode::Hz16k => 16_000,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecError {
    Short,
    Magic,
    Version,
    Dir,
    Rate,
    OddPcm,
    TooLong,
}

impl std::fmt::Display for CodecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            CodecError::Short => "audio frame shorter than the header",
            CodecError::Magic => "audio frame magic is not GS",
            CodecError::Version => "audio frame version is not 1",
            CodecError::Dir => "audio frame direction is not 0 or 1",
            CodecError::Rate => "audio frame rate code is not 0 or 1",
            CodecError::OddPcm => "PCM byte length is odd",
            CodecError::TooLong => "audio frame is longer than 4800 samples",
        })
    }
}

pub fn encode_pcm(
    dir: AudioDir,
    rate: RateCode,
    seq: u16,
    pcm: &[i16],
) -> Result<Vec<u8>, CodecError> {
    if pcm.len() > MAX_SAMPLES {
        return Err(CodecError::TooLong);
    }
    let mut out = Vec::with_capacity(HEADER_LEN + pcm.len() * 2);
    out.extend_from_slice(&[b'G', b'S', 1, dir as u8, rate as u8, 0]);
    out.extend_from_slice(&seq.to_le_bytes());
    for s in pcm {
        out.extend_from_slice(&s.to_le_bytes());
    }
    Ok(out)
}

pub fn decode_pcm(frame: &[u8]) -> Result<(AudioDir, RateCode, u16, Vec<i16>), CodecError> {
    if frame.len() < HEADER_LEN {
        return Err(CodecError::Short);
    }
    if frame[0] != b'G' || frame[1] != b'S' {
        return Err(CodecError::Magic);
    }
    if frame[2] != 1 {
        return Err(CodecError::Version);
    }
    let dir = match frame[3] {
        0 => AudioDir::Mic,
        1 => AudioDir::Speaker,
        _ => return Err(CodecError::Dir),
    };
    let rate = match frame[4] {
        0 => RateCode::Hz24k,
        1 => RateCode::Hz16k,
        _ => return Err(CodecError::Rate),
    };
    if frame.len() % 2 != 0 {
        return Err(CodecError::OddPcm);
    }
    let n = (frame.len() - HEADER_LEN) / 2;
    if n > MAX_SAMPLES {
        return Err(CodecError::TooLong);
    }
    let seq = u16::from_le_bytes([frame[6], frame[7]]);
    let pcm = frame[HEADER_LEN..]
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect();
    Ok((dir, rate, seq, pcm))
}

pub fn welcome_json() -> String {
    format!(
        "{{\"schema\":\"{SCHEMA}\",\"type\":\"welcome\",\"frame_ms\":{FRAME_MS},\"sample_rate\":{SPEAKER_RATE},\"dot\":\"idle\"}}"
    )
}

pub fn error_json(error: &str) -> String {
    format!(
        "{{\"schema\":\"{SCHEMA}\",\"type\":\"error\",\"error\":\"{}\"}}",
        escape(&redact_error(error))
    )
}

pub fn redact_error(error: &str) -> String {
    let lower = error.to_ascii_lowercase();
    if lower.contains("pcm") || lower.contains("transcript") {
        return "redacted".into();
    }
    let code: String = error
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .take(40)
        .collect();
    if code.is_empty() {
        "error".into()
    } else {
        code
    }
}

/// `source` is `brain` when a linked brain tick returned a pad, otherwise `resting`.
pub fn mood_json(t_ms: u64, source: &str, pad: Pad) -> String {
    let source = if source == "brain" {
        "brain"
    } else {
        "resting"
    };
    format!(
        "{{\"schema\":\"{SCHEMA}\",\"type\":\"mood\",\"t_ms\":{t_ms},\"source\":\"{source}\",\"pad\":{}}}",
        pad_json(pad)
    )
}

pub fn pad_json(pad: Pad) -> String {
    let pad = pad.finite_or_resting();
    format!(
        "{{\"arousal\":{},\"valence\":{},\"dominance\":{}}}",
        fmt_f32(pad.arousal),
        fmt_f32(pad.valence),
        fmt_f32(pad.dominance)
    )
}

fn fmt_f32(x: f32) -> String {
    let x = if x.is_finite() { x } else { 0.0 };
    let s = format!("{x}");
    if s == "-0" {
        "0".into()
    } else {
        s
    }
}

pub fn dot_json(state: &str, level: f32) -> String {
    let state = match state {
        "listening" => "listening",
        "speaking" => "speaking",
        _ => "idle",
    };
    let level = if level.is_finite() {
        level.clamp(0.0, 1.0)
    } else {
        0.0
    };
    format!(
        "{{\"schema\":\"{SCHEMA}\",\"type\":\"dot\",\"state\":\"{state}\",\"level\":{}}}",
        fmt_f32(level)
    )
}

pub fn vad_json(event: &str, t_ms: u64) -> String {
    let event = if event == "start" { "start" } else { "end" };
    format!("{{\"schema\":\"{SCHEMA}\",\"type\":\"vad\",\"event\":\"{event}\",\"t_ms\":{t_ms}}}")
}

pub fn pong_json() -> String {
    format!("{{\"schema\":\"{SCHEMA}\",\"type\":\"pong\"}}")
}

/// Idle thought. `verb` is the outbound intent token. `lean` is the dialogue
/// lean. `slots` are motive tokens. There is no sentence in this frame.
pub fn inner_json(
    verb: &str,
    lean: &str,
    surprise: f32,
    learned: bool,
    slots: &[String],
) -> String {
    serde_json::json!({
        "schema": SCHEMA,
        "type": "inner",
        "verb": verb,
        "lean": lean,
        "surprise": surprise,
        "learned": learned,
        "slots": slots,
    })
    .to_string()
}

pub fn turn_json(turn: &crate::reply::TurnOut) -> String {
    format!(
        "{{\"schema\":\"{SCHEMA}\",\"type\":\"turn\",\"reply\":\"{}\",\"intent\":{},\"arm\":{},\"propensity\":{},\"evidence\":{},\"sources\":{},\"violations\":{},\"lookup_us\":{},\"took_us\":{},\"mood\":{},\"empathy_e\":{},\"prosody\":{{\"rate\":{},\"energy\":{},\"pitch_st\":{},\"pause_ms\":{}}}}}",
        escape(&turn.reply),
        json_or_null(&turn.intent_json),
        json_or_null(&turn.arm_json),
        fmt_f32(if turn.propensity.is_finite() { turn.propensity } else { 0.0 }),
        string_array(&turn.evidence),
        string_array(&turn.sources),
        turn.violations,
        turn.lookup_us,
        turn.took_us,
        pad_json(turn.mood),
        fmt_f32(turn.empathy_e),
        fmt_f32(turn.prosody_rate),
        fmt_f32(turn.prosody_energy),
        fmt_f32(turn.prosody_pitch_st),
        fmt_f32(turn.prosody_pause_ms)
    )
}

/// One spoken piece on the socket the dot page already shows.
/// `reply` is `TurnOut.first_clause` or `TurnOut.rest`. There is no PCM.
pub fn spoken_text_json(text: &str, mood: Pad) -> String {
    format!(
        "{{\"schema\":\"{SCHEMA}\",\"type\":\"turn\",\"reply\":\"{}\",\"intent\":null,\"arm\":null,\"propensity\":0,\"evidence\":[],\"sources\":[],\"violations\":0,\"lookup_us\":0,\"took_us\":0,\"mood\":{},\"empathy_e\":0,\"prosody\":{{\"rate\":1,\"energy\":1,\"pitch_st\":0,\"pause_ms\":0}}}}",
        escape(text),
        pad_json(mood)
    )
}

fn json_or_null(raw: &str) -> String {
    let s = raw.trim();
    if s.is_empty() {
        "null".into()
    } else if serde_json::from_str::<Value>(s).is_ok() {
        s.to_string()
    } else {
        "null".into()
    }
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

#[derive(Debug, Clone, PartialEq)]
pub enum ClientControl {
    Hello {
        session: Option<String>,
    },
    Ping,
    Stop,
    Text {
        text: String,
    },
    Consent {
        sense: Option<bool>,
        adapt: Option<bool>,
        remember: Option<bool>,
        share: Option<bool>,
    },
    Unknown {
        kind: String,
    },
}

pub fn parse_client(text: &str) -> Result<ClientControl, String> {
    let v: Value = serde_json::from_str(text).map_err(|_| "json".to_string())?;
    let kind = v.get("type").and_then(Value::as_str).unwrap_or("");
    match kind {
        "hello" => {
            let session = match v.get("session") {
                None | Some(Value::Null) => None,
                Some(Value::String(s)) => Some(s.clone()),
                _ => return Err("session".into()),
            };
            Ok(ClientControl::Hello { session })
        }
        "ping" => Ok(ClientControl::Ping),
        "stop" => Ok(ClientControl::Stop),
        "text" => {
            let text = v
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            if text.chars().count() > 2_000 {
                return Err("text".into());
            }
            Ok(ClientControl::Text { text })
        }
        "consent" => Ok(ClientControl::Consent {
            sense: v.get("sense").and_then(Value::as_bool),
            adapt: v.get("adapt").and_then(Value::as_bool),
            remember: v.get("remember").and_then(Value::as_bool),
            share: v.get("share").and_then(Value::as_bool),
        }),
        "" => Err("type".into()),
        other => Ok(ClientControl::Unknown {
            kind: other
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
                .take(40)
                .collect(),
        }),
    }
}

pub fn query_carries_token(query: Option<&str>) -> bool {
    let Some(q) = query else { return false };
    for pair in q.split('&') {
        let key = pair.split('=').next().unwrap_or("");
        let key = key.trim();
        if key == "token" || key == "session" {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pcm_roundtrip() {
        let pcm = [0, 1, -1, 32767];
        let bytes = encode_pcm(AudioDir::Speaker, RateCode::Hz24k, 7, &pcm).unwrap();
        let (dir, rate, seq, back) = decode_pcm(&bytes).unwrap();
        assert_eq!(dir, AudioDir::Speaker);
        assert_eq!(rate.hz(), 24_000);
        assert_eq!(seq, 7);
        assert_eq!(back, pcm);
        assert!(decode_pcm(&[0, 1]).is_err());
    }

    #[test]
    fn welcome_matches_the_architecture_string() {
        assert_eq!(
            welcome_json(),
            "{\"schema\":\"galion-samantha-ws/1\",\"type\":\"welcome\",\"frame_ms\":80,\"sample_rate\":24000,\"dot\":\"idle\"}"
        );
        let mood = mood_json(0, "resting", Pad::RESTING);
        assert!(mood.contains("\"source\":\"resting\""));
        assert!(mood.contains("\"valence\":0.25"));
        assert!(mood.contains("\"arousal\":0.3"));
    }

    #[test]
    fn error_does_not_carry_audio_or_a_transcript() {
        assert_eq!(redact_error("pcm dump"), "redacted");
        assert_eq!(redact_error("see transcript"), "redacted");
        assert!(error_json("brain-unlinked").contains("brain-unlinked"));
    }

    #[test]
    fn inner_frame_is_tokens_only() {
        let line = inner_json(
            "answer",
            "hold",
            0.0,
            false,
            &["heard".into(), "pause".into()],
        );
        let parsed: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(parsed["type"], "inner");
        assert_eq!(parsed["verb"], "answer");
        assert_eq!(parsed["lean"], "hold");
        assert_eq!(parsed["slots"][0], "heard");
        assert!(parsed.get("reply").is_none());
    }

    #[test]
    fn query_token_is_rejected() {
        assert!(query_carries_token(Some("token=abc")));
        assert!(query_carries_token(Some("session=abc")));
        assert!(!query_carries_token(Some("x=1")));
        assert!(!query_carries_token(None));
    }
}
