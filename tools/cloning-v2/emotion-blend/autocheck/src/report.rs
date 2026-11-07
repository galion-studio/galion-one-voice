//! Write per-voice JSON + Markdown reports.

use anyhow::Result;
use serde::Serialize;
use serde_json::{json, Value};
use std::fs;
use std::path::Path;

use crate::analyze::AnalysisBundle;
use crate::fixes::FixOutcome;
use crate::gate::GateResult;
use crate::profile::{aggregate_ser, VoiceProfile};

#[derive(Debug, Serialize)]
pub struct AttemptReport {
    pub label: String,
    pub voice: String,
    pub gate: GateResult,
    pub wer_mean: f64,
    pub ecapa_mean: f64,
    pub emotion_distance: Option<f64>,
    pub label_counts: Value,
    pub n_lines: usize,
    pub mean_delta_f0_hz: f64,
    pub mean_delta_pace: f64,
    pub mean_ser: Value,
    pub fix: Option<FixOutcome>,
    pub renders_dir: String,
}

#[derive(Debug, Serialize)]
pub struct VoiceCheckReport {
    pub schema: String,
    pub generated_at: String,
    pub timezone: String,
    pub voice: String,
    pub persona: String,
    pub dry_run: bool,
    pub profile_source: String,
    pub baselines: Value,
    pub base_recording: String,
    pub attempts: Vec<AttemptReport>,
    pub kept: Option<String>,
    pub notes: Vec<String>,
}

pub fn attempt_from_analysis(
    label: &str,
    voice: &str,
    bundle: &AnalysisBundle,
    gate: GateResult,
    fix: Option<FixOutcome>,
    renders_dir: &Path,
) -> AttemptReport {
    let n = bundle.lines.len().max(1) as f64;
    let mean_delta_f0_hz = bundle.lines.iter().map(|l| l.delta_f0_mean_hz).sum::<f64>() / n;
    let mean_delta_pace = bundle.lines.iter().map(|l| l.delta_pace_syl_s).sum::<f64>() / n;
    let mean_ser = json!(aggregate_ser(&bundle.lines));
    AttemptReport {
        label: label.into(),
        voice: voice.into(),
        gate,
        wer_mean: bundle.scores.wer_mean,
        ecapa_mean: bundle.scores.ecapa_mean,
        emotion_distance: bundle.emotion_distance,
        label_counts: bundle.label_counts.clone(),
        n_lines: bundle.lines.len(),
        mean_delta_f0_hz,
        mean_delta_pace,
        mean_ser,
        fix,
        renders_dir: renders_dir.display().to_string(),
    }
}

pub fn write_reports(path_stem: &Path, report: &VoiceCheckReport) -> Result<()> {
    if let Some(parent) = path_stem.parent() {
        fs::create_dir_all(parent)?;
    }
    let json_path = path_stem.with_extension("json");
    let md_path = path_stem.with_extension("md");
    fs::write(&json_path, serde_json::to_string_pretty(report)?)?;
    fs::write(&md_path, to_markdown(report))?;
    Ok(())
}

pub fn to_markdown(r: &VoiceCheckReport) -> String {
    let mut s = String::new();
    s.push_str(&format!("# VoiceCheck report: {}\n\n", r.voice));
    s.push_str(&format!("- **When:** {} ({})\n", r.generated_at, r.timezone));
    s.push_str(&format!("- **Persona:** {}\n", r.persona));
    s.push_str(&format!("- **Dry-run:** {}\n", r.dry_run));
    s.push_str(&format!("- **Profile:** {}\n", r.profile_source));
    s.push_str(&format!("- **Base recording:** `{}`\n", r.base_recording));
    s.push_str(&format!(
        "- **Baselines:** {}\n",
        serde_json::to_string(&r.baselines).unwrap_or_default()
    ));
    s.push_str(&format!(
        "- **Kept fix:** {}\n\n",
        r.kept.as_deref().unwrap_or("*(none)*")
    ));
    s.push_str("## Attempts\n\n");
    s.push_str("| Label | Voice | WER | ECAPA | Emo dist | ΔF0 Hz | Gate |\n");
    s.push_str("|---|---|---:|---:|---:|---:|---|\n");
    for a in &r.attempts {
        s.push_str(&format!(
            "| {} | `{}` | {:.4} | {:.4} | {} | {:+.2} | {} |\n",
            a.label,
            a.voice,
            a.wer_mean,
            a.ecapa_mean,
            a.emotion_distance
                .map(|d| format!("{d:.4}"))
                .unwrap_or_else(|| "—".into()),
            a.mean_delta_f0_hz,
            if a.gate.keep { "KEEP" } else { "reject" }
        ));
    }
    s.push_str("\n### Gate reasons\n\n");
    for a in &r.attempts {
        s.push_str(&format!("- **{}:** {}\n", a.label, a.gate.reason));
    }
    if !r.notes.is_empty() {
        s.push_str("\n## Notes\n\n");
        for n in &r.notes {
            s.push_str(&format!("- {n}\n"));
        }
    }
    s.push('\n');
    s
}

pub fn stamp_now() -> String {
    // Prefer local Europe/Zagreb via `date`.
    if let Ok(out) = std::process::Command::new("date")
        .args(["+%Y-%m-%dT%H:%M:%S%z"])
        .output()
    {
        if out.status.success() {
            return String::from_utf8_lossy(&out.stdout).trim().to_string();
        }
    }
    "unknown".into()
}

pub fn baseline_json(profile: &VoiceProfile) -> Value {
    json!({
        "wer_max": profile.baselines.wer_max,
        "ecapa_min": profile.baselines.ecapa_min,
    })
}
