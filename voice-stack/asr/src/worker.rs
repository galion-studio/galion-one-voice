// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Adapter for the out-of-process `galion-asr-vosk` worker (protocol
//! `galion-asr-vosk/1`, PR #26). The worker accepts one whole utterance, so
//! `push_pcm` only buffers. Partials and the final come back from `finish`.

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};

use crate::b64::{b64_decode, b64_encode};
use crate::engine::Asr;
use crate::error::AsrError;
use crate::event::{samples_to_ms, AsrEvent, AsrKind, Word, SAMPLE_RATE};
use crate::json::{parse, to_line, Json};
use crate::paths::env_u32;
use crate::text::clean_transcript;

pub struct VoskWorkerAsr {
    child: Child,
    stdin: ChildStdin,
    reader: BufReader<std::process::ChildStdout>,
    pending: Vec<i16>,
    label: String,
}

impl VoskWorkerAsr {
    pub fn open(bin: &Path, model: &Path, lib: Option<&Path>) -> Result<Self, AsrError> {
        if !bin.is_file() {
            return Err(AsrError::new(format!(
                "galion-asr-vosk binary not found at {}. Build it with `cargo build -p galion-asr-vosk --bin galion-asr-vosk`. No transcript was invented.",
                bin.display()
            )));
        }
        let threads = env_u32("GALION_VOSK_THREADS", 2)?;
        let mut cmd = Command::new(bin);
        cmd.arg("--model").arg(model);
        if let Some(lib) = lib {
            cmd.arg("--lib").arg(lib);
            if let Some(dir) = lib.parent() {
                prepend_child_lib(&mut cmd, dir);
            }
        }
        if std::env::var_os("OMP_NUM_THREADS").is_none() {
            cmd.env("OMP_NUM_THREADS", threads.to_string());
        }
        if std::env::var_os("OPENBLAS_NUM_THREADS").is_none() {
            cmd.env("OPENBLAS_NUM_THREADS", threads.to_string());
        }
        Self::from_command(cmd)
    }

    pub fn from_command(mut cmd: Command) -> Result<Self, AsrError> {
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        let mut child = cmd.spawn().map_err(|err| {
            AsrError::new(format!(
                "spawn vosk worker: {err}. No transcript was invented."
            ))
        })?;
        let stdin = child.stdin.take().ok_or_else(|| {
            AsrError::new("vosk worker stdin missing. No transcript was invented.")
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            AsrError::new("vosk worker stdout missing. No transcript was invented.")
        })?;
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        let read = reader.read_line(&mut line);
        let ready = match read {
            Ok(0) => Err(AsrError::new(
                "vosk worker closed before ready. No transcript was invented.",
            )),
            Ok(_) => parse_event(&line),
            Err(err) => Err(AsrError::new(format!(
                "vosk worker ready: {err}. No transcript was invented."
            ))),
        };
        match ready {
            Ok(WorkerEvent::Ready { model }) => Ok(Self {
                child,
                stdin,
                reader,
                pending: Vec::new(),
                label: format!("vosk-worker:{model}"),
            }),
            Ok(WorkerEvent::Error { error }) => {
                let _ = child.kill();
                let _ = child.wait();
                Err(AsrError::new(error))
            }
            Ok(other) => {
                let _ = child.kill();
                let _ = child.wait();
                Err(AsrError::new(format!(
                    "vosk worker handshake was {other:?}. No transcript was invented."
                )))
            }
            Err(err) => {
                let _ = child.kill();
                let _ = child.wait();
                Err(err)
            }
        }
    }
}

impl Asr for VoskWorkerAsr {
    fn push_pcm(&mut self, pcm: &[i16]) -> Result<Vec<AsrEvent>, AsrError> {
        self.pending.extend_from_slice(pcm);
        Ok(Vec::new())
    }

    fn finish(&mut self) -> Result<Vec<AsrEvent>, AsrError> {
        if self.pending.is_empty() {
            return Ok(Vec::new());
        }
        let pcm = std::mem::take(&mut self.pending);
        let audio_ms = samples_to_ms(pcm.len());
        let line = encode_utterance(SAMPLE_RATE, &pcm);
        if let Err(err) = self
            .stdin
            .write_all(line.as_bytes())
            .and_then(|_| self.stdin.flush())
        {
            self.pending = pcm;
            return Err(AsrError::new(format!(
                "vosk worker write: {err}. No transcript was invented."
            )));
        }
        let mut events = Vec::new();
        loop {
            let mut line = String::new();
            let n = self.reader.read_line(&mut line).map_err(|err| {
                AsrError::new(format!(
                    "vosk worker read: {err}. No transcript was invented."
                ))
            })?;
            if n == 0 {
                let _ = self.child.try_wait();
                return Err(AsrError::new(
                    "vosk worker closed before a final. No transcript was invented.",
                ));
            }
            match parse_event(&line)? {
                WorkerEvent::Partial { text, .. } => {
                    let text = clean_transcript(&text);
                    if !text.is_empty() {
                        events.push(AsrEvent {
                            kind: AsrKind::Partial,
                            text,
                            audio_ms,
                            words: Vec::new(),
                            why: "worker",
                        });
                    }
                }
                WorkerEvent::Final { text, words, .. } => {
                    let text = clean_transcript(&text);
                    let words = words
                        .into_iter()
                        .filter(|w| w.word != "[unk]" && w.word != "<unk>")
                        .collect();
                    events.push(AsrEvent {
                        kind: AsrKind::Final,
                        text,
                        audio_ms,
                        words,
                        why: "worker",
                    });
                    let _ = self.child.try_wait();
                    return Ok(events);
                }
                WorkerEvent::Error { error } => {
                    return Err(AsrError::new(format!(
                        "{error} No transcript was invented."
                    )));
                }
                WorkerEvent::Ready { .. } => {
                    return Err(AsrError::new(
                        "unexpected ready from vosk worker. No transcript was invented.",
                    ));
                }
            }
        }
    }

