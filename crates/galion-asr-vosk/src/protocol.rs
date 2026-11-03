// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! NDJSON protocol between PhoneLane and `galion-asr-vosk`.
//!
//! One request line, then zero or more `partial` lines, then one `final`
//! or one `error`. Audio is 16 kHz mono PCM16, base64, fed to libvosk in
//! 20 ms frames (320 samples) inside the worker. This module does not
//! recognise speech and does not invent words.

use crate::json::{parse, to_line, Json};

/// Protocol name on the ready line.
pub const PROTOCOL: &str = "galion-asr-vosk/1";
/// 20 ms at 16 kHz.
pub const FRAME_SAMPLES: usize = 320;
/// Phone-path rate. The worker rejects anything else.
pub const SAMPLE_RATE: u32 = 16_000;
/// Default model directory, relative to the repo root. Not vendored.
pub const DEFAULT_MODEL_DIR: &str = "weights/asr/vosk-model-small-en-us-0.15";
/// Default libvosk shared object. Loaded only inside the worker process.
pub const DEFAULT_LIB: &str = "weights/asr/libvosk/libvosk.so";
/// Refuse an utterance longer than this rather than guessing.
pub const MAX_UTTERANCE_SAMPLES: usize = 60 * SAMPLE_RATE as usize;

/// One decoded word (`set_words` on the recognizer).
#[derive(Debug, Clone, PartialEq)]
pub struct Word {
    pub word: String,
    pub start: f32,
    pub end: f32,
    pub conf: f32,
}

/// A line the worker writes.
#[derive(Debug, Clone, PartialEq)]
pub enum WorkerEvent {
    Ready {
        model: String,
        frame_samples: u32,
    },
    Partial {
        text: String,
        t_ms: u32,
    },
    Final {
        text: String,
        t_ms: u32,
        words: Vec<Word>,
    },
    Error {
        error: String,
    },
}

/// One utterance the parent writes.
#[derive(Debug, Clone, PartialEq)]
pub struct Utterance {
    pub rate: u32,
    pub pcm: Vec<i16>,
    pub grammar: Option<Vec<String>>,
}

