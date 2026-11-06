//! galion-emotion-targets
//!
//! Opt-in pipeline stage (default off):
//!   (a) detect scorer categories below presence 0.2 on the original
//!   (b) synthesize probe lines for each missing emotion
//!   (c) score probes with the unmodified universal scorer
//!   (d) write a per-emotion reference-target file
//!
//! Does not modify clones, .cbxvoice production files, scorer code, models, or thresholds.

use anyhow::{bail, Context, Result};
use clap::Parser;
use galion_emotion_targets::catalog::ProbeCatalog;
use galion_emotion_targets::detect::{detect, parse_probe_scores, parse_score_file};
use galion_emotion_targets::invoke::{
    batch_speak, default_catalog, default_cbx, score_with, sha256_file, voice_add, write_manifest,
};
use galion_emotion_targets::targets::{build_targets, pick_best_probe};
use galion_emotion_targets::windows::{
    cut_window, duration_s, parse_window_spec, plan_windows, window_filename, WindowMeta,
};
use galion_emotion_targets::scorer::{parse_scorer_kind, resolve_persona_file, resolve_scorer};
use galion_emotion_targets::{default_probe_voice_name, is_protected_voice_name, PRESENCE};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
#[command(name = "galion-emotion-targets")]
struct Args {
    /// Voice id (used in the target file and default probe-voice name). Not a deployed overwrite.
    #[arg(long)]
    voice: String,
    /// Read-only original WAV.
    #[arg(long)]
    original: PathBuf,
    /// New per-voice output directory (windows, probes, scores, TARGETS.json).
    #[arg(long)]
    out_dir: PathBuf,
    #[arg(long, default_value_t = PRESENCE)]
    presence: f64,
    #[arg(long, default_value_t = 2.5)]
    window_s: f64,
    /// Optional extra `START-END` window (same as universal_match --forced-window).
    #[arg(long)]
    forced_window: Option<String>,
    #[arg(long)]
    catalog: Option<PathBuf>,
    /// Scoring backend: `universal` (default) or `persona` (opt-in).
    #[arg(long, default_value = "universal")]
    scorer: String,
    /// Optional explicit scorer script (tests / override). Default depends on --scorer.
    #[arg(long)]
    scorer_script: Option<PathBuf>,
    /// Optional persona TOML (`pipeline/personas/*.toml`) used only with `--scorer persona`.
    #[arg(long)]
    persona_file: Option<PathBuf>,
    #[arg(long)]
    python: Option<PathBuf>,
    #[arg(long)]
    cbx: Option<PathBuf>,
    /// Name for the temporary probe profile written under out_dir/voices/. Must be new.
    #[arg(long)]
    probe_voice_name: Option<String>,
    /// Reuse an already-encoded probe profile name (read-only). Skips voice add.
    #[arg(long)]
    reuse_voice: Option<String>,
    /// Reuse existing files in --out-dir instead of refusing overwrite (no deletes).
    #[arg(long)]
    resume: bool,
}

