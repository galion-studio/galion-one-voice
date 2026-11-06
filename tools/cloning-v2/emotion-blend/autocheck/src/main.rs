// Galion Voice — Perpetual Open MIT License
// (c) 2026 Maciej Grajczyk / G.A.L.I.O.N.
//! galion-voicecheck — automatic per-voice emotion/prosody check for Cloning V2.
//!
//! Usage:
//!   galion-voicecheck check --voice own-samantha [--base WAV] [--dry-run]
//!       [--renders-dir DIR] [--profile JSON] [--wer-max F] [--ecapa-min F]
//!       [--reuse-scores] [--skip-fixes] [--smoke-lines N]
//!   galion-voicecheck gate --wer F --ecapa F [--wer-max F] [--ecapa-min F]
//!   galion-voicecheck protect-name --voice NAME

mod analyze;
mod fixes;
mod gate;
mod paths;
mod profile;
mod render;
mod report;

use anyhow::{bail, Context, Result};
use gate::{evaluate, GateScores, GateThresholds};
use paths::{assert_safe_voice_name, Paths};
use profile::{persona_from_voice, resolve_profile, write_profile, VoiceProfile};
use report::{attempt_from_analysis, baseline_json, stamp_now, write_reports, VoiceCheckReport};
use serde_json::json;
use std::env;
use std::path::{Path, PathBuf};

fn usage() -> ! {
    eprintln!(
        "galion-voicecheck — automatic per-voice check + gated fixes (Cloning V2)\n\n\
USAGE:\n  galion-voicecheck check --voice NAME [options]\n  galion-voicecheck gate --wer F --ecapa F [--wer-max F] [--ecapa-min F]\n  galion-voicecheck protect-name --voice NAME\n\n\
check options:\n  --voice NAME          voice under test (e.g. own-samantha)\n  --base WAV            base recording (default: clip-store-own/<persona>/original.wav)\n  --ref WAV             ECAPA/emotion reference (default: clip-store-own/<persona>/reference.wav)\n  --profile JSON        base profile (else analysis/profile-<persona>.json or A-global/profile.json)\n  --renders-dir DIR     existing wav dir with t01.wav..t12.wav (implies no TTS for baseline)\n  --dry-run             no TTS; require --renders-dir or merge-v2 defaults for own-samantha/own-jarvis\n  --reuse-scores        prefer merge-v2 score JSON when persona matches (dry-run fast path)\n  --skip-fixes          only score baseline; do not run pluggable fixes\n  --wer-max F           gate WER ceiling (default from profile / Samantha 0.018)\n  --ecapa-min F         gate ECAPA floor; keep only if strictly greater (Samantha 0.7057)\n  --smoke-lines N       live-render only N lines (smoke); 0 = all 12 when not dry-run\n  --out-stem PATH       report stem (default reports/<voice>-<stamp>)\n"
    );
    std::process::exit(2);
}

#[derive(Default)]
struct CheckArgs {
    voice: String,
    base: Option<PathBuf>,
    reference: Option<PathBuf>,
    profile: Option<PathBuf>,
    renders_dir: Option<PathBuf>,
    dry_run: bool,
    reuse_scores: bool,
    skip_fixes: bool,
    wer_max: Option<f64>,
    ecapa_min: Option<f64>,
    smoke_lines: Option<usize>,
    out_stem: Option<PathBuf>,
}

