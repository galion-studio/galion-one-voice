//! Scorer backend selection for the missing-emotion-targets stage.
//!
//! Default is the current unmodified universal scorer. `persona` is opt-in.
//! This module does not edit `scorers/` or `pipeline/personas/`. It reads
//! `scorers/persona/INTERFACE.md` when present and otherwise uses the
//! conventional persona entry point documented below.

use crate::invoke::{cloning_v2_root, default_scorer_py, default_scorer_python};
use anyhow::{bail, Result};
use std::path::{Path, PathBuf};

pub const DEFAULT_SCORER: &str = "universal";
pub const PERSONA_CONVENTIONAL_SCRIPT: &str = "scorers/persona/py/score_persona.py";
pub const PERSONA_INTERFACE_REL: &str = "scorers/persona/INTERFACE.md";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScorerKind {
    Universal,
    Persona,
}

impl ScorerKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ScorerKind::Universal => "universal",
            ScorerKind::Persona => "persona",
        }
    }
}

pub fn parse_scorer_kind(s: &str) -> Result<ScorerKind> {
    match s.trim() {
        "" | "universal" => Ok(ScorerKind::Universal),
        "persona" => Ok(ScorerKind::Persona),
        other => bail!("unknown scorer {other:?}; use universal (default) or persona"),
    }
}

/// Explicit flag wins; otherwise config `missing_emotion_targets.scorer`; else universal.
pub fn scorer_kind_from_config(explicit: Option<&str>, config: &serde_json::Value) -> Result<ScorerKind> {
    if let Some(s) = explicit {
        return parse_scorer_kind(s);
    }
    if let Some(s) = config
        .get("missing_emotion_targets")
        .and_then(|v| v.get("scorer"))
        .and_then(|v| v.as_str())
    {
        return parse_scorer_kind(s);
    }
    Ok(ScorerKind::Universal)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PersonaInterface {
    pub script: PathBuf,
    pub python: Option<PathBuf>,
    pub persona_flag: String,
}

impl Default for PersonaInterface {
    fn default() -> Self {
        Self {
            script: PathBuf::from(PERSONA_CONVENTIONAL_SCRIPT),
            python: None,
            persona_flag: "--persona".into(),
        }
    }
}

/// Parse `scorers/persona/INTERFACE.md` without requiring a fixed template.
/// Recognizes `script:` / `entry:` lines, a `scorers/persona/*.py` path, and
/// `--persona-file` vs `--persona`.
pub fn parse_interface_md(text: &str) -> PersonaInterface {
    let mut spec = PersonaInterface::default();
    let mut found_script = false;
    for raw in text.lines() {
        let line = raw.trim();
        let lower = line.to_ascii_lowercase();
        if let Some(rest) = keyed_value(line, &["script", "entry", "entrypoint", "cli", "binary"]) {
            let path = rest.split_whitespace().next().unwrap_or(rest).trim_matches('`');
            if path.ends_with(".py") || path.contains("persona") {
                spec.script = PathBuf::from(path);
                found_script = true;
            }
        }
        if let Some(rest) = keyed_value(line, &["python", "interpreter"]) {
            let path = rest.split_whitespace().next().unwrap_or(rest).trim_matches('`');
            if !path.is_empty() {
                spec.python = Some(PathBuf::from(path));
            }
        }
        if lower.contains("--persona-file") {
            spec.persona_flag = "--persona-file".into();
        } else if spec.persona_flag == "--persona" && lower.contains("--persona") {
            spec.persona_flag = "--persona".into();
        }
        if !found_script {
            if let Some(p) = first_persona_py(line) {
                spec.script = PathBuf::from(p);
                found_script = true;
            }
        }
    }
    spec
}

fn keyed_value<'a>(line: &'a str, keys: &[&str]) -> Option<&'a str> {
    let lower = line.to_ascii_lowercase();
    for key in keys {
        let prefix = format!("{key}:");
        if lower.starts_with(&prefix) {
            return Some(line[key.len() + 1..].trim());
        }
        let prefix = format!("{key} =");
        if lower.starts_with(&prefix) {
            return Some(line[key.len() + 2..].trim());
        }
    }
    None
}

