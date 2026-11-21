// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Wire codec for `/v1/stream`. Binary audio uses a 20-byte header plus
//! little-endian s16 PCM. Control and state travel as one JSON object per
//! text frame. The layout is specified in `PROTOCOL.md`.

use serde_json::{json, Value};

use crate::audio::{FRAME_MS, MIC_RATE, TTS_RATE};
use crate::reply::{Pad, TurnOut};

pub const PROTOCOL: &str = "galion-samantha-local/1";
pub const HEADER_LEN: usize = 20;
pub const MAGIC: [u8; 4] = *b"G1PC";
pub const VERSION: u8 = 1;
pub const KIND_MIC: u8 = 1;
pub const KIND_TTS: u8 = 2;
pub const FLAG_START: u16 = 1;
pub const FLAG_END: u16 = 2;
pub const FLAG_BARGE: u16 = 4;
pub const MAX_SAMPLES: usize = 4096;
pub const MAX_TEXT_CHARS: usize = 2_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioFrame {
    pub kind: u8,
    pub flags: u16,
    pub sample_rate: u32,
    pub sequence: u32,
    pub pcm: Vec<i16>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecError {
    Short,
    Magic,
    Version,
    Kind,
    Channels,
    Reserved,
    Length,
    TooLong,
}

impl std::fmt::Display for CodecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            CodecError::Short => "frame shorter than 20 bytes",
            CodecError::Magic => "bad magic",
            CodecError::Version => "unsupported version",
            CodecError::Kind => "unknown kind",
            CodecError::Channels => "only mono is accepted",
            CodecError::Reserved => "reserved bytes must be zero",
            CodecError::Length => "pcm byte length is odd",
            CodecError::TooLong => "frame has more than 4096 samples",
        };
        f.write_str(msg)
    }
}

