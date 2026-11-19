// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Take scores.
//!
//! Whisper (or any CLI that prints a transcript) supplies words. WER is
//! computed here. ECAPA and the emotion classifier supply their own numbers.
//! A missing number stays unmeasured. Reference-clip emotion similarity is
//! not treated as the directed-emotion score.

use crate::command::{display_path, fill_argv, run_cmd, tail, TempDir, ToolError};
use crate::wer::word_error_rate;
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TakeScores {
    pub wer: Option<f64>,
    pub ecapa: Option<f64>,
    pub emotion: Option<f64>,
    pub transcript: String,
    pub detail: String,
}

impl Default for TakeScores {
    fn default() -> Self {
        Self {
            wer: None,
            ecapa: None,
            emotion: None,
            transcript: String::new(),
            detail: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct GateResult {
    pub wer: bool,
    pub ecapa: bool,
    pub emotion: bool,
    pub passed: bool,
    pub detail: String,
}

pub fn judge(gates: &crate::config::Gates, scores: &TakeScores) -> GateResult {
    let wer_ok = scores.wer.is_some_and(|rate| rate <= gates.wer_max);
    let ecapa_ok = scores.ecapa.is_some_and(|score| score > gates.ecapa_min);
    let emotion_ok = scores
        .emotion
        .is_some_and(|score| score >= gates.emotion_min);
    let mut notes = Vec::new();
    match scores.wer {
        None => notes.push("WER was not measured".to_string()),
        Some(_) if !wer_ok => notes.push("WER is above the gate".to_string()),
        _ => {}
    }
    match scores.ecapa {
        None => notes.push("ECAPA was not measured".to_string()),
        Some(_) if !ecapa_ok => notes.push("ECAPA is at or below the gate".to_string()),
        _ => {}
    }
    match scores.emotion {
        None => notes.push("emotion was not measured".to_string()),
        Some(_) if !emotion_ok => notes.push("emotion is below the gate".to_string()),
        _ => {}
    }
    GateResult {
        wer: wer_ok,
        ecapa: ecapa_ok,
        emotion: emotion_ok,
        passed: wer_ok && ecapa_ok && emotion_ok,
        detail: if notes.is_empty() {
            "passed".into()
        } else {
            notes.join("; ")
        },
    }
}

pub fn resolve_label(map: &BTreeMap<String, String>, emotion: &str) -> String {
    if let Some(label) = map.get(emotion) {
        return label.clone();
    }
    for (key, label) in map {
        if key.eq_ignore_ascii_case(emotion) {
            return label.clone();
        }
    }
    emotion.to_ascii_lowercase()
}

pub struct ScoreJob<'a> {
    pub line_index: usize,
    pub take_index: usize,
    pub text: &'a str,
    pub emotion: &'a str,
    pub reference: &'a Path,
    pub wav: &'a Path,
    pub device_ep: &'a str,
}

pub trait TakeScorer {
    fn name(&self) -> &str;
    fn score(&mut self, job: &ScoreJob<'_>) -> Result<TakeScores, ToolError>;
}

#[derive(Debug, Clone)]
pub struct ScriptedScorer {
    pub scores: Vec<Vec<TakeScores>>,
}

impl TakeScorer for ScriptedScorer {
    fn name(&self) -> &str {
        "scripted"
    }

    fn score(&mut self, job: &ScoreJob<'_>) -> Result<TakeScores, ToolError> {
        self.scores
            .get(job.line_index)
            .and_then(|row| row.get(job.take_index))
            .cloned()
            .ok_or_else(|| ToolError::Take("scripted score is missing for this take".into()))
    }
}

pub struct ProcessScorer {
    pub whisper: Vec<String>,
    pub ecapa: Vec<String>,
    pub emotion: Vec<String>,
    pub model: String,
    pub label_map: BTreeMap<String, String>,
}

impl TakeScorer for ProcessScorer {
    fn name(&self) -> &str {
        "process"
    }

    fn score(&mut self, job: &ScoreJob<'_>) -> Result<TakeScores, ToolError> {
        let dir = TempDir::new("galion-studio-score")?;
        let ecapa_json = dir.0.join("ecapa.json");
        let emotion_json = dir.0.join("emotion.json");
        let owned = [
            ("wav", display_path(job.wav)),
            ("reference", display_path(job.reference)),
            ("text", job.text.to_string()),
            ("emotion", job.emotion.to_string()),
            ("dir", display_path(&dir.0)),
            ("model", self.model.clone()),
            ("take", job.take_index.to_string()),
            ("line", job.line_index.to_string()),
            ("device", job.device_ep.to_string()),
        ];
        let whisper_vars = bind(&owned);

        let (transcript, wer, wer_note) = if self.whisper.is_empty() {
            (String::new(), None, "whisper command is empty".to_string())
        } else {
            match transcribe(&self.whisper, &whisper_vars, &dir.0) {
                Ok(text) => match word_error_rate(job.text, &text) {
                    Ok(measured) => (text, Some(measured.rate), "whisper transcript".into()),
                    Err(err) => (text, None, err),
                },
                Err(ToolError::Fatal(err)) => return Err(ToolError::Fatal(err)),
                Err(ToolError::Take(err)) => (String::new(), None, err),
            }
        };

        let ecapa_owned = with_json(&owned, &display_path(&ecapa_json));
        let ecapa_vars = bind(&ecapa_owned);
        let (ecapa, ecapa_note) = run_number(&self.ecapa, &ecapa_vars, &ecapa_json, |body| {
            parse_ecapa_score(body, job.wav)
        })?;
        let label = resolve_label(&self.label_map, job.emotion);
        let emotion_owned = with_json(&owned, &display_path(&emotion_json));
        let emotion_vars = bind(&emotion_owned);
        let (emotion, emotion_note) =
            run_number(&self.emotion, &emotion_vars, &emotion_json, |body| {
                parse_emotion_score(body, &label)
            })?;

        let detail = format!("{wer_note}; {ecapa_note}; {emotion_note}");
        Ok(TakeScores {
            wer,
            ecapa,
            emotion,
            transcript,
            detail,
        })
    }
}

fn bind<'a>(owned: &'a [(&'a str, String)]) -> Vec<(&'a str, &'a str)> {
    owned
        .iter()
        .map(|(key, value)| (*key, value.as_str()))
        .collect()
}

fn with_json<'a>(owned: &[(&'a str, String)], json: &str) -> Vec<(&'a str, String)> {
    let mut out: Vec<(&'a str, String)> = owned.iter().cloned().collect();
    out.push(("json", json.to_string()));
    out
}

fn transcribe(template: &[String], vars: &[(&str, &str)], dir: &Path) -> Result<String, ToolError> {
    let argv = fill_argv(template, vars)?;
    let output = run_cmd(&argv)?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if let Some(text) = transcript_from_dir(dir) {
        return Ok(text);
    }
    if let Some(text) = parse_transcript(&stdout) {
        return Ok(text);
    }
    if output.status.success() {
        let plain = stdout.trim();
        if !plain.is_empty() && plain.lines().count() <= 3 && !plain.starts_with('{') {
            return Ok(plain.to_string());
        }
    }
    Err(ToolError::Take(format!(
        "whisper returned no transcript: {}",
        tail(&stderr, 300)
    )))
}

fn run_number(
    template: &[String],
    vars: &[(&str, &str)],
    json_path: &Path,
    parse: impl Fn(&str) -> Option<f64>,
) -> Result<(Option<f64>, String), ToolError> {
    if template.is_empty() {
        return Ok((None, "command is empty".into()));
    }
    let _ = std::fs::remove_file(json_path);
    let argv = fill_argv(template, vars)?;
    let output = run_cmd(&argv)?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let file_body = std::fs::read_to_string(json_path).unwrap_or_default();
    let body = if file_body.trim().is_empty() {
        stdout
    } else {
        file_body
    };
    if let Some(score) = parse(&body) {
        return Ok((Some(score), "measured".into()));
    }
    if output.status.success() {
        return Ok((None, "score JSON had no directed value".into()));
    }
    Ok((
        None,
        format!("scorer exited {}: {}", output.status, tail(&stderr, 240)),
    ))
}

pub fn parse_transcript(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        if let Some(text) = value.get("text").and_then(|v| v.as_str()) {
            let text = text.trim();
            if !text.is_empty() {
                return Some(text.to_string());
            }
        }
        if let Some(items) = value.get("transcription").and_then(|v| v.as_array()) {
            let mut parts = Vec::new();
            for item in items {
                if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                    let text = text.trim();
                    if !text.is_empty() {
                        parts.push(text.to_string());
                    }
                }
            }
            if !parts.is_empty() {
                return Some(parts.join(" "));
            }
        }
        return None;
    }
    None
}