fn first_persona_py(line: &str) -> Option<String> {
    for tok in line.split(|c: char| c.is_whitespace() || c == '`' || c == '"' || c == '\'') {
        if tok.contains("scorers/persona/") && tok.ends_with(".py") {
            return Some(tok.trim_matches(|c| c == ',' || c == ')').to_string());
        }
        if tok.ends_with("score_persona.py") {
            return Some(tok.to_string());
        }
    }
    None
}

pub fn load_persona_interface(root: &Path) -> PersonaInterface {
    let path = root.join(PERSONA_INTERFACE_REL);
    match std::fs::read_to_string(&path) {
        Ok(text) => parse_interface_md(&text),
        Err(_) => PersonaInterface::default(),
    }
}

#[derive(Clone, Debug)]
pub struct ResolvedScorer {
    pub kind: ScorerKind,
    pub script: PathBuf,
    pub python: PathBuf,
    pub extra_args: Vec<String>,
    pub persona_file: Option<PathBuf>,
    pub interface_loaded: bool,
}

pub fn default_persona_file_candidates(voice: &str) -> Vec<PathBuf> {
    let root = cloning_v2_root();
    let mut out = vec![root.join(format!("pipeline/personas/{voice}.toml"))];
    if let Some(rest) = voice.strip_prefix("own-") {
        let alt = root.join(format!("pipeline/personas/{rest}.toml"));
        if alt != out[0] {
            out.push(alt);
        }
    }
    out
}

pub fn resolve_persona_file(voice: &str, explicit: Option<&Path>) -> Option<PathBuf> {
    if let Some(p) = explicit {
        return Some(p.to_path_buf());
    }
    default_persona_file_candidates(voice)
        .into_iter()
        .find(|p| p.is_file())
}

fn into_abs(root: &Path, p: PathBuf) -> PathBuf {
    if p.is_absolute() {
        p
    } else {
        root.join(p)
    }
}

