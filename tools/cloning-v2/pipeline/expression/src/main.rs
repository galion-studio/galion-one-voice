//! galion-expression — profile, plan, love/warmth diagnostic, offline eval.
//!
//! Default is inert: `plan` writes `apply: false`. Nothing here selects a
//! reference clip or touches a deployed voice.

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use galion_expression::love::{diagnose_gate_document, Observation};
use galion_expression::plan::{build_plan, PlanRequest};
use galion_expression::profile::{
    analyze_channels, overlay_from_score_record, ChannelInput, ExpressionProfile, ScoreOverlay,
};
use galion_expression::wav::read_mono_16k;
use galion_persona::load_persona;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Parser, Debug)]
#[command(name = "galion-expression")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// F0, energy, spectral tilt, vibrato, plus optional score overlays.
    Analyze {
        #[arg(long)]
        voice: String,
        /// Repeat as `emotion=path.wav`. Emotion `untagged` is allowed.
        #[arg(long = "channel")]
        channels: Vec<String>,
        /// Optional score_persona / score_universal JSON. Matched by file name.
        #[arg(long)]
        scores: Option<PathBuf>,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Conditioning candidate. `apply` stays false. `--love-warmth` arms Samantha only.
    Plan {
        #[arg(long)]
        profile: PathBuf,
        #[arg(long)]
        persona: PathBuf,
        #[arg(long)]
        love_warmth: bool,
        #[arg(long)]
        published_love_prior: bool,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Real loss vs detector miss. Reads a case JSON (reference, clones, optional gate rows).
    Diagnose {
        #[arg(long)]
        case: PathBuf,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Offline bundle for a later laptop run (RTX 4060 8 GB). CPU prosody always;
    /// CLAP love margins only with `--score`.
    Eval {
        #[arg(long)]
        wav: PathBuf,
        #[arg(long)]
        voice: String,
        /// Emotion label for this wav. Love margins are interpreted under this label.
        #[arg(long, default_value = "untagged")]
        emotion: String,
        #[arg(long)]
        persona: PathBuf,
        #[arg(long)]
        case: Option<PathBuf>,
        /// Extra observation JSON to diagnose against the case reference (a clone).
        #[arg(long)]
        clone_obs: Option<PathBuf>,
        #[arg(long)]
        out_dir: PathBuf,
        /// Run scorers/persona love prompt banks. Requires local CLAP weights.
        #[arg(long)]
        score: bool,
        /// `cpu` (default) or `cuda`. CUDA is one CLAP pass, sized for an 8 GB card.
        #[arg(long, default_value = "cpu")]
        device: String,
        #[arg(long, default_value_t = 8.0)]
        gpu_mem_gb: f64,
        #[arg(long)]
        love_warmth: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Analyze {
            voice,
            channels,
            scores,
            out,
        } => {
            let profile = analyze_cli(&voice, &channels, scores.as_deref())?;
            emit(&serde_json::to_value(&profile)?, out.as_deref())
        }
        Cmd::Plan {
            profile,
            persona,
            love_warmth,
            published_love_prior,
            out,
        } => {
            let profile: ExpressionProfile = serde_json::from_reader(
                std::fs::File::open(&profile)
                    .with_context(|| format!("open {}", profile.display()))?,
            )?;
            let persona = load_persona(&persona).map_err(|e| anyhow::anyhow!(e))?;
            let plan = build_plan(PlanRequest {
                profile: &profile,
                persona: &persona,
                love_warmth,
                published_love_prior,
            });
            emit(&serde_json::to_value(&plan)?, out.as_deref())
        }
        Cmd::Diagnose { case, out } => {
            let doc: Value = serde_json::from_reader(
                std::fs::File::open(&case).with_context(|| format!("open {}", case.display()))?,
            )?;
            emit(&diagnose_gate_document(&doc), out.as_deref())
        }
        Cmd::Eval {
            wav,
            voice,
            emotion,
            persona,
            case,
            clone_obs,
            out_dir,
            score,
            device,
            gpu_mem_gb,
            love_warmth,
        } => eval_cmd(
            &wav,
            &voice,
            &emotion,
            &persona,
            case.as_deref(),
            clone_obs.as_deref(),
            &out_dir,
            score,
            &device,
            gpu_mem_gb,
            love_warmth,
        ),
    }
}

fn emit(value: &Value, out: Option<&Path>) -> Result<()> {
    let text = serde_json::to_string_pretty(value)? + "\n";
    if let Some(path) = out {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, &text).with_context(|| format!("write {}", path.display()))?;
    } else {
        print!("{text}");
    }
    Ok(())
}

fn analyze_cli(voice: &str, channels: &[String], scores: Option<&Path>) -> Result<ExpressionProfile> {
    if channels.is_empty() {
        bail!("pass at least one --channel emotion=path.wav");
    }
    let score_doc = match scores {
        Some(p) => Some(
            serde_json::from_str::<Value>(&std::fs::read_to_string(p)?)
                .with_context(|| format!("parse {}", p.display()))?,
        ),
        None => None,
    };
    let mut inputs = Vec::new();
    for spec in channels {
        let (emotion, path) = spec
            .split_once('=')
            .context("--channel expects emotion=path.wav")?;
        let samples = read_mono_16k(Path::new(path))?;
        let overlay = score_doc
            .as_ref()
            .map(|doc| overlay_for(doc, Path::new(path)))
            .unwrap_or_default();
        inputs.push(ChannelInput {
            emotion: emotion.to_string(),
            samples_16k: samples,
            scores: overlay,
        });
    }
    Ok(analyze_channels(voice, &inputs))
}

fn overlay_for(doc: &Value, wav: &Path) -> ScoreOverlay {
    let name = wav.file_name().and_then(|s| s.to_str()).unwrap_or("");
    let files = doc.get("files").and_then(|v| v.as_array());
    if let Some(files) = files {
        for rec in files {
            let file = rec
                .get("file")
                .or_else(|| rec.get("path"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if file.ends_with(name) && !name.is_empty() {
                return overlay_from_score_record(rec);
            }
        }
    }
    ScoreOverlay::default()
}

fn eval_cmd(
    wav: &Path,
    voice: &str,
    emotion: &str,
    persona_path: &Path,
    case: Option<&Path>,
    clone_obs: Option<&Path>,
    out_dir: &Path,
    score: bool,
    device: &str,
    gpu_mem_gb: f64,
    love_warmth: bool,
) -> Result<()> {
    std::fs::create_dir_all(out_dir)?;
    let samples = read_mono_16k(wav)?;
    let mut scores = ScoreOverlay::default();
    let mut scoring = json!({"ran": false, "device": device});
    if score {
        if device == "cuda" && gpu_mem_gb + 1e-9 < 6.0 {
            bail!("CLAP on CUDA wants a card with at least 6 GB; got {gpu_mem_gb}");
        }
        let obs_path = out_dir.join("clap_observation.json");
        let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("py/eval_obs.py");
        let status = Command::new("python3")
            .arg(&script)
            .arg("--wav")
            .arg(wav)
            .arg("--persona")
            .arg(persona_path)
            .arg("--out")
            .arg(&obs_path)
            .env("GALION_CLAP_DEVICE", device)
            .status()
            .context("spawn python3 eval_obs.py")?;
        if !status.success() {
            bail!("love prompt-bank scoring failed (local CLAP weights required)");
        }
        let obs: Value = serde_json::from_str(&std::fs::read_to_string(&obs_path)?)?;
        scores.love_mean_cos = obs.get("love_mean_cos").and_then(|v| v.as_f64());
        scores.neutral_mean_cos = obs.get("neutral_mean_cos").and_then(|v| v.as_f64());
        scores.sed_mean_cos = obs.get("sed_mean_cos").and_then(|v| v.as_f64());
        scores.clap_love = obs.get("love").and_then(|v| v.as_f64());
        scores.clap_seductive = obs.get("seductive").and_then(|v| v.as_f64());
        if let Some(sad) = obs.get("e2v_sad").and_then(|v| v.as_f64()) {
            scores.e2v.insert("sad".into(), sad);
        }
        scoring = json!({
            "ran": true,
            "device": device,
            "gpu_mem_gb": gpu_mem_gb,
            "model": "laion/clap-htsat-fused",
            "sequential": true,
            "qwen": false,
            "piper": "not invoked",
            "emotion2vec": "cpu via score_persona.py --skip-clap --skip-efficientat when wanted",
        });
    }
    let profile = analyze_channels(
        voice,
        &[ChannelInput {
            emotion: emotion.into(),
            samples_16k: samples,
            scores,
        }],
    );
    let persona = load_persona(persona_path).map_err(|e| anyhow::anyhow!(e))?;
    let plan = build_plan(PlanRequest {
        profile: &profile,
        persona: &persona,
        love_warmth,
        published_love_prior: emotion != "love",
    });
    emit(
        &serde_json::to_value(&profile)?,
        Some(&out_dir.join("profile.json")),
    )?;
    emit(
        &serde_json::to_value(&plan)?,
        Some(&out_dir.join("plan.json")),
    )?;
    if let Some(case_path) = case {
        let mut doc: Value = serde_json::from_reader(std::fs::File::open(case_path)?)?;
        if let Some(extra) = clone_obs {
            let obs: Observation = serde_json::from_reader(std::fs::File::open(extra)?)?;
            let arr = doc
                .as_object_mut()
                .and_then(|o| o.get_mut("clones"))
                .and_then(|v| v.as_array_mut());
            if let Some(arr) = arr {
                arr.push(serde_json::to_value(obs)?);
            }
        }
        emit(
            &diagnose_gate_document(&doc),
            Some(&out_dir.join("diagnostic.json")),
        )?;
    }
    emit(
        &json!({
            "schema": "galion-expression-eval/1",
            "voice": voice,
            "emotion": emotion,
            "device": device,
            "gpu_mem_gb": gpu_mem_gb,
            "scoring": scoring,
            "profile": "profile.json",
            "plan": "plan.json",
            "apply": plan.apply,
            "love_warmth_apply": plan.love_warmth.apply,
        }),
        Some(&out_dir.join("eval.json")),
    )?;
    Ok(())
}
