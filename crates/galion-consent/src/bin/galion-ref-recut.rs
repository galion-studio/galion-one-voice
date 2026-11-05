// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! `galion-ref-recut --in SRC.wav --out OUT.wav [--start S --end E]
//!   [--persona P [--texture T] [--update-refs]]`
//!
//! Owner item 4 (2026-11-04): re-cut a reference to 8–12 s mono 16-bit and
//! move its sha256 pin. Downmix + cut only: no denoise, no normalisation, no
//! resample. Writes OUT and OUT.sha256 (`<hex>  <name>`), never a consent
//! record. An existing OUT is refused unless `--backup`, which renames it to
//! `OUT.bak-<unix-seconds>` (nothing is deleted).
//!
//! `--persona` prints the REFS.sha256 row. `--update-refs` also swaps that
//! persona's row in `scripts/clone/REFS.sha256` (old row kept as a comment)
//! and lists the other files that still pin the old sha256.

use galion_consent::recut::{self, Pcm16};
use galion_consent::{persona_of, repo_root};
use std::path::PathBuf;
use std::process::ExitCode;

fn arg(args: &[String], key: &str) -> Option<String> {
    args.iter()
        .position(|a| a == key)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(src), Some(out)) = (arg(&args, "--in"), arg(&args, "--out")) else {
        eprintln!("usage: galion-ref-recut --in SRC.wav --out OUT.wav [--start S --end E] [--persona P [--texture T] [--update-refs]] [--backup]");
        return ExitCode::from(64);
    };
    let (src, out) = (PathBuf::from(src), PathBuf::from(out));
    if let Err(e) = recut::refuse_blocked(&src).and_then(|_| recut::refuse_blocked(&out)) {
        eprintln!("recut: {e}");
        return ExitCode::from(2);
    }
    let window = match (arg(&args, "--start"), arg(&args, "--end")) {
        (Some(a), Some(b)) => match (a.parse::<f64>(), b.parse::<f64>()) {
            (Ok(a), Ok(b)) => Some((a, b)),
            _ => {
                eprintln!("recut: --start/--end must be seconds");
                return ExitCode::from(64);
            }
        },
        (None, None) => None,
        _ => {
            eprintln!("recut: give both --start and --end, or neither");
            return ExitCode::from(64);
        }
    };
    let persona = match arg(&args, "--persona") {
        Some(p) => match persona_of(&p) {
            Some(n) => Some(n),
            None => {
                eprintln!("recut: persona must be samantha or jarvis");
                return ExitCode::from(2);
            }
        },
        None => None,
    };
    let bytes = match std::fs::read(&src) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("recut: read {}: {e}", src.display());
            return ExitCode::from(1);
        }
    };
    let pcm: Pcm16 = match recut::read_pcm16(&bytes) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("recut: {}: {e}", src.display());
            return ExitCode::from(1);
        }
    };
    let cut = match recut::recut(&pcm, window) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("recut: {e}");
            return ExitCode::from(1);
        }
    };
    if out.exists() {
        if !args.iter().any(|a| a == "--backup") {
            eprintln!(
                "recut: {} exists; pass --backup to rename it aside",
                out.display()
            );
            return ExitCode::from(2);
        }
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let mut bak = out.clone().into_os_string();
        bak.push(format!(".bak-{secs}"));
        if let Err(e) = std::fs::rename(&out, &bak) {
            eprintln!("recut: rename aside: {e}");
            return ExitCode::from(1);
        }
        let side = out.with_extension("wav.sha256");
        if side.exists() {
            let mut sb = side.clone().into_os_string();
            sb.push(format!(".bak-{secs}"));
            let _ = std::fs::rename(&side, sb);
        }
        eprintln!("recut: old file kept as {}", PathBuf::from(bak).display());
    }
    let wav = cut.wav_bytes();
    let sha = recut::sha256_hex(&wav);
    let name = out
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let mut side = out.clone().into_os_string();
    side.push(".sha256");
    if let Err(e) =
        std::fs::write(&out, &wav).and_then(|_| std::fs::write(&side, format!("{sha}  {name}\n")))
    {
        eprintln!("recut: write: {e}");
        return ExitCode::from(1);
    }
    println!(
        "source: {} ({} Hz, {} ch, {:.3} s)",
        src.display(),
        pcm.rate,
        pcm.channels,
        pcm.samples.len() as f64 / pcm.channels as f64 / pcm.rate as f64
    );
    println!(
        "cut: {:.3}-{:.3} s, mono s16 {} Hz, no denoise, no normalisation",
        cut.start_sample as f64 / cut.rate as f64,
        cut.end_sample as f64 / cut.rate as f64,
        cut.rate
    );
    println!("duration_s: {:.3}", cut.seconds());
    println!("sha256: {sha}");
    println!("wrote: {} (+ .sha256)", out.display());
    println!("consent: not written. A real consent.txt needs duration_s and sha256 above, signed by the performer.");
    let Some(persona) = persona else {
        return ExitCode::SUCCESS;
    };
    let texture = arg(&args, "--texture").unwrap_or_else(|| {
        if persona == "jarvis" {
            "preserve-hub".into()
        } else {
            "preserve".into()
        }
    });
    let row = recut::refs_row(persona, &sha, &cut, &texture);
    println!("refs_row: {row}");
    if !args.iter().any(|a| a == "--update-refs") {
        return ExitCode::SUCCESS;
    }
    // The checkout you run it from wins over the build-time path.
    let repo = std::env::current_dir()
        .ok()
        .filter(|d| d.join("scripts/clone/REFS.sha256").is_file())
        .unwrap_or_else(repo_root);
    let refs = repo.join("scripts/clone/REFS.sha256");
    let text = match std::fs::read_to_string(&refs) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("recut: read {}: {e}", refs.display());
            return ExitCode::from(1);
        }
    };
    let (new_text, old) = match recut::replace_refs_row(&text, persona, &row, "by galion-ref-recut")
    {
        Ok(v) => v,
        Err(e) => {
            eprintln!("recut: {e}");
            return ExitCode::from(1);
        }
    };
    if let Err(e) = std::fs::write(&refs, new_text) {
        eprintln!("recut: write {}: {e}", refs.display());
        return ExitCode::from(1);
    }
    println!("updated: {} ({persona} {old} -> {sha})", refs.display());
    for f in recut::files_pinning(&repo, &old) {
        if f != refs {
            println!(
                "still_pins_old: {}",
                f.strip_prefix(&repo).unwrap_or(&f).display()
            );
        }
    }
    ExitCode::SUCCESS
}