pub fn resolve_scorer(
    kind: ScorerKind,
    script_override: Option<PathBuf>,
    python_override: Option<PathBuf>,
    persona_file: Option<PathBuf>,
) -> ResolvedScorer {
    let root = cloning_v2_root();
    match kind {
        ScorerKind::Universal => ResolvedScorer {
            kind,
            script: script_override.unwrap_or_else(default_scorer_py),
            python: python_override.unwrap_or_else(default_scorer_python),
            extra_args: vec![],
            persona_file: None,
            interface_loaded: false,
        },
        ScorerKind::Persona => {
            let iface_path = root.join(PERSONA_INTERFACE_REL);
            let loaded = iface_path.is_file();
            let iface = load_persona_interface(&root);
            let script = script_override.unwrap_or_else(|| into_abs(&root, iface.script.clone()));
            let python = python_override.unwrap_or_else(|| {
                iface
                    .python
                    .clone()
                    .map(|p| into_abs(&root, p))
                    .filter(|p| p.exists())
                    .or_else(|| {
                        let v = root.join("scorers/persona/venv/bin/python3");
                        if v.exists() {
                            Some(v)
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(default_scorer_python)
            });
            let mut extra_args = vec![];
            if let Some(pf) = persona_file.clone() {
                extra_args.push(iface.persona_flag.clone());
                extra_args.push(pf.to_string_lossy().into());
            }
            ResolvedScorer {
                kind,
                script,
                python,
                extra_args,
                persona_file,
                interface_loaded: loaded,
            }
        }
    }
}

/// argv after the interpreter: script, wavs, --out, --threads, extras.
pub fn score_argv(resolved: &ResolvedScorer, wavs: &[PathBuf], out_json: &Path) -> Vec<String> {
    let mut args = vec![resolved.script.to_string_lossy().into()];
    for w in wavs {
        args.push(w.to_string_lossy().into());
    }
    args.push("--out".into());
    args.push(out_json.to_string_lossy().into());
    args.push("--threads".into());
    args.push("2".into());
    args.extend(resolved.extra_args.iter().cloned());
    args
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn default_backend_is_universal() {
        assert_eq!(parse_scorer_kind("").unwrap(), ScorerKind::Universal);
        assert_eq!(parse_scorer_kind("universal").unwrap(), ScorerKind::Universal);
        assert_eq!(scorer_kind_from_config(None, &json!({})).unwrap(), ScorerKind::Universal);
        assert_eq!(
            scorer_kind_from_config(None, &json!({"missing_emotion_targets": {"enabled": true}})).unwrap(),
            ScorerKind::Universal
        );
    }

    #[test]
    fn opt_in_persona_from_flag_and_config() {
        assert_eq!(parse_scorer_kind("persona").unwrap(), ScorerKind::Persona);
        assert_eq!(
            scorer_kind_from_config(None, &json!({"missing_emotion_targets": {"scorer": "persona"}})).unwrap(),
            ScorerKind::Persona
        );
        assert_eq!(
            scorer_kind_from_config(Some("universal"), &json!({"missing_emotion_targets": {"scorer": "persona"}})).unwrap(),
            ScorerKind::Universal
        );
        assert!(parse_scorer_kind("clap").is_err());
    }

    #[test]
    fn resolve_universal_points_at_current_scorer() {
        let r = resolve_scorer(ScorerKind::Universal, None, None, None);
        assert_eq!(r.kind, ScorerKind::Universal);
        assert!(r.script.ends_with("scorers/universal/py/score_universal.py"), "{}", r.script.display());
        assert!(r.extra_args.is_empty());
        let argv = score_argv(&r, &[PathBuf::from("a.wav")], Path::new("out.json"));
        assert!(argv.iter().any(|a| a.ends_with("score_universal.py")));
        assert!(!argv.iter().any(|a| a == "--persona" || a == "--persona-file"));
        assert_eq!(argv[1], "a.wav");
        assert_eq!(argv[2], "--out");
    }

    #[test]
    fn resolve_persona_uses_conventional_path_until_interface_exists() {
        let r = resolve_scorer(ScorerKind::Persona, None, None, Some(PathBuf::from("/tmp/samantha.toml")));
        assert_eq!(r.kind, ScorerKind::Persona);
        if !r.interface_loaded {
            assert!(
                r.script.ends_with(PERSONA_CONVENTIONAL_SCRIPT) || r.script.ends_with("score_persona.py"),
                "{}",
                r.script.display()
            );
            assert_eq!(r.extra_args, vec!["--persona".to_string(), "/tmp/samantha.toml".to_string()]);
        } else {
            assert!(!r.extra_args.is_empty());
            assert_eq!(r.extra_args[1], "/tmp/samantha.toml");
        }
        let argv = score_argv(&r, &[PathBuf::from("a.wav")], Path::new("out.json"));
        assert!(argv.contains(&"/tmp/samantha.toml".to_string()));
    }

    #[test]
    fn parse_interface_md_extracts_script_and_persona_flag() {
        let spec = parse_interface_md(
            r#"
# Persona scorer

script: scorers/persona/py/score_persona.py
python: scorers/persona/venv/bin/python3

```
score_persona.py wavs... --out OUT.json --persona-file PERSONA.toml
```
"#,
        );
        assert_eq!(spec.script, PathBuf::from("scorers/persona/py/score_persona.py"));
        assert_eq!(spec.python, Some(PathBuf::from("scorers/persona/venv/bin/python3")));
        assert_eq!(spec.persona_flag, "--persona-file");
    }

    #[test]
    fn parse_interface_md_finds_embedded_path() {
        let spec = parse_interface_md("Run `scorers/persona/py/score_persona.py` with --persona FILE");
        assert_eq!(spec.script, PathBuf::from("scorers/persona/py/score_persona.py"));
        assert_eq!(spec.persona_flag, "--persona");
    }

    #[test]
    fn persona_file_candidates_include_own_prefix_and_stripped() {
        let c = default_persona_file_candidates("own-samantha");
        assert!(c.iter().any(|p| p.ends_with("pipeline/personas/own-samantha.toml")));
        assert!(c.iter().any(|p| p.ends_with("pipeline/personas/samantha.toml")));
    }
}