pub fn encode_frame(frame: &AudioFrame) -> Result<Vec<u8>, CodecError> {
    if frame.kind != KIND_MIC && frame.kind != KIND_TTS {
        return Err(CodecError::Kind);
    }
    if frame.pcm.len() > MAX_SAMPLES {
        return Err(CodecError::TooLong);
    }
    let mut out = Vec::with_capacity(HEADER_LEN + frame.pcm.len() * 2);
    out.extend_from_slice(&MAGIC);
    out.push(VERSION);
    out.push(frame.kind);
    out.extend_from_slice(&frame.flags.to_le_bytes());
    out.extend_from_slice(&frame.sample_rate.to_le_bytes());
    out.extend_from_slice(&frame.sequence.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    for s in &frame.pcm {
        out.extend_from_slice(&s.to_le_bytes());
    }
    Ok(out)
}

pub fn decode_frame(bytes: &[u8]) -> Result<AudioFrame, CodecError> {
    if bytes.len() < HEADER_LEN {
        return Err(CodecError::Short);
    }
    if bytes[0..4] != MAGIC {
        return Err(CodecError::Magic);
    }
    if bytes[4] != VERSION {
        return Err(CodecError::Version);
    }
    let kind = bytes[5];
    if kind != KIND_MIC && kind != KIND_TTS {
        return Err(CodecError::Kind);
    }
    let flags = u16::from_le_bytes([bytes[6], bytes[7]]);
    let sample_rate = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
    let sequence = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
    let channels = u16::from_le_bytes([bytes[16], bytes[17]]);
    let reserved = u16::from_le_bytes([bytes[18], bytes[19]]);
    if channels != 1 {
        return Err(CodecError::Channels);
    }
    if reserved != 0 {
        return Err(CodecError::Reserved);
    }
    let pcm_bytes = &bytes[HEADER_LEN..];
    if pcm_bytes.len() % 2 != 0 {
        return Err(CodecError::Length);
    }
    let n = pcm_bytes.len() / 2;
    if n > MAX_SAMPLES {
        return Err(CodecError::TooLong);
    }
    let mut pcm = Vec::with_capacity(n);
    for chunk in pcm_bytes.chunks_exact(2) {
        pcm.push(i16::from_le_bytes([chunk[0], chunk[1]]));
    }
    Ok(AudioFrame {
        kind,
        flags,
        sample_rate,
        sequence,
        pcm,
    })
}

#[derive(Debug, Clone, PartialEq)]
pub enum ClientControl {
    Hello,
    Ping { t_ms: u64 },
    Barge,
    Text { text: String },
    Unknown { kind: String },
}

pub fn parse_client(text: &str) -> Result<ClientControl, String> {
    let v: Value = serde_json::from_str(text).map_err(|e| format!("json: {e}"))?;
    let kind = v
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    match kind.as_str() {
        "hello" => Ok(ClientControl::Hello),
        "ping" => Ok(ClientControl::Ping {
            t_ms: v.get("t_ms").and_then(Value::as_u64).unwrap_or(0),
        }),
        "barge" => Ok(ClientControl::Barge),
        "text" => {
            let text = v
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            if text.chars().count() > MAX_TEXT_CHARS {
                return Err(format!("text longer than {MAX_TEXT_CHARS} chars"));
            }
            Ok(ClientControl::Text { text })
        }
        "" => Err("missing type".into()),
        other => Ok(ClientControl::Unknown {
            kind: other.to_string(),
        }),
    }
}

pub fn ready_json(ear: &str, mouth: &str, mind: &str) -> Value {
    json!({
        "type": "ready",
        "protocol": PROTOCOL,
        "bind": "127.0.0.1",
        "mic_rate": MIC_RATE,
        "tts_rate": TTS_RATE,
        "frame_ms": FRAME_MS,
        "mic_frame_samples": crate::audio::MIC_FRAME_SAMPLES,
        "mood_rest": {
            "arousal": Pad::RESTING.arousal,
            "valence": Pad::RESTING.valence,
            "dominance": Pad::RESTING.dominance,
        },
        "ear": ear,
        "mouth": mouth,
        "mind": mind,
    })
}

pub fn state_json(state: &str, reason: Option<&str>) -> Value {
    let mut v = json!({ "type": "state", "state": state });
    if let Some(reason) = reason {
        v["reason"] = json!(reason);
    }
    v
}

pub fn mood_json(pad: Pad, tick: u64, state: &str) -> Value {
    json!({
        "type": "mood",
        "arousal": pad.arousal,
        "valence": pad.valence,
        "dominance": pad.dominance,
        "tick": tick,
        "frame_ms": FRAME_MS,
        "state": state,
    })
}

pub fn heard_json(text: &str, ear: &str) -> Value {
    json!({
        "type": "heard",
        "text": text,
        "ear": ear,
        "recognized": false,
    })
}

pub fn reply_json(out: &TurnOut, mouth: &str, mouth_us: u32) -> Value {
    json!({
        "type": "reply",
        "text": out.reply,
        "intent": out.intent,
        "arm": out.arm,
        "propensity": out.propensity,
        "evidence": out.evidence,
        "sources": out.sources,
        "violations": out.violations,
        "lookup_us": out.lookup_us,
        "took_us": out.took_us,
        "mouth_us": mouth_us,
        "mouth": mouth,
        "mood": {
            "arousal": out.mood.arousal,
            "valence": out.mood.valence,
            "dominance": out.mood.dominance,
        },
    })
}

pub fn error_json(error: &str) -> Value {
    json!({ "type": "error", "error": error })
}

pub fn pong_json(echo_ms: u64) -> Value {
    json!({ "type": "pong", "echo_ms": echo_ms })
}

pub fn hello_json() -> Value {
    json!({ "type": "hello", "protocol": PROTOCOL })
}

/// Stamp `t_ms` from the start of the socket and render one line.
pub fn stamp(mut v: Value, t_ms: u64) -> String {
    if let Some(obj) = v.as_object_mut() {
        obj.insert("t_ms".into(), json!(t_ms));
    }
    v.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> AudioFrame {
        AudioFrame {
            kind: KIND_MIC,
            flags: FLAG_START,
            sample_rate: MIC_RATE,
            sequence: 7,
            pcm: vec![1, -2, 3, 0],
        }
    }

    #[test]
    fn roundtrip() {
        let frame = sample();
        let bytes = encode_frame(&frame).unwrap();
        assert_eq!(decode_frame(&bytes).unwrap(), frame);
    }

    #[test]
    fn rejects_bad_frames() {
        assert_eq!(decode_frame(&[0, 1, 2]).unwrap_err(), CodecError::Short);
        let mut bytes = encode_frame(&sample()).unwrap();
        bytes[0] = b'X';
        assert_eq!(decode_frame(&bytes).unwrap_err(), CodecError::Magic);
        let mut bytes = encode_frame(&sample()).unwrap();
        bytes[4] = 9;
        assert_eq!(decode_frame(&bytes).unwrap_err(), CodecError::Version);
        let mut bytes = encode_frame(&sample()).unwrap();
        bytes[5] = 9;
        assert_eq!(decode_frame(&bytes).unwrap_err(), CodecError::Kind);
        let mut bytes = encode_frame(&sample()).unwrap();
        bytes[16] = 2;
        assert_eq!(decode_frame(&bytes).unwrap_err(), CodecError::Channels);
        let mut bytes = encode_frame(&sample()).unwrap();
        bytes[18] = 1;
        assert_eq!(decode_frame(&bytes).unwrap_err(), CodecError::Reserved);
        let mut bytes = encode_frame(&sample()).unwrap();
        bytes.push(0);
        assert_eq!(decode_frame(&bytes).unwrap_err(), CodecError::Length);
        let big = AudioFrame {
            pcm: vec![0; MAX_SAMPLES + 1],
            ..sample()
        };
        assert_eq!(encode_frame(&big).unwrap_err(), CodecError::TooLong);
    }

    #[test]
    fn client_controls() {
        assert_eq!(
            parse_client(r#"{"type":"barge"}"#).unwrap(),
            ClientControl::Barge
        );
        assert_eq!(
            parse_client(r#"{"type":"ping","t_ms":5}"#).unwrap(),
            ClientControl::Ping { t_ms: 5 }
        );
        assert_eq!(
            parse_client(r#"{"type":"text","text":"hi"}"#).unwrap(),
            ClientControl::Text { text: "hi".into() }
        );
        assert!(matches!(
            parse_client(r#"{"type":"nope"}"#).unwrap(),
            ClientControl::Unknown { .. }
        ));
        assert!(parse_client("not json").is_err());
        let huge = format!(
            r#"{{"type":"text","text":"{}"}}"#,
            "a".repeat(MAX_TEXT_CHARS + 1)
        );
        assert!(parse_client(&huge).is_err());
    }

    #[test]
    fn heard_never_claims_recognition() {
        let v = heard_json("hello", "scripted");
        assert_eq!(v["recognized"], false);
    }
}
