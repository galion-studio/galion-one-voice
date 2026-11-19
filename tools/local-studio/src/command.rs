// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Argv templates and process launch.
//!
//! Commands run without a shell. Offline environment variables are set so a
//! wrapped tool does not fetch weights. This crate still cannot stop a
//! program that ignores those variables; a download attempt fails the take.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolError {
    Fatal(String),
    Take(String),
}

impl ToolError {
    pub fn message(&self) -> &str {
        match self {
            ToolError::Fatal(s) | ToolError::Take(s) => s,
        }
    }
}

pub fn fill_argv(template: &[String], vars: &[(&str, &str)]) -> Result<Vec<String>, ToolError> {
    template.iter().map(|part| fill_one(part, vars)).collect()
}

fn fill_one(template: &str, vars: &[(&str, &str)]) -> Result<String, ToolError> {
    let chars: Vec<char> = template.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '{' {
            if let Some(rel) = chars[i + 1..].iter().position(|c| *c == '}') {
                let key: String = chars[i + 1..i + 1 + rel].iter().collect();
                if key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') && !key.is_empty() {
                    let value = vars
                        .iter()
                        .find(|(name, _)| *name == key)
                        .map(|(_, value)| *value)
                        .ok_or_else(|| {
                            ToolError::Fatal(format!("command placeholder {{{key}}} has no value"))
                        })?;
                    out.push_str(value);
                    i += rel + 2;
                    continue;
                }
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    Ok(out)
}

pub fn run_cmd(argv: &[String]) -> Result<Output, ToolError> {
    if argv.is_empty() || argv[0].trim().is_empty() {
        return Err(ToolError::Fatal("command is empty".into()));
    }
    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("HF_HUB_OFFLINE", "1")
        .env("HF_HUB_DISABLE_TELEMETRY", "1")
        .env("TRANSFORMERS_OFFLINE", "1")
        .env("HF_DATASETS_OFFLINE", "1");
    match cmd.output() {
        Ok(output) => Ok(output),
        Err(err) if err.kind() == ErrorKind::NotFound => {
            Err(ToolError::Fatal(format!("{} is not installed", argv[0])))
        }
        Err(err) => Err(ToolError::Fatal(format!(
            "{} failed to start ({err})",
            argv[0]
        ))),
    }
}

pub fn tail(text: &str, limit: usize) -> String {
    let trimmed = text.trim();
    let count = trimmed.chars().count();
    if count <= limit {
        return trimmed.to_string();
    }
    trimmed.chars().skip(count - limit).collect()
}

pub fn on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let direct = dir.join(name);
        if direct.is_file() {
            return Some(direct);
        }
        if cfg!(windows) {
            let exe = dir.join(format!("{name}.exe"));
            if exe.is_file() {
                return Some(exe);
            }
        }
    }
    None
}

pub fn env_bin(key: &str) -> Option<PathBuf> {
    let value = std::env::var(key).ok()?;
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let path = PathBuf::from(value);
    if path.is_file() {
        return Some(path);
    }
    on_path(value)
}

#[derive(Debug, Clone)]
pub struct ToolPaths {
    pub cbx: Option<PathBuf>,
    pub whisper: Option<PathBuf>,
    pub ecapa: Option<PathBuf>,
    pub emotion: Option<PathBuf>,
    pub ffmpeg: Option<PathBuf>,
}

pub fn discover() -> ToolPaths {
    ToolPaths {
        cbx: env_bin("GALION_CBX_BIN").or_else(|| on_path("cbx")),
        whisper: env_bin("GALION_WHISPER").or_else(|| on_path("whisper")),
        ecapa: env_bin("GALION_ECAPA_BIN").or_else(|| on_path("galion-ecapa")),
        emotion: env_bin("GALION_EMOTION_BIN").or_else(|| on_path("galion-emotion")),
        ffmpeg: env_bin("GALION_FFMPEG").or_else(|| on_path("ffmpeg")),
    }
}

pub fn default_cbx_argv(bin: &str) -> Vec<String> {
    vec![
        bin.into(),
        "--ep".into(),
        "{device}".into(),
        "speak".into(),
        "--dtype".into(),
        "fp32".into(),
        "--text".into(),
        "{text}".into(),
        "--voice-wav".into(),
        "{reference}".into(),
        "--out-wav".into(),
        "{out}".into(),
        "--seed".into(),
        "{seed}".into(),
        "--temperature".into(),
        "{temperature}".into(),
        "--pace".into(),
        "1.0".into(),
    ]
}

pub fn default_whisper_argv(bin: &str) -> Vec<String> {
    vec![
        bin.into(),
        "{wav}".into(),
        "--model".into(),
        "{model}".into(),
        "--language".into(),
        "en".into(),
        "--task".into(),
        "transcribe".into(),
        "--output_format".into(),
        "json".into(),
        "--output_dir".into(),
        "{dir}".into(),
        "--fp16".into(),
        "False".into(),
        "--verbose".into(),
        "False".into(),
    ]
}

pub fn default_ecapa_argv(bin: &str) -> Vec<String> {
    vec![
        bin.into(),
        "--ref".into(),
        "{reference}".into(),
        "{wav}".into(),
        "--threads".into(),
        "2".into(),
        "--out".into(),
        "{json}".into(),
    ]
}

pub fn default_emotion_argv(bin: &str) -> Vec<String> {
    vec![
        bin.into(),
        "score".into(),
        "--ref".into(),
        "{reference}".into(),
        "{wav}".into(),
        "--threads".into(),
        "2".into(),
        "--out".into(),
        "{json}".into(),
    ]
}

static TEMP_SEQ: AtomicU64 = AtomicU64::new(1);

pub struct TempDir(pub PathBuf);

impl TempDir {
    pub fn new(prefix: &str) -> Result<Self, ToolError> {
        let n = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("{prefix}-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&path)
            .map_err(|err| ToolError::Fatal(format!("{}: {err}", path.display())))?;
        Ok(Self(path))
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub fn display_path(path: &Path) -> String {
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cbx_template_stays_a_local_argv() {
        let argv = fill_argv(
            &default_cbx_argv("cbx"),
            &[
                ("device", "cpu"),
                ("text", "Hello there."),
                ("reference", "/voices/owner.wav"),
                ("out", "/tmp/out.wav"),
                ("seed", "7"),
                ("temperature", "0.7000"),
            ],
        )
        .unwrap();
        let joined = argv.join(" ");
        assert!(joined.contains("--ep cpu"));
        assert!(joined.contains("speak"));
        assert!(joined.contains("--voice-wav /voices/owner.wav"));
        assert!(!joined.contains("download"));
        assert!(!joined.contains("http"));
        assert_eq!(argv[0], "cbx");
    }

    #[test]
    fn text_with_braces_is_inserted_once() {
        let argv = fill_argv(
            &["{text}".into()],
            &[("text", "value {seed} stays"), ("seed", "1")],
        )
        .unwrap();
        assert_eq!(argv, vec!["value {seed} stays".to_string()]);
    }

    #[test]
    fn unknown_placeholder_is_an_error() {
        let err = fill_argv(&["{missing}".into()], &[]).unwrap_err();
        assert!(matches!(err, ToolError::Fatal(_)));
    }
}