fn write_new_json(path: &Path, value: &Value, resume: bool) -> Result<()> {
    if path.exists() {
        if resume {
            eprintln!("[targets] resume: keep {}", path.display());
            return Ok(());
        }
        bail!("refuse overwrite: {}", path.display());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(value)? + "\n")
        .with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

fn collect_wavs(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    if !dir.exists() {
        return Ok(out);
    }
    for ent in std::fs::read_dir(dir)? {
        let p = ent?.path();
        if p.extension().and_then(|e| e.to_str()) == Some("wav") {
            out.push(p);
        }
    }
    out.sort();
    Ok(out)
}

fn main() -> Result<()> {
    let args = Args::parse();
    if args.voice.is_empty() || args.voice.chars().any(|c| !c.is_ascii_alphanumeric() && c != '-' && c != '_') {
        bail!("unsafe voice name");
    }
    if !args.original.is_file() {
        bail!("original not found: {}", args.original.display());
    }
    std::fs::create_dir_all(&args.out_dir)
        .with_context(|| format!("create {}", args.out_dir.display()))?;

    let catalog_path = args.catalog.clone().unwrap_or_else(default_catalog);
    let catalog = ProbeCatalog::load(&catalog_path)?;
    let kind = parse_scorer_kind(&args.scorer)?;
    let persona_file = if kind == galion_emotion_targets::scorer::ScorerKind::Persona {
        resolve_persona_file(&args.voice, args.persona_file.as_deref())
    } else {
        None
    };
    let resolved = resolve_scorer(kind, args.scorer_script.clone(), args.python.clone(), persona_file);
    eprintln!(
        "[targets] scorer-backend={} script={} persona_file={:?} interface_loaded={}",
        resolved.kind.as_str(),
        resolved.script.display(),
        resolved.persona_file,
        resolved.interface_loaded
    );
    let cbx = args.cbx.clone().unwrap_or_else(default_cbx);
    let log = args.out_dir.join("logs/run.log");

    let sha = sha256_file(&args.original)?;
    let dur = duration_s(&args.original)?;
    let mut spans = plan_windows(dur, args.window_s);
    if let Some(spec) = &args.forced_window {
        let (s, e) = parse_window_spec(spec).with_context(|| format!("bad --forced-window {spec}"))?;
        if !spans.iter().any(|w| (w.start_s - s).abs() < 0.05 && (w.end_s - e).abs() < 0.05) {
            spans.push(galion_emotion_targets::windows::Span {
                start_s: s,
                end_s: e,
            });
        }
    }

    let win_dir = args.out_dir.join("windows");
    std::fs::create_dir_all(&win_dir)?;
    let mut metas = Vec::new();
    let mut wavs = Vec::new();
    for (i, span) in spans.iter().enumerate() {
        let name = window_filename(i, span.start_s, span.end_s);
        let dest = win_dir.join(&name);
        if dest.exists() {
            if !args.resume {
                bail!("refuse overwrite: {}", dest.display());
            }
            eprintln!("[targets] resume: reuse {}", dest.display());
        } else {
            cut_window(&args.original, &dest, span.start_s, span.end_s)?;
        }
        let meta = WindowMeta {
            id: format!("w{i:02}"),
            path: dest.to_string_lossy().to_string(),
            start_s: span.start_s,
            end_s: span.end_s,
        };
        wavs.push(dest);
        metas.push(meta);
    }
    write_new_json(&args.out_dir.join("windows_meta.json"), &json!(metas), args.resume)?;

    let win_score = args.out_dir.join("scores/original_windows.json");
    eprintln!(
        "[targets] scoring {} original windows with {} {}",
        wavs.len(),
        resolved.kind.as_str(),
        resolved.script.display()
    );
    score_with(&resolved, &wavs, &win_score, &log, args.resume)?;
    let win_doc: Value = serde_json::from_str(&std::fs::read_to_string(&win_score)?)?;
    let window_scores = parse_score_file(&win_doc, &metas)?;
    let detection = detect(&window_scores, args.presence);
    write_new_json(&args.out_dir.join("detection.json"), &json!(detection), args.resume)?;

    let missing: Vec<String> = detection
        .missing()
        .into_iter()
        .map(|c| c.category.clone())
        .collect();
    eprintln!(
        "[targets] voice={} present={} missing={:?}",
        args.voice,
        detection.present().len(),
        missing
    );

    let text_dir = args.out_dir.join("text");
    std::fs::create_dir_all(&text_dir)?;
    let mut probe_wavs: Vec<PathBuf> = Vec::new();
    let mut probes_by_cat: BTreeMap<String, galion_emotion_targets::targets::ProbePick> = BTreeMap::new();

    if !missing.is_empty() {
        if !cbx.is_file() {
            bail!("cbx binary missing: {}", cbx.display());
        }
        let probe_name = args
            .reuse_voice
            .clone()
            .or(args.probe_voice_name.clone())
            .unwrap_or_else(|| default_probe_voice_name(&args.voice));
        if is_protected_voice_name(&probe_name) {
            bail!("refusing protected probe voice name {probe_name}");
        }
        if args.reuse_voice.is_none() {
            let voice_file = args.out_dir.join("voices").join(format!("{probe_name}.cbxvoice"));
            eprintln!("[targets] encoding probe voice {probe_name} -> {}", voice_file.display());
            voice_add(&cbx, &probe_name, &args.original, &voice_file, &log, args.resume)?;
        } else {
            eprintln!("[targets] reusing probe voice {probe_name} (no voice add)");
        }

        for cat in &missing {
            let emo = catalog.get(cat)?;
            let lines: Vec<(String, String)> = emo
                .lines
                .iter()
                .map(|l| (l.id.clone(), l.text.clone()))
                .collect();
            let man = text_dir.join(format!("{cat}.tsv"));
            write_manifest(&man, &lines, args.resume)?;
            let dest = args.out_dir.join("wav").join(cat);
            eprintln!(
                "[targets] synthesize {} ({} lines, pace={}) -> {}",
                cat,
                lines.len(),
                emo.pace,
                dest.display()
            );
            let ids: Vec<String> = lines.iter().map(|(id, _)| id.clone()).collect();
            batch_speak(&cbx, &probe_name, &man, &dest, emo.pace, &log, &ids, args.resume)?;
            probe_wavs.extend(collect_wavs(&dest)?);
        }

        if probe_wavs.is_empty() {
            bail!("synthesis produced no wavs");
        }
        let probe_score = args.out_dir.join("scores/probes.json");
        eprintln!(
            "[targets] scoring {} probe wavs with {} scorer",
            probe_wavs.len(),
            resolved.kind.as_str()
        );
        score_with(&resolved, &probe_wavs, &probe_score, &log, args.resume)?;
        let probe_doc: Value = serde_json::from_str(&std::fs::read_to_string(&probe_score)?)?;
        let scored_probes = parse_probe_scores(&probe_doc)?;
        // Group probes back by parent emotion folder name when possible, else score all for each cat.
        for cat in &missing {
            let dest = args.out_dir.join("wav").join(cat);
            let marker = format!("/wav/{cat}/");
            let subset: Vec<_> = scored_probes
                .iter()
                .filter(|p| {
                    Path::new(&p.path).starts_with(&dest) || p.path.contains(&marker)
                })
                .cloned()
                .collect();
            let use_set = if subset.is_empty() {
                scored_probes.clone()
            } else {
                subset
            };
            probes_by_cat.insert(cat.clone(), pick_best_probe(cat, &use_set));
        }
    }

    let mut targets = build_targets(
        &args.voice,
        &args.original.to_string_lossy(),
        &sha,
        args.presence,
        &detection,
        &window_scores,
        &probes_by_cat,
    );
    targets["scorer"] = json!(resolved.kind.as_str());
    if let Some(pf) = &resolved.persona_file {
        targets["persona_file"] = json!(pf);
    }
    let targets_path = args.out_dir.join("TARGETS.json");
    write_new_json(&targets_path, &targets, args.resume)?;

    let summary = json!({
        "voice": args.voice,
        "original": args.original,
        "original_sha256": sha,
        "duration_s": dur,
        "n_windows": window_scores.len(),
        "presence_floor": args.presence,
        "present": detection.present().into_iter().map(|c| json!({
            "category": c.category, "strength": c.strength, "span": c.best_window_span, "source": c.source
        })).collect::<Vec<_>>(),
        "missing": missing,
        "targets": targets_path,
        "scorer": resolved.kind.as_str(),
        "scorer_script": resolved.script,
    });
    write_new_json(&args.out_dir.join("SUMMARY.json"), &summary, args.resume)?;
    eprintln!("[targets] wrote {}", targets_path.display());
    println!("{}", targets_path.display());
    Ok(())
}