/// Final text the parent keeps. `[unk]` is not a word we will speak back as if heard.
pub fn clean_transcript(text: &str) -> String {
    text.split_whitespace()
        .filter(|w| *w != "[unk]" && *w != "<unk>")
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn model_dir_ok(path: &std::path::Path) -> bool {
    path.is_dir() && path.join("am/final.mdl").is_file() && path.join("conf/model.conf").is_file()
}

pub fn encode_ready(model: &str) -> String {
    to_line(&Json::Obj(vec![
        ("type".into(), Json::Str("ready".into())),
        ("protocol".into(), Json::Str(PROTOCOL.into())),
        ("model".into(), Json::Str(model.into())),
        ("frame_samples".into(), Json::Num(FRAME_SAMPLES as f64)),
    ]))
}

pub fn encode_error(error: &str) -> String {
    to_line(&Json::Obj(vec![
        ("type".into(), Json::Str("error".into())),
        ("error".into(), Json::Str(error.into())),
    ]))
}

pub fn encode_partial(text: &str, t_ms: u32) -> String {
    to_line(&Json::Obj(vec![
        ("type".into(), Json::Str("partial".into())),
        ("text".into(), Json::Str(text.into())),
        ("t_ms".into(), Json::Num(t_ms as f64)),
    ]))
}

pub fn encode_final(text: &str, t_ms: u32, words: &[Word]) -> String {
    let words = words
        .iter()
        .map(|w| {
            Json::Obj(vec![
                ("word".into(), Json::Str(w.word.clone())),
                ("start".into(), Json::Num(w.start as f64)),
                ("end".into(), Json::Num(w.end as f64)),
                ("conf".into(), Json::Num(w.conf as f64)),
            ])
        })
        .collect();
    to_line(&Json::Obj(vec![
        ("type".into(), Json::Str("final".into())),
        ("text".into(), Json::Str(text.into())),
        ("t_ms".into(), Json::Num(t_ms as f64)),
        ("words".into(), Json::Arr(words)),
    ]))
}

pub fn encode_utterance(rate: u32, pcm: &[i16], grammar: Option<&[String]>) -> String {
    let mut raw = Vec::with_capacity(pcm.len() * 2);
    for s in pcm {
        raw.extend_from_slice(&s.to_le_bytes());
    }
    let mut kv = vec![
        ("cmd".into(), Json::Str("utterance".into())),
        ("rate".into(), Json::Num(rate as f64)),
        ("pcm_b64".into(), Json::Str(b64_encode(&raw))),
    ];
    if let Some(g) = grammar {
        kv.push((
            "grammar".into(),
            Json::Arr(g.iter().cloned().map(Json::Str).collect()),
        ));
    }
    to_line(&Json::Obj(kv))
}

pub fn encode_grammar(phrases: &[String]) -> String {
    to_line(&Json::Arr(phrases.iter().cloned().map(Json::Str).collect()))
}

pub fn parse_event(line: &str) -> Result<WorkerEvent, String> {
    let v = parse(line.trim())?;
    let kind = v.get("type").and_then(Json::as_str).unwrap_or("");
    match kind {
        "ready" => Ok(WorkerEvent::Ready {
            model: v
                .get("model")
                .and_then(Json::as_str)
                .unwrap_or("")
                .to_string(),
            frame_samples: v
                .get("frame_samples")
                .and_then(Json::as_f64)
                .unwrap_or(FRAME_SAMPLES as f64) as u32,
        }),
        "partial" => Ok(WorkerEvent::Partial {
            text: v
                .get("text")
                .and_then(Json::as_str)
                .unwrap_or("")
                .to_string(),
            t_ms: num_u32(v.get("t_ms")),
        }),
        "final" => Ok(WorkerEvent::Final {
            text: v
                .get("text")
                .and_then(Json::as_str)
                .unwrap_or("")
                .to_string(),
            t_ms: num_u32(v.get("t_ms")),
            words: words_of(v.get("words")),
        }),
        "error" => Ok(WorkerEvent::Error {
            error: v
                .get("error")
                .and_then(Json::as_str)
                .unwrap_or("vosk worker error")
                .to_string(),
        }),
        other => Err(format!("unknown worker event {other}")),
    }
}

pub fn parse_utterance(line: &str) -> Result<Utterance, String> {
    let v = parse(line.trim())?;
    let cmd = v.get("cmd").and_then(Json::as_str).unwrap_or("");
    if cmd != "utterance" {
        return Err(format!("expected cmd utterance, got {cmd}"));
    }
    let rate = v.get("rate").and_then(Json::as_f64).unwrap_or(0.0) as u32;
    let b64 = v
        .get("pcm_b64")
        .and_then(Json::as_str)
        .ok_or("missing pcm_b64")?;
    let raw = b64_decode(b64)?;
    if raw.len() % 2 != 0 {
        return Err("pcm byte length is odd".into());
    }
    let pcm: Vec<i16> = raw
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect();
    if pcm.len() > MAX_UTTERANCE_SAMPLES {
        return Err(format!(
            "utterance is {} samples, over the {} sample cap. No transcript was invented.",
            pcm.len(),
            MAX_UTTERANCE_SAMPLES
        ));
    }
    let grammar = match v.get("grammar") {
        None => None,
        Some(g) => {
            let arr = g.as_arr().ok_or("grammar must be an array of strings")?;
            let mut phrases = Vec::with_capacity(arr.len());
            for item in arr {
                phrases.push(
                    item.as_str()
                        .ok_or("grammar phrase must be a string")?
                        .to_string(),
                );
            }
            Some(phrases)
        }
    };
    Ok(Utterance { rate, pcm, grammar })
}

/// Pull `text` (final) or `partial` out of a libvosk JSON blob.
pub fn vosk_text(blob: &str, key: &str) -> Result<String, String> {
    let v = parse(blob.trim())?;
    Ok(v.get(key).and_then(Json::as_str).unwrap_or("").to_string())
}

pub fn vosk_words(blob: &str) -> Result<(String, Vec<Word>), String> {
    let v = parse(blob.trim())?;
    let text = v
        .get("text")
        .and_then(Json::as_str)
        .unwrap_or("")
        .to_string();
    Ok((text, words_of(v.get("result"))))
}

fn words_of(v: Option<&Json>) -> Vec<Word> {
    let Some(arr) = v.and_then(Json::as_arr) else {
        return Vec::new();
    };
    let mut words = Vec::new();
    for item in arr {
        let Some(word) = item.get("word").and_then(Json::as_str) else {
            continue;
        };
        if word.is_empty() {
            continue;
        }
        words.push(Word {
            word: word.to_string(),
            start: item.get("start").and_then(Json::as_f64).unwrap_or(0.0) as f32,
            end: item.get("end").and_then(Json::as_f64).unwrap_or(0.0) as f32,
            conf: item.get("conf").and_then(Json::as_f64).unwrap_or(0.0) as f32,
        });
    }
    words
}

fn num_u32(v: Option<&Json>) -> u32 {
    v.and_then(Json::as_f64).unwrap_or(0.0).max(0.0) as u32
}

pub fn b64_encode(data: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    let mut i = 0;
    while i + 3 <= data.len() {
        let n = ((data[i] as u32) << 16) | ((data[i + 1] as u32) << 8) | data[i + 2] as u32;
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(T[((n >> 6) & 63) as usize] as char);
        out.push(T[(n & 63) as usize] as char);
        i += 3;
    }
    if i < data.len() {
        let a = data[i] as u32;
        let b = if i + 1 < data.len() {
            data[i + 1] as u32
        } else {
            0
        };
        let n = (a << 16) | (b << 8);
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        if i + 1 < data.len() {
            out.push(T[((n >> 6) & 63) as usize] as char);
            out.push('=');
        } else {
            out.push('=');
            out.push('=');
        }
    }
    out
}

pub fn b64_decode(s: &str) -> Result<Vec<u8>, String> {
    fn val(c: u8) -> Result<u8, String> {
        match c {
            b'A'..=b'Z' => Ok(c - b'A'),
            b'a'..=b'z' => Ok(c - b'a' + 26),
            b'0'..=b'9' => Ok(c - b'0' + 52),
            b'+' => Ok(62),
            b'/' => Ok(63),
            _ => Err("bad base64".into()),
        }
    }
    let bytes = s.as_bytes();
    if bytes.len() % 4 != 0 {
        return Err("bad base64 length".into());
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    let mut i = 0;
    while i < bytes.len() {
        let (a, b, c, d) = (bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]);
        let pad = (c == b'=') as usize + (d == b'=') as usize;
        if c == b'=' && d != b'=' {
            return Err("bad base64 padding".into());
        }
        let av = val(a)?;
        let bv = val(b)?;
        let cv = if c == b'=' { 0 } else { val(c)? };
        let dv = if d == b'=' { 0 } else { val(d)? };
        let n = ((av as u32) << 18) | ((bv as u32) << 12) | ((cv as u32) << 6) | dv as u32;
        out.push((n >> 16) as u8);
        if pad < 2 {
            out.push((n >> 8) as u8);
        }
        if pad < 1 {
            out.push(n as u8);
        }
        i += 4;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utterance_roundtrip_keeps_samples_and_grammar() {
        let pcm = vec![0i16, -1, 32767, -32768];
        let grammar = vec!["thanks".to_string(), "[unk]".to_string()];
        let line = encode_utterance(16_000, &pcm, Some(&grammar));
        assert!(!line.contains('\n'));
        let back = parse_utterance(&line).unwrap();
        assert_eq!(back.rate, 16_000);
        assert_eq!(back.pcm, pcm);
        assert_eq!(back.grammar.unwrap(), grammar);
        let free = parse_utterance(&encode_utterance(16_000, &pcm, None)).unwrap();
        assert!(free.grammar.is_none());
    }

    #[test]
    fn events_roundtrip_and_clean_drops_unk() {
        let ready = parse_event(&encode_ready("weights/asr/m")).unwrap();
        assert_eq!(
            ready,
            WorkerEvent::Ready {
                model: "weights/asr/m".into(),
                frame_samples: FRAME_SAMPLES as u32
            }
        );
        assert_eq!(FRAME_SAMPLES, 320);
        let words = vec![Word {
            word: "thanks".into(),
            start: 0.1,
            end: 0.4,
            conf: 0.9,
        }];
        let ev = parse_event(&encode_final("thanks [unk]", 40, &words)).unwrap();
        match ev {
            WorkerEvent::Final { text, t_ms, words } => {
                assert_eq!((text.as_str(), t_ms), ("thanks [unk]", 40));
                assert_eq!(words[0].word, "thanks");
                assert_eq!(clean_transcript(&text), "thanks");
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(clean_transcript("[unk]"), "");
        assert_eq!(clean_transcript("  hi   there "), "hi there");
        let err = parse_event(&encode_error("model missing")).unwrap();
        assert!(matches!(err, WorkerEvent::Error { error } if error == "model missing"));
    }

    #[test]
    fn base64_known_vector_and_odd_tail() {
        assert_eq!(b64_encode(&[0x00, 0x01, 0xff]), "AAH/");
        assert_eq!(b64_decode("AAH/").unwrap(), vec![0x00, 0x01, 0xff]);
        assert_eq!(b64_decode(&b64_encode(b"hi")).unwrap(), b"hi");
        assert!(b64_decode("abc").is_err());
    }

    #[test]
    fn vosk_blob_words() {
        let blob =
            r#"{"result":[{"conf":1.0,"end":1.11,"start":0.87,"word":"what"}],"text":"what"}"#;
        let (text, words) = vosk_words(blob).unwrap();
        assert_eq!(text, "what");
        assert_eq!(words[0].word, "what");
        assert!((words[0].start - 0.87).abs() < 1e-4);
        assert_eq!(vosk_text(r#"{"partial":"wh"}"#, "partial").unwrap(), "wh");
    }

    #[test]
    fn model_dir_needs_both_files() {
        let dir = std::env::temp_dir().join(format!("galion-vosk-model-ok-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("am")).unwrap();
        std::fs::create_dir_all(dir.join("conf")).unwrap();
        assert!(!model_dir_ok(&dir));
        std::fs::write(dir.join("am/final.mdl"), b"x").unwrap();
        assert!(!model_dir_ok(&dir));
        std::fs::write(dir.join("conf/model.conf"), b"x").unwrap();
        assert!(model_dir_ok(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
