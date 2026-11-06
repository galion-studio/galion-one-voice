//! Pluggable fix steps. External commands override the bundled simple versions.
//!
//! Env contract for a plugin:
//!   VOICECHECK_VOICE       production voice under test (read-only)
//!   VOICECHECK_PERSONA     samantha|jarvis
//!   VOICECHECK_BASE_WAV    original.wav (read-only)
//!   VOICECHECK_REF_WAV     reference.wav (read-only)
//!   VOICECHECK_PROFILE     path to profile JSON
//!   VOICECHECK_OUT_VOICE   target own-<p>-v2-check-* name (must not be production)
//!   VOICECHECK_WORK        work directory for this attempt
//!   VOICECHECK_DRY_RUN     1 if dry-run
//!   VOICECHECK_AUTOCHECK   autocheck root
//! Plugin must print a single JSON object to stdout:
//!   {"status":"ok"|"skipped"|"error","out_voice":"...","detail":"..."}

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::paths::{assert_safe_voice_name, Paths};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixKind {
    RefExpressive,
    ProsodyLight,
    Combined,
}

impl FixKind {
    pub fn id(self) -> &'static str {
        match self {
            FixKind::RefExpressive => "a_ref_expressive",
            FixKind::ProsodyLight => "b_prosody_light",
            FixKind::Combined => "c_combined",
        }
    }

    pub fn all() -> [FixKind; 3] {
        [
            FixKind::RefExpressive,
            FixKind::ProsodyLight,
            FixKind::Combined,
        ]
    }

    pub fn default_script(self, fixes_dir: &Path) -> PathBuf {
        match self {
            FixKind::RefExpressive => fixes_dir.join("fix-a-ref-expressive.sh"),
            FixKind::ProsodyLight => fixes_dir.join("fix-b-prosody-light.sh"),
            FixKind::Combined => fixes_dir.join("fix-c-combined.sh"),
        }
    }

    pub fn env_override(self) -> &'static str {
        match self {
            FixKind::RefExpressive => "VOICECHECK_FIX_A",
            FixKind::ProsodyLight => "VOICECHECK_FIX_B",
            FixKind::Combined => "VOICECHECK_FIX_C",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixOutcome {
    pub kind: FixKind,
    pub status: String,
    pub out_voice: Option<String>,
    pub detail: String,
    pub command: String,
}

pub fn resolve_fix_cmd(kind: FixKind, paths: &Paths) -> PathBuf {
    if let Ok(p) = std::env::var(kind.env_override()) {
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    kind.default_script(&paths.fixes_dir)
}

pub fn run_fix(
    paths: &Paths,
    kind: FixKind,
    voice: &str,
    persona: &str,
    base_wav: &Path,
    ref_wav: &Path,
    profile_path: &Path,
    dry_run: bool,
) -> Result<FixOutcome> {
    let cmd_path = resolve_fix_cmd(kind, paths);
    let out_voice = format!("own-{persona}-v2-check-{}", kind.id());
    assert_safe_voice_name(&out_voice)?;
    let work = paths.work.join(format!("{voice}-{}", kind.id()));
    std::fs::create_dir_all(&work)?;

    if !cmd_path.is_file() {
        return Ok(FixOutcome {
            kind,
            status: "error".into(),
            out_voice: None,
            detail: format!("fix plugin missing: {}", cmd_path.display()),
            command: cmd_path.display().to_string(),
        });
    }

    let mut cmd = Command::new("nice");
    cmd.args(["-n", "10"])
        .arg("bash")
        .arg(&cmd_path)
        .env("VOICECHECK_VOICE", voice)
        .env("VOICECHECK_PERSONA", persona)
        .env("VOICECHECK_BASE_WAV", base_wav)
        .env("VOICECHECK_REF_WAV", ref_wav)
        .env("VOICECHECK_PROFILE", profile_path)
        .env("VOICECHECK_OUT_VOICE", &out_voice)
        .env("VOICECHECK_WORK", &work)
        .env("VOICECHECK_DRY_RUN", if dry_run { "1" } else { "0" })
        .env("VOICECHECK_AUTOCHECK", &paths.autocheck)
        .env("VOICECHECK_VOICES_DIR", &paths.voices_dir)
        .env("VOICECHECK_CLONING_V2", &paths.cloning_v2);

    let output = cmd.output().with_context(|| format!("run {}", cmd_path.display()))?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !output.status.success() {
        return Ok(FixOutcome {
            kind,
            status: "error".into(),
            out_voice: None,
            detail: format!("exit {:?}; stderr={stderr}; stdout={stdout}", output.status.code()),
            command: cmd_path.display().to_string(),
        });
    }
    let parsed: Value = serde_json::from_str(&stdout).unwrap_or_else(|_| {
        serde_json::json!({"status":"ok","out_voice": out_voice, "detail": stdout})
    });
    let status = parsed
        .get("status")
        .and_then(|x| x.as_str())
        .unwrap_or("ok")
        .to_string();
    let ov = parsed
        .get("out_voice")
        .and_then(|x| x.as_str())
        .map(|s| s.to_string())
        .or_else(|| Some(out_voice.clone()));
    if let Some(ref name) = ov {
        if status == "ok" {
            assert_safe_voice_name(name)?;
        }
    }
    let detail = parsed
        .get("detail")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    if status != "ok" && status != "skipped" {
        bail!("fix {} reported status={status}: {detail}", kind.id());
    }
    Ok(FixOutcome {
        kind,
        status,
        out_voice: ov,
        detail,
        command: cmd_path.display().to_string(),
    })
}