fn parse_check(args: &[String]) -> Result<CheckArgs> {
    let mut c = CheckArgs::default();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--voice" => {
                i += 1;
                c.voice = args.get(i).context("--voice needs value")?.clone();
            }
            "--base" => {
                i += 1;
                c.base = Some(PathBuf::from(args.get(i).context("--base")?));
            }
            "--ref" => {
                i += 1;
                c.reference = Some(PathBuf::from(args.get(i).context("--ref")?));
            }
            "--profile" => {
                i += 1;
                c.profile = Some(PathBuf::from(args.get(i).context("--profile")?));
            }
            "--renders-dir" => {
                i += 1;
                c.renders_dir = Some(PathBuf::from(args.get(i).context("--renders-dir")?));
            }
            "--dry-run" => c.dry_run = true,
            "--reuse-scores" => c.reuse_scores = true,
            "--skip-fixes" => c.skip_fixes = true,
            "--wer-max" => {
                i += 1;
                c.wer_max = Some(args.get(i).context("--wer-max")?.parse()?);
            }
            "--ecapa-min" => {
                i += 1;
                c.ecapa_min = Some(args.get(i).context("--ecapa-min")?.parse()?);
            }
            "--smoke-lines" => {
                i += 1;
                c.smoke_lines = Some(args.get(i).context("--smoke-lines")?.parse()?);
            }
            "--out-stem" => {
                i += 1;
                c.out_stem = Some(PathBuf::from(args.get(i).context("--out-stem")?));
            }
            "-h" | "--help" => usage(),
            other => bail!("unknown check arg: {other}"),
        }
        i += 1;
    }
    if c.voice.is_empty() {
        bail!("--voice is required");
    }
    Ok(c)
}

fn autocheck_root() -> PathBuf {
    env::var("VOICECHECK_AUTOCHECK")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        })
}

fn cmd_gate(args: &[String]) -> Result<()> {
    let mut wer = None;
    let mut ecapa = None;
    let mut thr = GateThresholds::default();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--wer" => {
                i += 1;
                wer = Some(args.get(i).context("--wer")?.parse()?);
            }
            "--ecapa" => {
                i += 1;
                ecapa = Some(args.get(i).context("--ecapa")?.parse()?);
            }
            "--wer-max" => {
                i += 1;
                thr.wer_max = args.get(i).context("--wer-max")?.parse()?;
            }
            "--ecapa-min" => {
                i += 1;
                thr.ecapa_min = args.get(i).context("--ecapa-min")?.parse()?;
            }
            other => bail!("unknown gate arg: {other}"),
        }
        i += 1;
    }
    let scores = GateScores {
        wer_mean: wer.context("--wer required")?,
        ecapa_mean: ecapa.context("--ecapa required")?,
    };
    let r = evaluate(scores, thr);
    println!("{}", serde_json::to_string_pretty(&r)?);
    if r.keep {
        Ok(())
    } else {
        std::process::exit(1);
    }
}

fn ensure_profile_on_disk(
    paths: &Paths,
    persona: &str,
    profile: &VoiceProfile,
    loaded_from: &Path,
) -> Result<PathBuf> {
    // Publish under analysis/ ONLY if missing. Never strip worker-A extra fields
    // (original_windows / clone_lines / …) from an existing profile-*.json.
    let dest = paths
        .autocheck
        .parent()
        .unwrap()
        .join(format!("analysis/profile-{persona}.json"));
    if !dest.is_file() {
        write_profile(&dest, profile)?;
        return Ok(dest);
    }
    // Also keep a minimal snapshot under autocheck/fixtures for B-only reloads.
    let snap = paths.autocheck.join(format!("fixtures/profile-{persona}-minimal.json"));
    if !snap.is_file() {
        let _ = write_profile(&snap, profile);
    }
    Ok(if loaded_from.is_file() {
        loaded_from.to_path_buf()
    } else {
        dest
    })
}

fn collect_wavs(dir: &Path, line_ids: &[String]) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for id in line_ids {
        let p = dir.join(format!("{id}.wav"));
        if !p.is_file() {
            bail!("missing render {}", p.display());
        }
        out.push(p);
    }
    Ok(out)
}

