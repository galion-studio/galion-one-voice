// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! galion-mne-contract encode|decode|validate

use galion_mne_contract::mirror;
use galion_mne_contract::validate;
use std::env;
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("encode") => cmd_convert(&mut args, true),
        Some("decode") => cmd_convert(&mut args, false),
        Some("validate") => cmd_validate(&mut args),
        Some("fixtures") => cmd_fixtures(&mut args),
        _ => {
            usage();
            ExitCode::from(2)
        }
    }
}

fn cmd_convert(args: &mut impl Iterator<Item = String>, to_binary: bool) -> ExitCode {
    let (Some(src), Some(dst)) = (args.next(), args.next()) else {
        usage();
        return ExitCode::from(2);
    };
    if args.next().is_some() {
        usage();
        return ExitCode::from(2);
    }
    let doc = match mirror::load_path(Path::new(&src)) {
        Ok(doc) => doc,
        Err(e) => return fail(e),
    };
    let dst = Path::new(&dst);
    if to_binary && dst.extension().and_then(|s| s.to_str()) != Some("mne") {
        return fail("encode writes a .mne file".into());
    }
    match mirror::store_path(dst, &doc) {
        Ok(()) => {
            println!("{} frames -> {}", doc.frame_count(), dst.display());
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}

fn cmd_validate(args: &mut impl Iterator<Item = String>) -> ExitCode {
    let Some(src) = args.next() else {
        usage();
        return ExitCode::from(2);
    };
    if args.next().is_some() {
        usage();
        return ExitCode::from(2);
    };
    let path = Path::new(&src);
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => return fail(format!("read {}: {e}", path.display())),
    };
    let doc = match mirror::parse_bytes(path, &bytes) {
        Ok(doc) => doc,
        Err(e) => return fail(e),
    };
    match validate(&doc) {
        Ok(()) => {
            let kind = match doc.kind {
                galion_mne_contract::DocKind::Intent => "intent",
                galion_mne_contract::DocKind::Readback => "readback",
            };
            println!("ok {kind} {} frames {}", doc.frame_count(), path.display());
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}

fn cmd_fixtures(args: &mut impl Iterator<Item = String>) -> ExitCode {
    let dir = args.next().unwrap_or_else(|| "fixtures".into());
    if args.next().is_some() {
        usage();
        return ExitCode::from(2);
    }
    match mirror::load_dir(Path::new(&dir)) {
        Ok(docs) if docs.is_empty() => fail(format!("no toml fixtures in {dir}")),
        Ok(docs) => {
            println!("ok {} fixtures in {dir}", docs.len());
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}

fn fail(err: String) -> ExitCode {
    eprintln!("{err}");
    ExitCode::from(1)
}

fn usage() {
    eprintln!(
        "\
galion-mne-contract encode <in.json|toml|mne> <out.mne>
galion-mne-contract decode <in.mne> <out.json|toml>
galion-mne-contract validate <file>
galion-mne-contract fixtures [dir]"
    );
}
