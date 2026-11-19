// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! galion-local-studio — local clone takes, gates, 432 Hz bed, master.

#![forbid(unsafe_code)]

use galion_local_studio::command::{
    default_cbx_argv, default_ecapa_argv, default_emotion_argv, default_whisper_argv, discover,
};
use galion_local_studio::config::{Backend, LoudnessPref, StudioConfig};
use galion_local_studio::device::{choose, DeviceRequest};
use galion_local_studio::doctor;
use galion_local_studio::pipeline::{render_script, LoudnessEngine, RunInput};
use galion_local_studio::render::{CommandRenderer, SyntheticRenderer, TakeRenderer};
use galion_local_studio::score::{ProcessScorer, TakeScorer};
use galion_local_studio::wer::word_error_rate;
use std::collections::BTreeMap;
use std::path::PathBuf;

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let (cmd, args) = parse_args()?;
    match cmd.as_str() {
        "help" | "--help" | "-h" => {
            println!("{HELP}");
            Ok(())
        }
        "doctor" => {
            let cfg = match args.get("config") {
                Some(path) => Some(StudioConfig::load(PathBuf::from(path).as_path())?),
                None => None,
            };
            let body = doctor::report(cfg.as_ref(), doctor::nvidia_from_host());
            println!(
                "{}",
                serde_json::to_string_pretty(&body).map_err(|err| err.to_string())?
            );
            Ok(())
        }
        "wer" => {
            let reference = need(&args, "ref")?;
            let hypothesis = need(&args, "hyp")?;
            let measured = word_error_rate(reference, hypothesis)?;
            println!(
                "{{\"wer\":{:.6},\"errors\":{},\"reference_words\":{}}}",
                measured.rate, measured.errors, measured.reference_words
            );
            Ok(())
        }
        "render" => render(&args),
        other => Err(format!("unknown command {other}\n{HELP}")),
    }
}

fn render(args: &BTreeMap<String, String>) -> Result<(), String> {
    let mut config = StudioConfig::load(PathBuf::from(need(args, "config")?).as_path())?;
    if let Some(takes) = args.get("takes") {
        config.render.takes = takes
            .parse()
            .map_err(|_| format!("--takes {takes} is not a whole number"))?;
    }
    if let Some(device) = args.get("device") {
        config.render.device = parse_device(device)?;
    }
    if args.get("no-bed").is_some() {
        config.bed.enabled = false;
    }
    if let Some(engine) = args.get("loudness") {
        config.master.engine = parse_loudness(engine)?;
    }
    config.validate()?;
    let synthetic = args.get("synthetic").is_some();
    let tools = discover();
    let device = choose(config.render.device, doctor::nvidia_from_host())?;
    let loudness = resolve_loudness(config.master.engine, tools.ffmpeg.clone())?;
    let ffmpeg = tools.ffmpeg.clone();
    let reference = PathBuf::from(need(args, "reference")?);
    let script =
        std::fs::read_to_string(need(args, "script")?).map_err(|err| format!("script: {err}"))?;
    let out_dir = PathBuf::from(need(args, "out")?);
    let owner_flag = args.get("owner-reference").is_some();
    let repo = find_repo_root();
    let mut renderer = build_renderer(&config, synthetic, &tools)?;
    let mut scorer = build_scorer(&config, &tools)?;
    let input = RunInput {
        reference: &reference,
        script: &script,
        out_dir: &out_dir,
        repo_root: repo.as_deref(),
        owner_flag,
        device,
        loudness,
        ffmpeg,
    };
    match render_script(&config, &input, renderer.as_mut(), scorer.as_mut()) {
        Ok(output) => {
            println!(
                "renderer={} captions={} master={}",
                renderer.name(),
                output.captions_path.display(),
                output.master_wav.display()
            );
            Ok(())
        }
        Err(err) => Err(err.to_string()),
    }
}

fn build_renderer(
    config: &StudioConfig,
    synthetic: bool,
    tools: &galion_local_studio::command::ToolPaths,
) -> Result<Box<dyn TakeRenderer>, String> {
    if synthetic || config.render.backend == Backend::Synthetic {
        return Ok(Box::new(SyntheticRenderer));
    }
    let template = if !config.render.command.is_empty() {
        config.render.command.clone()
    } else if config.render.backend == Backend::Cbx {
        let bin = tools.cbx.as_ref().ok_or(
            "cbx is not on PATH. Set GALION_CBX_BIN or render.command. Weights are not downloaded",
        )?;
        default_cbx_argv(&bin.display().to_string())
    } else {
        return Err("render.command is empty".into());
    };
    Ok(Box::new(CommandRenderer {
        template,
        retry_cpu: true,
    }))
}