    fn reset(&mut self) -> Result<(), AsrError> {
        self.pending.clear();
        Ok(())
    }

    fn name(&self) -> &str {
        &self.label
    }
}

impl Drop for VoskWorkerAsr {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn prepend_child_lib(cmd: &mut Command, dir: &Path) {
    let dir = dir.display().to_string();
    #[cfg(unix)]
    {
        let old = std::env::var("LD_LIBRARY_PATH").unwrap_or_default();
        let joined = if old.is_empty() {
            dir
        } else {
            format!("{dir}:{old}")
        };
        cmd.env("LD_LIBRARY_PATH", joined);
    }
    #[cfg(windows)]
    {
        let old = std::env::var("PATH").unwrap_or_default();
        let joined = if old.is_empty() {
            dir
        } else {
            format!("{dir};{old}")
        };
        cmd.env("PATH", joined);
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (cmd, dir);
    }
}

#[derive(Debug)]
#[allow(dead_code)]
enum WorkerEvent {
    Ready {
        model: String,
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

pub fn encode_utterance(rate: u32, pcm: &[i16]) -> String {
    let mut raw = Vec::with_capacity(pcm.len() * 2);
    for sample in pcm {
        raw.extend_from_slice(&sample.to_le_bytes());
    }
    let line = to_line(&Json::Obj(vec![
        ("cmd".into(), Json::Str("utterance".into())),
        ("rate".into(), Json::Num(rate as f64)),
        ("pcm_b64".into(), Json::Str(b64_encode(&raw))),
    ]));
    format!("{line}\n")
}

#[allow(dead_code)]
pub fn decode_utterance(line: &str) -> Result<(u32, Vec<i16>), AsrError> {
    let value = parse(line.trim()).map_err(AsrError::new)?;
    let rate = value.get("rate").and_then(Json::as_f64).unwrap_or(0.0) as u32;
    let b64 = value
        .get("pcm_b64")
        .and_then(Json::as_str)
        .ok_or_else(|| AsrError::new("missing pcm_b64"))?;
    let raw = b64_decode(b64).map_err(AsrError::new)?;
    if raw.len() % 2 != 0 {
        return Err(AsrError::new("pcm byte length is odd"));
    }
    let pcm = raw
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect();
    Ok((rate, pcm))
}

fn parse_event(line: &str) -> Result<WorkerEvent, AsrError> {
    let value = parse(line.trim()).map_err(|err| AsrError::new(format!("worker JSON: {err}")))?;
    let kind = value.get("type").and_then(Json::as_str).unwrap_or("");
    match kind {
        "ready" => Ok(WorkerEvent::Ready {
            model: value
                .get("model")
                .and_then(Json::as_str)
                .unwrap_or("")
                .to_string(),
        }),
        "partial" => Ok(WorkerEvent::Partial {
            text: value
                .get("text")
                .and_then(Json::as_str)
                .unwrap_or("")
                .to_string(),
            t_ms: num_u32(value.get("t_ms")),
        }),
        "final" => Ok(WorkerEvent::Final {
            text: value
                .get("text")
                .and_then(Json::as_str)
                .unwrap_or("")
                .to_string(),
            t_ms: num_u32(value.get("t_ms")),
            words: words_of(value.get("words")),
        }),
        "error" => Ok(WorkerEvent::Error {
            error: value
                .get("error")
                .and_then(Json::as_str)
                .unwrap_or("vosk worker error")
                .to_string(),
        }),
        other => Err(AsrError::new(format!("unknown worker event {other}"))),
    }
}

fn words_of(value: Option<&Json>) -> Vec<Word> {
    let Some(arr) = value.and_then(Json::as_arr) else {
        return Vec::new();
    };
    let mut words = Vec::new();
    for item in arr {
        let Some(word) = item.get("word").and_then(Json::as_str) else {
            continue;
        };
        words.push(Word {
            word: word.to_string(),
            start: item.get("start").and_then(Json::as_f64).unwrap_or(0.0) as f32,
            end: item.get("end").and_then(Json::as_f64).unwrap_or(0.0) as f32,
            conf: item.get("conf").and_then(Json::as_f64).unwrap_or(0.0) as f32,
        });
    }
    words
}

fn num_u32(value: Option<&Json>) -> u32 {
    value.and_then(Json::as_f64).unwrap_or(0.0).max(0.0) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utterance_pcm_roundtrip() {
        let pcm = vec![0, 1, -1, 32767, -32768];
        let line = encode_utterance(16_000, &pcm);
        let (rate, back) = decode_utterance(&line).unwrap();
        assert_eq!(rate, 16_000);
        assert_eq!(back, pcm);
    }

    #[test]
    fn parses_final_with_words() {
        let line = r#"{"type":"final","text":"hello there","t_ms":40,"words":[{"word":"hello","start":0.0,"end":0.2,"conf":0.5},{"word":"there","start":0.2,"end":0.4,"conf":0.75}]}"#;
        match parse_event(line).unwrap() {
            WorkerEvent::Final { text, words, t_ms } => {
                assert_eq!(text, "hello there");
                assert_eq!(t_ms, 40);
                assert_eq!(words.len(), 2);
                assert_eq!(words[1].word, "there");
            }
            other => panic!("unexpected {other:?}"),
        }
    }
}