fn cmd_check(args: &[String]) -> Result<()> {
    let a = parse_check(args)?;
    let root = autocheck_root();
    let paths = Paths::discover(&root)?;
    paths.ensure_dirs()?;
    paths.require_tools()?;

    let persona = persona_from_voice(&a.voice);
    let base_wav = a
        .base
        .clone()
        .unwrap_or_else(|| Paths::clip_store_original(&persona));
    let ref_wav = a
        .reference
        .clone()
        .unwrap_or_else(|| Paths::clip_store_reference(&persona));
    if !base_wav.is_file() {
        bail!("base recording missing: {}", base_wav.display());
    }
    if !ref_wav.is_file() {
        bail!("reference wav missing: {}", ref_wav.display());
    }

    let (profile_path, mut profile) =
        resolve_profile(&paths.autocheck, &persona, a.profile.as_deref())?;
    if profile.base_recording.is_empty() {
        profile.base_recording = base_wav.display().to_string();
    }
    if profile.reference_wav.is_empty() {
        profile.reference_wav = ref_wav.display().to_string();
    }
    if let Some(w) = a.wer_max {
        profile.baselines.wer_max = w;
    }
    if let Some(e) = a.ecapa_min {
        profile.baselines.ecapa_min = e;
    }
    let profile_path = ensure_profile_on_disk(&paths, &persona, &profile, &profile_path)?;

    let mut notes = Vec::new();
    notes.push(format!(
        "profile loaded from {} ({})",
        profile_path.display(),
        profile.source.as_deref().unwrap_or("?")
    ));

    // Line set
    let all_lines = Paths::line_ids();
    let line_ids: Vec<String> = if let Some(n) = a.smoke_lines {
        if n == 0 {
            all_lines.clone()
        } else {
            all_lines.into_iter().take(n).collect()
        }
    } else if a.dry_run {
        all_lines
    } else {
        all_lines
    };

    // Baseline renders
    let baseline_dir = if let Some(d) = &a.renders_dir {
        d.clone()
    } else if a.dry_run {
        let d = Paths::merge_v2_wavs(&persona);
        if !d.join("t01.wav").is_file() {
            bail!(
                "dry-run needs --renders-dir or merge-v2/wav/{persona}/ (missing {})",
                d.display()
            );
        }
        notes.push(format!("dry-run using existing renders at {}", d.display()));
        d
    } else {
        let d = paths.work.join(format!("{}-baseline", a.voice));
        notes.push(format!(
            "live render T0 of {} lines via cbx-speak-voice under flock",
            line_ids.len()
        ));
        render::render_set(&paths, &a.voice, &d, &line_ids, 0.0)?;
        d
    };

    let wavs = collect_wavs(&baseline_dir, &line_ids)?;

    // Score baseline
    let work = paths.work.join(format!("{}-baseline-scores", a.voice));
    let baseline_bundle = if a.reuse_scores && a.smoke_lines.is_none() {
        if let Some(b) = analyze::try_load_merge_scores(&persona, &profile.base)? {
            notes.push("reused merge-v2 score JSON for baseline ( --reuse-scores )".into());
            // Still write line deltas; copy score paths into work for the report.
            std::fs::create_dir_all(&work)?;
            for (src, name) in [
                (&b.emotion_json, "emotion.json"),
                (&b.ecapa_json, "ecapa.json"),
                (&b.wer_json, "wer.json"),
            ] {
                let _ = std::fs::copy(src, work.join(name));
            }
            b
        } else {
            analyze::score_wavs(
                &paths,
                &wavs,
                &paths.text12,
                &ref_wav,
                &work,
                &profile.base,
            )?
        }
    } else {
        analyze::score_wavs(
            &paths,
            &wavs,
            &paths.text12,
            &ref_wav,
            &work,
            &profile.base,
        )?
    };

    let mut attempts = Vec::new();
    let baseline_gate = evaluate(baseline_bundle.scores, profile.baselines);
    attempts.push(attempt_from_analysis(
        "baseline",
        &a.voice,
        &baseline_bundle,
        baseline_gate,
        None,
        &baseline_dir,
    ));

    let mut kept: Option<String> = None;

    if !a.skip_fixes {
        for kind in fixes::FixKind::all() {
            let outcome = match fixes::run_fix(
                &paths,
                kind,
                &a.voice,
                &persona,
                &base_wav,
                &ref_wav,
                &profile_path,
                a.dry_run,
            ) {
                Ok(o) => o,
                Err(e) => {
                    notes.push(format!("fix {} error: {e:#}", kind.id()));
                    continue;
                }
            };
            if outcome.status == "skipped" {
                notes.push(format!(
                    "fix {} skipped: {}",
                    kind.id(),
                    outcome.detail
                ));
                // Record skip without gate numbers.
                continue;
            }
            if outcome.status != "ok" {
                notes.push(format!(
                    "fix {} status={}: {}",
                    kind.id(),
                    outcome.status,
                    outcome.detail
                ));
                continue;
            }
            let out_voice = match &outcome.out_voice {
                Some(v) => v.clone(),
                None => continue,
            };
            // In dry-run the simple plugins do not produce new wavs; score is skipped.
            if a.dry_run {
                notes.push(format!(
                    "fix {} dry-run ok (no re-score): {}",
                    kind.id(),
                    outcome.detail
                ));
                continue;
            }
            // Live: render + score the candidate voice (respect smoke_lines).
            assert_safe_voice_name(&out_voice)?;
            let cand_dir = paths.work.join(format!("{out_voice}-wav"));
            render::render_set(&paths, &out_voice, &cand_dir, &line_ids, 0.0)?;
            let cand_wavs = collect_wavs(&cand_dir, &line_ids)?;
            let cand_work = paths.work.join(format!("{out_voice}-scores"));
            let cand_bundle = analyze::score_wavs(
                &paths,
                &cand_wavs,
                &paths.text12,
                &ref_wav,
                &cand_work,
                &profile.base,
            )?;
            let g = evaluate(cand_bundle.scores, profile.baselines);
            let keep_name = if g.keep {
                kept = Some(out_voice.clone());
                true
            } else {
                false
            };
            attempts.push(attempt_from_analysis(
                kind.id(),
                &out_voice,
                &cand_bundle,
                g,
                Some(outcome),
                &cand_dir,
            ));
            if keep_name {
                notes.push(format!("kept fix voice {out_voice}"));
                // Still try remaining fixes? Spec: "tries up to three fixes" / "keeps a fix only if".
                // Keep first passing; continue recording others but do not overwrite kept.
            }
        }
    } else {
        notes.push("fixes skipped (--skip-fixes)".into());
    }

    let stamp = stamp_now();
    let out_stem = a.out_stem.unwrap_or_else(|| {
        paths
            .reports
            .join(format!("{}-{}", a.voice, stamp.replace(':', "").replace('+', "p")))
    });

    let report = VoiceCheckReport {
        schema: "galion-voicecheck-report/1".into(),
        generated_at: stamp,
        timezone: "Europe/Zagreb (UTC+1)".into(),
        voice: a.voice.clone(),
        persona: persona.clone(),
        dry_run: a.dry_run,
        profile_source: profile_path.display().to_string(),
        baselines: baseline_json(&profile),
        base_recording: base_wav.display().to_string(),
        attempts,
        kept: kept.clone(),
        notes,
    };
    write_reports(&out_stem, &report)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "ok": true,
            "voice": a.voice,
            "persona": persona,
            "dry_run": a.dry_run,
            "kept": kept,
            "baseline": {
                "wer_mean": baseline_bundle.scores.wer_mean,
                "ecapa_mean": baseline_bundle.scores.ecapa_mean,
                "emotion_distance": baseline_bundle.emotion_distance,
            },
            "report_json": out_stem.with_extension("json").display().to_string(),
            "report_md": out_stem.with_extension("md").display().to_string(),
        }))?
    );
    Ok(())
}

fn main() {
    let mut args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        usage();
    }
    let cmd = args.remove(0);
    let result = match cmd.as_str() {
        "check" => cmd_check(&args),
        "gate" => cmd_gate(&args),
        "protect-name" => {
            let mut voice = None;
            let mut i = 0;
            while i < args.len() {
                if args[i] == "--voice" {
                    i += 1;
                    voice = args.get(i).cloned();
                }
                i += 1;
            }
            match voice {
                Some(v) => assert_safe_voice_name(&v).map(|_| {
                    println!("{}", json!({"ok": true, "voice": v}));
                }),
                None => Err(anyhow::anyhow!("--voice required")),
            }
        }
        "-h" | "--help" | "help" => usage(),
        other => Err(anyhow::anyhow!("unknown command: {other}")),
    };
    if let Err(e) = result {
        eprintln!("galion-voicecheck error: {e:#}");
        std::process::exit(1);
    }
}