fn build_scorer(
    config: &StudioConfig,
    tools: &galion_local_studio::command::ToolPaths,
) -> Result<Box<dyn TakeScorer>, String> {
    let whisper = if config.whisper.command.is_empty() {
        tools
            .whisper
            .as_ref()
            .map(|path| default_whisper_argv(&path.display().to_string()))
            .unwrap_or_default()
    } else {
        config.whisper.command.clone()
    };
    let ecapa = if config.ecapa.command.is_empty() {
        tools
            .ecapa
            .as_ref()
            .map(|path| default_ecapa_argv(&path.display().to_string()))
            .unwrap_or_default()
    } else {
        config.ecapa.command.clone()
    };
    let emotion = if config.emotion.command.is_empty() {
        tools
            .emotion
            .as_ref()
            .map(|path| default_emotion_argv(&path.display().to_string()))
            .unwrap_or_default()
    } else {
        config.emotion.command.clone()
    };
    let mut missing = Vec::new();
    if whisper.is_empty() {
        missing.push("whisper");
    }
    if ecapa.is_empty() {
        missing.push("galion-ecapa");
    }
    if emotion.is_empty() {
        missing.push("galion-emotion");
    }
    if !missing.is_empty() {
        return Err(format!(
            "missing local scorer: {}. Set the command in the config or install the tool. An unmeasured score fails the gate. Models are not downloaded",
            missing.join(", ")
        ));
    }
    Ok(Box::new(ProcessScorer {
        whisper,
        ecapa,
        emotion,
        model: config.whisper.model.clone(),
        label_map: config.emotion.label_map.clone(),
    }))
}

fn resolve_loudness(pref: LoudnessPref, ffmpeg: Option<PathBuf>) -> Result<LoudnessEngine, String> {
    match pref {
        LoudnessPref::InProcess => Ok(LoudnessEngine::InProcess),
        LoudnessPref::Ffmpeg => {
            let bin = ffmpeg.ok_or("master.engine is ffmpeg and ffmpeg is not on PATH")?;
            Ok(LoudnessEngine::Ffmpeg(bin))
        }
        LoudnessPref::Auto => Ok(match ffmpeg {
            Some(bin) => LoudnessEngine::Ffmpeg(bin),
            None => LoudnessEngine::InProcess,
        }),
    }
}

fn parse_device(value: &str) -> Result<DeviceRequest, String> {
    match value {
        "auto" => Ok(DeviceRequest::Auto),
        "cpu" => Ok(DeviceRequest::Cpu),
        "cuda" => Ok(DeviceRequest::Cuda),
        other => Err(format!("--device {other} is not auto, cpu, or cuda")),
    }
}

fn parse_loudness(value: &str) -> Result<LoudnessPref, String> {
    match value {
        "auto" => Ok(LoudnessPref::Auto),
        "in-process" => Ok(LoudnessPref::InProcess),
        "ffmpeg" => Ok(LoudnessPref::Ffmpeg),
        other => Err(format!(
            "--loudness {other} is not auto, in-process, or ffmpeg"
        )),
    }
}

fn find_repo_root() -> Option<PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        if dir.join("LOCKS.toml").is_file() && dir.join(".git").exists() {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}

fn parse_args() -> Result<(String, BTreeMap<String, String>), String> {
    let mut args = std::env::args().skip(1);
    let cmd = args.next().unwrap_or_else(|| "help".into());
    let mut map = BTreeMap::new();
    while let Some(arg) = args.next() {
        let Some(key) = arg.strip_prefix("--") else {
            return Err(format!("unexpected argument {arg}"));
        };
        if key.is_empty() {
            return Err("empty flag".into());
        }
        if matches!(key, "owner-reference" | "synthetic" | "no-bed" | "help") {
            map.insert(key.to_string(), "true".into());
            continue;
        }
        let value = args
            .next()
            .ok_or_else(|| format!("--{key} needs a value"))?;
        map.insert(key.to_string(), value);
    }
    Ok((cmd, map))
}

fn need<'a>(args: &'a BTreeMap<String, String>, key: &str) -> Result<&'a str, String> {
    args.get(key)
        .map(String::as_str)
        .ok_or_else(|| format!("--{key} is required"))
}

const HELP: &str = "\
galion-local-studio — local takes, gates, 432 Hz bed, master

  render --config FILE --reference CLIP --script FILE --out DIR --owner-reference
         [--device auto|cpu|cuda] [--takes N] [--loudness auto|in-process|ffmpeg]
         [--no-bed] [--synthetic]
  doctor [--config FILE]
  wer --ref TEXT --hyp TEXT

The reference clip is the owner's own voice and is passed at runtime.
CPU is the default. auto uses CUDA when an NVIDIA GPU is visible.
Synthetic audio is a test tone. Whisper, ECAPA, the emotion classifier,
and ffmpeg are programs already on this machine. Nothing is downloaded.
";