fn transcript_from_dir(dir: &Path) -> Option<String> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut files: Vec<_> = entries.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    files.sort();
    for path in files {
        let ext = path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if ext != "json" && ext != "txt" {
            continue;
        }
        let Ok(body) = std::fs::read_to_string(&path) else {
            continue;
        };
        if ext == "txt" {
            let text = body.trim();
            if !text.is_empty() && !text.starts_with('{') {
                return Some(text.to_string());
            }
        }
        if let Some(text) = parse_transcript(&body) {
            return Some(text);
        }
    }
    None
}

pub fn parse_ecapa_score(text: &str, wav: &Path) -> Option<f64> {
    let value: Value = serde_json::from_str(text.trim()).ok()?;
    if let Some(number) = as_f64(&value) {
        return Some(number);
    }
    let obj = value.as_object()?;
    for key in ["ecapa", "similarity", "cosine"] {
        if let Some(number) = obj.get(key).and_then(as_f64) {
            return Some(number);
        }
    }
    if let Some(scores) = obj.get("scores").and_then(|v| v.as_object()) {
        let name = wav.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let full = wav.to_string_lossy();
        for (key, val) in scores {
            if key.as_str() == full.as_ref() || (!name.is_empty() && key.ends_with(name)) {
                if let Some(number) = as_f64(val) {
                    return Some(number);
                }
            }
        }
        if scores.len() == 1 {
            return scores.values().next().and_then(as_f64);
        }
    }
    obj.get("mean").and_then(as_f64)
}

