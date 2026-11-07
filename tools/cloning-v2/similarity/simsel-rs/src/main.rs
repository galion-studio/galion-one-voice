mod bestof;
mod multiclip;
mod prep;
mod scorers;
mod wav;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::time::Instant;

#[derive(Parser, Debug)]
#[command(name = "simsel", about = "Similarity V2: Rust ref prep, multi-clip, best-of-N")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Trim + declick + peak/loudness normalize (TP ≤ -1 dBFS)
    PrepCleaned {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Multi-clip pack: overlapping windows from the same original
    PrepMulticlip {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long, default_value = "long")]
        mode: String,
    },
    /// Best-of-N synth selection (emotion distance primary, ECAPA secondary)
    BestOfN {
        #[arg(long, default_value = "/workspace/swarm/voice-v2-sprint/bakeoff-t648/src/cbx/target/release/cbx")]
        cbx: PathBuf,
        #[arg(long)]
        voice: String,
        #[arg(long)]
        text: String,
        #[arg(long)]
        reference: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value_t = 3)]
        n: usize,
        #[arg(long, default_value_t = 1.2)]
        repetition_penalty: f32,
        #[arg(long, default_value = "fp32")]
        dtype: String,
        #[arg(long, default_value_t = 2)]
        intra_threads: usize,
        #[arg(long)]
        work_dir: PathBuf,
        /// Emotion weight (top priority)
        #[arg(long, default_value_t = 0.65)]
        w_emotion: f64,
        #[arg(long, default_value_t = 0.35)]
        w_ecapa: f64,
    },
    /// Show which cloning-v2/scorers/*/READY are present
    ScorersStatus,
    /// Block until listed scorers are READY
    WaitScorers {
        #[arg(long, default_value = "emotion,ecapa")]
        names: String,
        #[arg(long, default_value_t = 600)]
        timeout_s: u64,
    },
    /// Score one hyp vs ref with available Rust scorers
    ScorePair {
        #[arg(long)]
        hyp: PathBuf,
        #[arg(long)]
        reference: PathBuf,
    },
    /// Rebuild all persona refs under similarity/refs from inputs/
    PrepAll {
        #[arg(long, default_value = "/workspace/swarm/voice-v2-sprint/cloning-v2/similarity")]
        root: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::PrepCleaned { input, output } => {
            let v = prep::prep_cleaned(&input, &output)?;
            println!("{}", serde_json::to_string_pretty(&v)?);
        }
        Cmd::PrepMulticlip { input, output, mode } => {
            let v = if mode == "phrases" {
                multiclip::prep_multiclip_phrases(&input, &output)?
            } else {
                multiclip::prep_multiclip_long(&input, &output)?
            };
            println!("{}", serde_json::to_string_pretty(&v)?);
        }
        Cmd::BestOfN {
            cbx,
            voice,
            text,
            reference,
            out,
            n,
            repetition_penalty,
            dtype,
            intra_threads,
            work_dir,
            w_emotion,
            w_ecapa,
        } => {
            let args = bestof::BestOfArgs {
                cbx,
                voice,
                text,
                reference,
                out,
                n,
                repetition_penalty,
                dtype,
                intra_threads,
                work_dir,
                w_emotion,
                w_ecapa,
            };
            let v = bestof::run_best_of_n(&args)?;
            println!("{}", serde_json::to_string_pretty(&v)?);
        }
        Cmd::ScorersStatus => {
            println!("{}", serde_json::to_string_pretty(&scorers::scorers_status())?);
        }
        Cmd::WaitScorers { names, timeout_s } => {
            let list: Vec<&str> = names.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
            let bins = scorers::wait_ready(&list, timeout_s)?;
            let v: Vec<_> = bins
                .iter()
                .map(|b| serde_json::json!({"name": b.name, "bin": b.bin.display().to_string()}))
                .collect();
            println!("{}", serde_json::to_string_pretty(&v)?);
        }
        Cmd::ScorePair { hyp, reference } => {
            let t0 = Instant::now();
            let mut out = serde_json::Map::new();
            for name in ["emotion", "ecapa", "articulation", "quality"] {
                if scorers::is_ready(name) {
                    let sc = scorers::load_scorer(name)?;
                    match scorers::run_pair_scorer(&sc, &hyp, &reference) {
                        Ok((v, ms)) => {
                            out.insert(name.to_string(), serde_json::json!({"ok": true, "ms": ms, "result": v}));
                        }
                        Err(e) => {
                            out.insert(name.to_string(), serde_json::json!({"ok": false, "error": e.to_string()}));
                        }
                    }
                } else {
                    out.insert(name.to_string(), serde_json::json!({"ok": false, "error": "not ready"}));
                }
            }
            out.insert("elapsed_ms".into(), serde_json::json!(t0.elapsed().as_secs_f64() * 1000.0));
            println!("{}", serde_json::to_string_pretty(&serde_json::Value::Object(out))?);
        }
        Cmd::PrepAll { root } => {
            let t0 = Instant::now();
            let inputs = root.join("inputs");
            let refs = root.join("refs");
            std::fs::create_dir_all(&refs)?;
            let mut report = serde_json::Map::new();
            for persona in ["samantha", "jarvis"] {
                let src = inputs.join(format!("{persona}-original.wav"));
                let cleaned = refs.join(format!("{persona}-rs-cleaned-ln.wav"));
                let multi = refs.join(format!("{persona}-rs-multiclip-long-ln.wav"));
                let phrases = refs.join(format!("{persona}-rs-multiclip-phrases-ln.wav"));
                let c = prep::prep_cleaned(&src, &cleaned)?;
                let m = multiclip::prep_multiclip_long(&src, &multi)?;
                let p = multiclip::prep_multiclip_phrases(&src, &phrases)?;
                report.insert(persona.to_string(), serde_json::json!({"cleaned": c, "multiclip_long": m, "multiclip_phrases": p}));
            }
            let elapsed = t0.elapsed().as_secs_f64() * 1000.0;
            let out = serde_json::json!({"elapsed_ms": elapsed, "personas": report});
            let path = refs.join("prep_report_rs.json");
            std::fs::write(&path, serde_json::to_string_pretty(&out)?)?;
            println!("{}", serde_json::to_string_pretty(&out)?);
        }
    }
    Ok(())
}