pub fn parse_emotion_score(text: &str, label: &str) -> Option<f64> {
    let value: Value = serde_json::from_str(text.trim()).ok()?;
    if let Some(number) = as_f64(&value) {
        return Some(number);
    }
    let obj = value.as_object()?;
    for key in ["emotion_match", "emotion", "score"] {
        if let Some(number) = obj.get(key).and_then(as_f64) {
            return Some(number);
        }
    }
    if let Some(number) = lookup_probs(obj, label) {
        return Some(number);
    }
    let files = obj.get("files")?.as_array()?;
    let first = files.first()?.as_object()?;
    lookup_probs(first, label).or_else(|| {
        first
            .get("ser")
            .and_then(|ser| ser.as_object())
            .and_then(|ser| lookup_probs(ser, label))
    })
}

fn lookup_probs(obj: &Map<String, Value>, label: &str) -> Option<f64> {
    let probs = obj.get("probs")?.as_object()?;
    for (key, value) in probs {
        if key.eq_ignore_ascii_case(label) {
            return as_f64(value);
        }
    }
    None
}

fn as_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64().filter(|n| n.is_finite()),
        Value::String(text) => text.trim().parse::<f64>().ok().filter(|n| n.is_finite()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Gates;

    fn scores(wer: f64, ecapa: f64, emotion: f64) -> TakeScores {
        TakeScores {
            wer: Some(wer),
            ecapa: Some(ecapa),
            emotion: Some(emotion),
            transcript: String::new(),
            detail: String::new(),
        }
    }

    #[test]
    fn gate_boundaries_match_the_rollout() {
        let gates = Gates::rollout();
        let edge = scores(0.018, 0.7057, 0.83);
        let judged = judge(&gates, &edge);
        assert!(judged.wer);
        assert!(!judged.ecapa);
        assert!(judged.emotion);
        assert!(!judged.passed);

        let pass = scores(0.018, 0.7058, 0.83);
        assert!(judge(&gates, &pass).passed);

        let mut missing = scores(0.0, 0.8, 0.9);
        missing.wer = None;
        assert!(!judge(&gates, &missing).passed);
    }

    #[test]
    fn ecapa_json_uses_the_clip_score() {
        let wav = Path::new("/tmp/take.wav");
        let text = r#"{"ref":"/tmp/ref.wav","half_vs_half_ceiling":0.99,"scores":{"/tmp/take.wav":0.8123},"mean":0.8123}"#;
        let score = parse_ecapa_score(text, wav).unwrap();
        assert!((score - 0.8123).abs() < 1e-12);
    }

    #[test]
    fn reference_similarity_is_not_the_directed_emotion() {
        let text =
            r#"{"emotion_similarity":0.99,"files":[{"vs_ref":{"emotion_similarity":0.99}}]}"#;
        assert!(parse_emotion_score(text, "calm").is_none());
    }

    #[test]
    fn directed_posterior_is_the_emotion_score() {
        let text = r#"{"files":[{"ser":{"probs":{"calm":0.91,"happy":0.02}}}]}"#;
        let score = parse_emotion_score(text, "calm").unwrap();
        assert!((score - 0.91).abs() < 1e-12);
    }

    #[test]
    fn label_map_matches_case() {
        let mut map = BTreeMap::new();
        map.insert("warm".into(), "happy".into());
        assert_eq!(resolve_label(&map, "Warm"), "happy");
        assert_eq!(resolve_label(&BTreeMap::new(), "Calm"), "calm");
    }

    #[test]
    fn whisper_json_text_is_the_transcript() {
        let text = parse_transcript(r#"{"text":" hello there "}"#).unwrap();
        assert_eq!(text, "hello there");
    }
}
